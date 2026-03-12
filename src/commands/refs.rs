use anyhow::Result;

use crate::index::auto;
use crate::ir::types::Dependency;
use crate::format::OutputFormat;

/// Run the `refs` command: show what references a given symbol.
pub fn run(name: &str, format_str: &str, project_root: &Option<std::path::PathBuf>) -> Result<()> {
    let root = resolve_root(project_root)?;
    let index = auto::ensure_index(&root)?;

    let refs = index.refs_to(name);

    if refs.is_empty() {
        println!("No references found for '{}'.", name);
        return Ok(());
    }

    let output = match OutputFormat::from_str(format_str) {
        OutputFormat::Json => format_json(&refs),
        OutputFormat::Text => format_text(&refs),
    };

    println!("{}", output);
    Ok(())
}

fn resolve_root(project_root: &Option<std::path::PathBuf>) -> Result<std::path::PathBuf> {
    if let Some(root) = project_root {
        return Ok(root.clone());
    }
    let cwd = std::env::current_dir()?;
    auto::detect_project_root(&cwd)
        .ok_or_else(|| anyhow::anyhow!("Could not find Cargo.toml in any parent directory"))
}

fn format_text(refs: &[&Dependency]) -> String {
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
    for dep in refs {
        let kind = dep_kind_str(dep);
        let loc = format!("{}:{}", dep.loc.file.display(), dep.loc.line);
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
        crate::ir::types::DepKind::Import => "Import",
        crate::ir::types::DepKind::FunctionCall => "FunctionCall",
        crate::ir::types::DepKind::TypeReference => "TypeReference",
        crate::ir::types::DepKind::TraitImpl => "TraitImpl",
        crate::ir::types::DepKind::FieldType => "FieldType",
    }
}

fn format_json(refs: &[&Dependency]) -> String {
    serde_json::to_string_pretty(&refs).unwrap_or_else(|_| "[]".to_string())
}
