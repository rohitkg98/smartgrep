use anyhow::Result;

use crate::format::OutputFormat;
use crate::index::auto;
use crate::ir::types::{Symbol, SymbolKind, Visibility};

/// Run the `show` command: display full detail for a named symbol.
pub fn run(name: &str, format_str: &str, project_root: &Option<std::path::PathBuf>) -> Result<()> {
    let root = resolve_root(project_root)?;
    let index = auto::ensure_index(&root)?;

    let symbols = index.by_name(name);

    if symbols.is_empty() {
        eprintln!("No symbol found matching '{}'", name);
        return Ok(());
    }

    let output = match OutputFormat::from_str(format_str) {
        OutputFormat::Json => format_json(&symbols),
        OutputFormat::Text => format_text(&symbols),
    };

    println!("{}", output);
    Ok(())
}

/// Format symbol detail as text, usable from tests.
pub fn format_text(symbols: &[&Symbol]) -> String {
    let mut sections = Vec::new();

    for sym in symbols {
        sections.push(format_symbol_detail(sym));
    }

    sections.join("\n---\n")
}

fn format_symbol_detail(sym: &Symbol) -> String {
    let mut lines = Vec::new();

    // Kind and qualified name
    lines.push(format!("{} {}", sym.kind, sym.qualified_name));

    // Location
    lines.push(format!("  file: {}:{}", sym.loc.file.display(), sym.loc.line));

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
    if matches!(sym.kind, SymbolKind::Function | SymbolKind::Method) && !sym.params.is_empty() {
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

fn resolve_root(project_root: &Option<std::path::PathBuf>) -> Result<std::path::PathBuf> {
    if let Some(root) = project_root {
        return Ok(root.clone());
    }
    let cwd = std::env::current_dir()?;
    auto::detect_project_root(&cwd)
        .ok_or_else(|| anyhow::anyhow!("Could not find Cargo.toml in any parent directory"))
}
