use anyhow::Result;

use crate::daemon::client;
use crate::index::auto;
use crate::ir::types::SymbolKind;
use crate::format::OutputFormat;

/// Run the `ls` command: list symbols, optionally filtered by kind.
pub fn run(symbol_type: &Option<String>, format_str: &str, project_root: &Option<std::path::PathBuf>, no_daemon: bool) -> Result<()> {
    let root = resolve_root(project_root)?;

    // Try daemon first (auto-starts if needed, skipped if --no-daemon)
    let args = symbol_type.as_deref().unwrap_or("");
    if let Some(output) = client::try_daemon(&root, "ls", args, format_str, no_daemon) {
        println!("{}", output);
        return Ok(());
    }

    let index = auto::ensure_index(&root)?;

    let kind_filter = symbol_type.as_deref().and_then(parse_kind_filter);

    let symbols: Vec<_> = if let Some(ref kind) = kind_filter {
        index.by_kind(kind)
    } else {
        index.symbols.iter().collect()
    };

    let output = match OutputFormat::from_str(format_str) {
        OutputFormat::Json => format_json(&symbols),
        OutputFormat::Text => format_text(&symbols),
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

fn parse_kind_filter(s: &str) -> Option<SymbolKind> {
    match s.to_lowercase().as_str() {
        "functions" | "function" | "fn" => Some(SymbolKind::Function),
        "methods" | "method" => Some(SymbolKind::Method),
        "structs" | "struct" => Some(SymbolKind::Struct),
        "enums" | "enum" => Some(SymbolKind::Enum),
        "traits" | "trait" => Some(SymbolKind::Trait),
        "impls" | "impl" => Some(SymbolKind::Impl),
        "consts" | "const" => Some(SymbolKind::Const),
        "types" | "type" => Some(SymbolKind::TypeAlias),
        "modules" | "module" | "mod" => Some(SymbolKind::Module),
        _ => None,
    }
}

fn format_text(symbols: &[&crate::ir::types::Symbol]) -> String {
    if symbols.is_empty() {
        return "No symbols found.".to_string();
    }

    let kind_width = symbols
        .iter()
        .map(|s| format!("{}", s.kind).len())
        .max()
        .unwrap_or(0);
    let name_width = symbols
        .iter()
        .map(|s| display_name(s).len())
        .max()
        .unwrap_or(0);

    let mut lines = Vec::new();
    for sym in symbols {
        let kind_str = format!("{}", sym.kind);
        let name = display_name(sym);
        let loc = format!("{}:{}", sym.loc.file.display(), sym.loc.line);

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

fn display_name(sym: &crate::ir::types::Symbol) -> String {
    if let Some(ref parent) = sym.parent {
        format!("{}::{}", parent, sym.name)
    } else {
        sym.name.clone()
    }
}

fn build_extra(sym: &crate::ir::types::Symbol) -> String {
    match sym.kind {
        SymbolKind::Function | SymbolKind::Method => {
            let params: Vec<String> = sym
                .params
                .iter()
                .filter(|p| p.name != "self")
                .map(|p| {
                    if p.type_name.is_empty() {
                        p.name.clone()
                    } else {
                        format!("{}: {}", p.name, p.type_name)
                    }
                })
                .collect();
            let ret = sym
                .return_type
                .as_ref()
                .map(|r| format!(" {}", r))
                .unwrap_or_default();
            format!("({}){}", params.join(", "), ret)
        }
        SymbolKind::Struct => {
            if sym.fields.is_empty() {
                String::new()
            } else {
                let field_names: Vec<&str> = sym.fields.iter().map(|f| f.name.as_str()).collect();
                format!("{{{}}}", field_names.join(", "))
            }
        }
        _ => String::new(),
    }
}

fn format_json(symbols: &[&crate::ir::types::Symbol]) -> String {
    serde_json::to_string_pretty(&symbols).unwrap_or_else(|_| "[]".to_string())
}
