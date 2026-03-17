use anyhow::Result;

use crate::daemon::client;
use crate::format::path_alias;
use crate::ir::types::Dependency;
use crate::format::OutputFormat;

/// Run the `refs` command: show what references a given symbol.
pub fn run(name: &str, format_str: &str, project_root: &Option<std::path::PathBuf>, use_daemon: bool) -> Result<()> {
    let root = super::resolve_root(project_root)?;

    // Try daemon first (auto-starts if needed, skipped if --no-daemon)
    if let Some(output) = client::try_daemon(&root, "refs", name, format_str, use_daemon) {
        println!("{}", output);
        return Ok(());
    }

    let start = std::time::Instant::now();
    let index = crate::index::auto::ensure_index(&root)?;

    let refs = index.refs_to(name);

    if refs.is_empty() {
        super::log_direct(&root, "refs", name, "", start.elapsed().as_millis() as u64);
        println!("No references found for '{}'.", name);
        return Ok(());
    }

    let output = match format_str.parse::<OutputFormat>().unwrap() {
        OutputFormat::Json => format_json(&refs),
        OutputFormat::Text => format_text(&refs),
    };

    super::log_direct(&root, "refs", name, &output, start.elapsed().as_millis() as u64);
    println!("{}", output);
    Ok(())
}

pub fn format_text(refs: &[&Dependency]) -> String {
    // Collect file paths for display optimization
    let file_paths: Vec<&str> = refs
        .iter()
        .map(|d| d.loc.file.to_str().unwrap_or(""))
        .collect();
    let display = path_alias::compute_path_display(&file_paths);

    let kind_width = refs
        .iter()
        .map(|d| dep_kind_str(d).len())
        .max()
        .unwrap_or(0);
    let from_width = refs
        .iter()
        .map(|d| d.from_qualified.len())
        .max()
        .unwrap_or(0);

    let mut lines = Vec::new();

    // Emit header if applicable
    if let Some(ref d) = display {
        lines.push(d.header());
        lines.push(String::new());
    }

    for dep in refs {
        let kind = dep_kind_str(dep);
        let raw_file = dep.loc.file.to_string_lossy();
        let loc = if let Some(ref d) = display {
            d.format_loc(&raw_file, dep.loc.line)
        } else {
            format!("{}:{}", raw_file, dep.loc.line)
        };
        lines.push(format!(
            "{:<kw$}  {:<fw$}  {}",
            kind, dep.from_qualified, loc,
            kw = kind_width, fw = from_width,
        ));
    }
    lines.join("\n")
}

fn dep_kind_str(dep: &Dependency) -> &'static str {
    match dep.kind {
        crate::ir::types::DepKind::Import => "import",
        crate::ir::types::DepKind::Call => "call",
        crate::ir::types::DepKind::TypeRef => "type_ref",
        crate::ir::types::DepKind::Implements => "implements",
        crate::ir::types::DepKind::FieldType => "field_type",
    }
}

fn format_json(refs: &[&Dependency]) -> String {
    serde_json::to_string_pretty(&refs).unwrap_or_else(|_| "[]".to_string())
}
