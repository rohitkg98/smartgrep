use anyhow::Result;

use crate::daemon::client;
use crate::format::path_alias;
use crate::format::OutputFormat;
use crate::index::auto;
use crate::ir::types::{Symbol, Visibility};

/// Run the `show` command: display full detail for a named symbol.
pub fn run(name: &str, format_str: &str, project_root: &Option<std::path::PathBuf>, use_daemon: bool) -> Result<()> {
    let root = super::resolve_root(project_root)?;

    // Try daemon first (auto-starts if needed, skipped if --no-daemon)
    if let Some(output) = client::try_daemon(&root, "show", name, format_str, use_daemon) {
        println!("{}", output);
        return Ok(());
    }

    let start = std::time::Instant::now();
    let index = auto::ensure_index(&root)?;

    let symbols = index.by_name(name);

    if symbols.is_empty() {
        super::log_direct(&root, "show", name, "", start.elapsed().as_millis() as u64);
        eprintln!("No symbol found matching '{}'", name);
        return Ok(());
    }

    let output = match format_str.parse::<OutputFormat>().unwrap() {
        OutputFormat::Json => format_json(&symbols),
        OutputFormat::Text => format_text(&symbols),
    };

    super::log_direct(&root, "show", name, &output, start.elapsed().as_millis() as u64);
    println!("{}", output);
    Ok(())
}

/// Format symbol detail as text, usable from tests.
pub fn format_text(symbols: &[&Symbol]) -> String {
    // Compute path alias across all symbols
    let file_paths: Vec<&str> = symbols
        .iter()
        .map(|s| s.loc.file.to_str().unwrap_or(""))
        .collect();
    let alias = path_alias::compute_path_alias(&file_paths);

    let mut sections = Vec::new();

    // Emit alias header if applicable
    if let Some(ref a) = alias {
        sections.push(a.header());
    }

    for sym in symbols {
        sections.push(format_symbol_detail(sym, alias.as_ref()));
    }

    if alias.is_some() {
        // Header is first, then join detail sections with ---
        let header = sections.remove(0);
        format!("{}\n\n{}", header, sections.join("\n---\n"))
    } else {
        sections.join("\n---\n")
    }
}

fn format_symbol_detail(sym: &Symbol, alias: Option<&path_alias::PathAlias>) -> String {
    let mut lines = Vec::new();

    // Kind and qualified name
    lines.push(format!("{} {}", sym.kind, sym.qualified_name));

    // Location
    let raw_file = sym.loc.file.to_string_lossy();
    let file_str = if let Some(a) = alias {
        a.shorten(&raw_file)
    } else {
        raw_file.to_string()
    };
    lines.push(format!("  file: {}:{}", file_str, sym.loc.line));

    // Visibility
    lines.push(format!("  visibility: {}", visibility_str(&sym.visibility)));

    // Parent (for methods)
    if let Some(ref parent) = sym.parent {
        lines.push(format!("  parent: {}", parent));
    }

    // Signature
    if let Some(ref sig) = sym.signature {
        lines.push(format!("  signature: {}", sig));
    }

    // Params (for functions/methods)
    if (sym.kind == "fn" || sym.kind == "func" || sym.kind == "method") && !sym.params.is_empty() {
        let param_strs: Vec<String> = sym
            .params
            .iter()
            .map(|p| {
                if p.type_name.is_empty() {
                    p.name.clone()
                } else {
                    format!("{}: {}", p.name, p.type_name)
                }
            })
            .collect();
        lines.push(format!("  params: ({})", param_strs.join(", ")));
    }

    // Return type
    if let Some(ref ret) = sym.return_type {
        lines.push(format!("  returns: {}", ret));
    }

    // Fields (for structs)
    if !sym.fields.is_empty() {
        lines.push("  fields:".to_string());
        for f in &sym.fields {
            lines.push(format!(
                "    {} {}: {}",
                visibility_str(&f.visibility),
                f.name,
                f.type_name
            ));
        }
    }

    // Attributes
    if !sym.attributes.is_empty() {
        lines.push(format!("  attributes: {}", sym.attributes.join(", ")));
    }

    lines.join("\n")
}

fn visibility_str(vis: &Visibility) -> &'static str {
    match vis {
        Visibility::Public => "pub",
        Visibility::Crate => "pub(crate)",
        Visibility::Private => "private",
    }
}

fn format_json(symbols: &[&Symbol]) -> String {
    serde_json::to_string_pretty(&symbols).unwrap_or_else(|_| "[]".to_string())
}
