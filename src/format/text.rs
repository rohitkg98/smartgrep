use crate::format::path_alias;
use crate::ir::types::{Ir, Symbol};

/// Format IR symbols as greppable text output.
/// Each line: kind\tname\tfile:line\t[extra]
pub fn format_symbols(ir: &Ir) -> String {
    if ir.symbols.is_empty() {
        return String::new();
    }

    // Collect file paths for display optimization
    let file_paths: Vec<&str> = ir
        .symbols
        .iter()
        .map(|s| s.loc.file.to_str().unwrap_or(""))
        .collect();
    let display = path_alias::compute_path_display(&file_paths);

    let mut lines = Vec::new();

    // Emit header if applicable
    if let Some(ref d) = display {
        lines.push(d.header());
        lines.push(String::new()); // blank line after header
    }

    // Compute column widths for alignment
    let kind_width = ir
        .symbols
        .iter()
        .map(|s| s.kind.len())
        .max()
        .unwrap_or(0);
    let name_width = ir
        .symbols
        .iter()
        .map(|s| display_name(s).len())
        .max()
        .unwrap_or(0);

    for sym in &ir.symbols {
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

pub fn display_name(sym: &Symbol) -> String {
    if let Some(ref parent) = sym.parent {
        format!("{}::{}", parent, sym.name)
    } else {
        sym.name.clone()
    }
}

pub fn build_extra(sym: &Symbol) -> String {
    match sym.kind.as_str() {
        "fn" | "func" | "function" | "method" => {
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
        "struct" | "class" | "record" => {
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
