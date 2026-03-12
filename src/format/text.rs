use crate::format::path_alias;
use crate::ir::types::{Ir, Symbol, SymbolKind};

/// Format IR symbols as greppable text output.
/// Each line: kind\tname\tfile:line\t[extra]
pub fn format_symbols(ir: &Ir) -> String {
    if ir.symbols.is_empty() {
        return String::new();
    }

    // Collect file paths for alias detection
    let file_paths: Vec<&str> = ir
        .symbols
        .iter()
        .map(|s| s.loc.file.to_str().unwrap_or(""))
        .collect();
    let alias = path_alias::compute_path_alias(&file_paths);

    let mut lines = Vec::new();

    // Emit alias header if applicable
    if let Some(ref a) = alias {
        lines.push(a.header());
        lines.push(String::new()); // blank line after header
    }

    // Compute column widths for alignment
    let kind_width = ir
        .symbols
        .iter()
        .map(|s| format!("{}", s.kind).len())
        .max()
        .unwrap_or(0);
    let name_width = ir
        .symbols
        .iter()
        .map(|s| display_name(s).len())
        .max()
        .unwrap_or(0);

    for sym in &ir.symbols {
        let kind_str = format!("{}", sym.kind);
        let name = display_name(sym);
        let raw_file = sym.loc.file.to_string_lossy();
        let file_str = if let Some(ref a) = alias {
            a.shorten(&raw_file)
        } else {
            raw_file.to_string()
        };
        let loc = format!("{}:{}", file_str, sym.loc.line);

        let extra = build_extra(sym);

        let line = if extra.is_empty() {
            format!("{:<kw$}  {:<nw$}  {}", kind_str, name, loc, kw = kind_width, nw = name_width)
        } else {
            format!(
                "{:<kw$}  {:<nw$}  {}  {}",
                kind_str,
                name,
                loc,
                extra,
                kw = kind_width,
                nw = name_width,
            )
        };
        lines.push(line);
    }

    lines.join("\n")
}

fn display_name(sym: &Symbol) -> String {
    if let Some(ref parent) = sym.parent {
        format!("{}::{}", parent, sym.name)
    } else {
        sym.name.clone()
    }
}

fn build_extra(sym: &Symbol) -> String {
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
