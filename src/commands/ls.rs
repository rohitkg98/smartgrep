use anyhow::Result;

use crate::format::path_alias;
use crate::format::text::{display_name, build_extra};
use crate::format::OutputFormat;
use crate::index::auto;
use crate::query::parser::normalize_kind_filter;

/// Run the `ls` command: list symbols, optionally filtered by kind and file path.
pub fn run(symbol_type: &Option<String>, in_path: &Option<String>, format_str: &str, project_root: &Option<std::path::PathBuf>, use_daemon: bool) -> Result<()> {
    let root = super::resolve_root(project_root)?;

    // Try daemon first (auto-starts if needed, skipped if --no-daemon)
    let kind_arg = symbol_type.as_deref().unwrap_or("");
    let args = match in_path {
        Some(path) => format!("{} --in {}", kind_arg, path),
        None => kind_arg.to_string(),
    };
    if let Some(output) = client::try_daemon(&root, "ls", &args, format_str, use_daemon) {
        println!("{}", output);
        return Ok(());
    }

    let start = std::time::Instant::now();
    let index = auto::ensure_index(&root)?;

    let kind_filter = symbol_type.as_deref().and_then(normalize_kind_filter);

    let mut symbols: Vec<_> = if let Some(ref kinds) = kind_filter {
        let kind_refs: Vec<&str> = kinds.iter().map(|s| s.as_str()).collect();
        index.by_kinds(&kind_refs)
    } else {
        index.symbols.iter().collect()
    };

    // Filter by file path substring if --in is specified
    if let Some(ref path) = in_path {
        symbols.retain(|s| s.loc.file.to_string_lossy().contains(path.as_str()));
    }

    let output = match format_str.parse::<OutputFormat>().unwrap() {
        OutputFormat::Json => format_json(&symbols),
        OutputFormat::Text => format_text(&symbols),
    };

    super::log_direct(&root, "ls", &args, &output, start.elapsed().as_millis() as u64);
    println!("{}", output);
    Ok(())
}

use crate::daemon::client;

pub fn format_text(symbols: &[&crate::ir::types::Symbol]) -> String {
    if symbols.is_empty() {
        return "No symbols found.".to_string();
    }

    // Collect file paths for display optimization
    let file_paths: Vec<&str> = symbols
        .iter()
        .map(|s| s.loc.file.to_str().unwrap_or(""))
        .collect();
    let display = path_alias::compute_path_display(&file_paths);

    let kind_width = symbols
        .iter()
        .map(|s| s.kind.len())
        .max()
        .unwrap_or(0);
    let name_width = symbols
        .iter()
        .map(|s| display_name(s).len())
        .max()
        .unwrap_or(0);

    let mut lines = Vec::new();

    // Emit header if applicable
    if let Some(ref d) = display {
        lines.push(d.header());
        lines.push(String::new());
    }

    for sym in symbols {
        let kind_str = &sym.kind;
        let name = display_name(sym);
        let raw_file = sym.loc.file.to_string_lossy();
        let loc = if let Some(ref d) = display {
            d.format_loc(&raw_file, sym.loc.line)
        } else {
            format!("{}:{}", raw_file, sym.loc.line)
        };

        let extra = build_extra(sym);

        let line = if extra.is_empty() {
            format!(
                "{:<kw$}  {:<nw$}  {}",
                kind_str, name, loc,
                kw = kind_width, nw = name_width,
            )
        } else {
            format!(
                "{:<kw$}  {:<nw$}  {}  {}",
                kind_str, name, loc, extra,
                kw = kind_width, nw = name_width,
            )
        };
        lines.push(line);
    }
    lines.join("\n")
}

fn format_json(symbols: &[&crate::ir::types::Symbol]) -> String {
    serde_json::to_string_pretty(&symbols).unwrap_or_else(|_| "[]".to_string())
}
