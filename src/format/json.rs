use crate::ir::types::Ir;

/// Format IR symbols as a JSON array.
pub fn format_symbols(ir: &Ir) -> String {
    serde_json::to_string_pretty(&ir.symbols).unwrap_or_else(|_| "[]".to_string())
}
