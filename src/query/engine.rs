use std::collections::BTreeMap;

use anyhow::Result;

use std::collections::HashSet;

use crate::format::path_alias;
use crate::index::types::Index;
use crate::ir::types::{DepKind, Dependency, Symbol, Visibility};

use super::ast::*;

/// A result row from query execution. Uses a flat key-value map for flexibility.
#[derive(Debug, Clone)]
pub struct Row {
    pub fields: BTreeMap<String, String>,
}

impl Row {
    pub fn new() -> Self {
        Row {
            fields: BTreeMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: String) {
        self.fields.insert(key.to_string(), value);
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.fields.get(key)
    }
}

/// Execute a batch of queries and return formatted output.
pub fn execute_batch(batch: &Batch, index: &Index, format: &str) -> Result<String> {
    let mut sections = Vec::new();
    let multiple = batch.queries.len() > 1;

    for (i, query) in batch.queries.iter().enumerate() {
        let rows = execute_query(query, index)?;
        let formatted = format_rows(&rows, query, format);

        if multiple {
            sections.push(format!("# Query {}\n{}", i + 1, formatted));
        } else {
            sections.push(formatted);
        }
    }

    Ok(sections.join("\n\n"))
}

/// Execute a single query against the index.
pub fn execute_query(query: &Query, index: &Index) -> Result<Vec<Row>> {
    // Step 1: resolve the source
    let mut rows = resolve_source(&query.source, index)?;

    // Step 2: apply pipeline stages
    for stage in &query.stages {
        rows = apply_stage(stage, rows, index)?;
    }

    Ok(rows)
}

/// Resolve the source clause into initial rows.
fn resolve_source(source: &Source, index: &Index) -> Result<Vec<Row>> {
    match source {
        Source::Symbols {
            kind_filter,
            in_file,
            implementing,
            where_clause,
        } => {
            let symbols: Vec<&Symbol> = if let Some(ref file_path) = in_file {
                // Find symbols whose file contains the given path substring
                index
                    .symbols
                    .iter()
                    .filter(|s| {
                        s.loc.file.to_string_lossy().contains(file_path.as_str())
                    })
                    .collect()
            } else if let Some(ref kind) = kind_filter {
                index.by_kind(kind)
            } else {
                index.symbols.iter().collect()
            };

            // Apply kind filter if both in_file and kind_filter are set
            let symbols: Vec<&Symbol> = if in_file.is_some() && kind_filter.is_some() {
                let sk = kind_filter.as_ref().unwrap();
                symbols.into_iter().filter(|s| s.kind == sk.as_str()).collect()
            } else {
                symbols
            };

            // Apply implementing filter
            let symbols = if let Some(ref trait_name) = implementing {
                const GO_KINDS: &[&str] = &["func", "struct", "interface", "method", "const", "type"];
                if kind_filter.as_deref().map(|k| GO_KINDS.contains(&k)).unwrap_or(false) {
                    return Err(anyhow::anyhow!(
                        "Go uses structural typing — `implementing` is not valid for Go.\n\
                         To find types that satisfy an interface, check which types have \
                         the required methods:\n  \
                         methods where name = <MethodName> | show parent, file"
                    ));
                }
                let implementors: HashSet<&str> = index.deps.iter()
                    .filter(|d| d.kind == DepKind::Implements && d.to_name == trait_name.as_str())
                    .map(|d| d.from_qualified.as_str())
                    .collect();
                symbols.into_iter().filter(|s| implementors.contains(s.qualified_name.as_str())).collect()
            } else {
                symbols
            };

            let mut rows: Vec<Row> = symbols.iter().map(|s| symbol_to_row(s)).collect();

            // Apply where clause
            if !where_clause.is_empty() {
                rows = filter_rows(rows, where_clause);
            }

            Ok(rows)
        }

        Source::Symbol { name, where_clause } => {
            let symbols = index.by_name(name);
            let mut rows: Vec<Row> = symbols.iter().map(|s| symbol_to_row(s)).collect();
            if !where_clause.is_empty() {
                rows = filter_rows(rows, where_clause);
            }
            Ok(rows)
        }

        Source::Deps { name, where_clause } => {
            let deps: Vec<&Dependency> = if let Some(ref n) = name {
                // Get deps for all symbols with this name
                let symbols = index.by_name(n);
                symbols
                    .iter()
                    .flat_map(|s| index.deps_of(&s.qualified_name))
                    .collect()
            } else {
                // All deps
                index.deps.iter().collect()
            };

            let mut rows: Vec<Row> = deps.iter().map(|d| dep_to_row(d)).collect();
            if !where_clause.is_empty() {
                rows = filter_rows(rows, where_clause);
            }
            Ok(rows)
        }

        Source::Refs { name, where_clause } => {
            let refs: Vec<&Dependency> = if let Some(ref n) = name {
                index.refs_to(n)
            } else {
                // All deps (refs are just deps viewed from the other side)
                index.deps.iter().collect()
            };

            let mut rows: Vec<Row> = refs.iter().map(|d| dep_to_row(d)).collect();
            if !where_clause.is_empty() {
                rows = filter_rows(rows, where_clause);
            }
            Ok(rows)
        }
    }
}

/// Convert a Symbol to a Row.
fn symbol_to_row(sym: &Symbol) -> Row {
    let mut row = Row::new();
    row.set("name", sym.name.clone());
    row.set("qualified_name", sym.qualified_name.clone());
    row.set("kind", sym.kind.clone());
    row.set("file", sym.loc.file.to_string_lossy().to_string());
    row.set("line", sym.loc.line.to_string());
    row.set(
        "visibility",
        match &sym.visibility {
            Visibility::Public => "public".to_string(),
            Visibility::Crate => "crate".to_string(),
            Visibility::Private => "private".to_string(),
        },
    );
    if let Some(ref parent) = sym.parent {
        row.set("parent", parent.clone());
    }
    if let Some(ref sig) = sym.signature {
        row.set("signature", sig.clone());
    }
    if let Some(ref ret) = sym.return_type {
        row.set("return_type", ret.clone());
    }
    if !sym.attributes.is_empty() {
        row.set("attributes", sym.attributes.join(", "));
    }
    row.set("field_count", sym.fields.len().to_string());
    row.set("param_count", sym.params.len().to_string());
    // Store the display name with parent for nicer output
    if let Some(ref parent) = sym.parent {
        row.set("display_name", format!("{}::{}", parent, sym.name));
    } else {
        row.set("display_name", sym.name.clone());
    }
    row
}

/// Convert a Dependency to a Row.
fn dep_to_row(dep: &Dependency) -> Row {
    let mut row = Row::new();
    row.set("from", dep.from_qualified.clone());
    row.set("to", dep.to_name.clone());
    row.set("dep_kind", format!("{}", dep.kind));
    row.set("file", dep.loc.file.to_string_lossy().to_string());
    row.set("line", dep.loc.line.to_string());
    row
}

/// Apply a pipeline stage to rows.
fn apply_stage(stage: &Stage, mut rows: Vec<Row>, index: &Index) -> Result<Vec<Row>> {
    match stage {
        Stage::With { enrichments } => {
            for row in &mut rows {
                for enrichment in enrichments {
                    enrich_row(row, enrichment, index);
                }
            }
            Ok(rows)
        }

        Stage::Show { columns } => {
            // Filter each row to only the specified columns
            let mut filtered = Vec::new();
            for row in &rows {
                let mut new_row = Row::new();
                for col in columns {
                    if let Some(val) = row.get(col) {
                        new_row.set(col, val.clone());
                    }
                }
                filtered.push(new_row);
            }
            Ok(filtered)
        }

        Stage::Where { conditions } => Ok(filter_rows(rows, conditions)),

        Stage::Sort { field, descending } => {
            rows.sort_by(|a, b| {
                let va = a.get(field).map(|s| s.as_str()).unwrap_or("");
                let vb = b.get(field).map(|s| s.as_str()).unwrap_or("");

                // Try numeric comparison first
                if let (Ok(na), Ok(nb)) = (va.parse::<i64>(), vb.parse::<i64>()) {
                    if *descending {
                        nb.cmp(&na)
                    } else {
                        na.cmp(&nb)
                    }
                } else if *descending {
                    vb.cmp(va)
                } else {
                    va.cmp(vb)
                }
            });
            Ok(rows)
        }

        Stage::Limit { count } => {
            rows.truncate(*count);
            Ok(rows)
        }
    }
}

/// Enrich a row with additional data from the index.
fn enrich_row(row: &mut Row, enrichment: &Enrichment, index: &Index) {
    // Look up the symbol for enrichments that need it
    let qn = row.get("qualified_name").cloned();

    match enrichment {
        Enrichment::Fields => {
            if let Some(ref qn) = qn {
                if let Some(sym) = index.by_qualified(qn) {
                    if !sym.fields.is_empty() {
                        let fields_str: Vec<String> = sym
                            .fields
                            .iter()
                            .map(|f| format!("{}: {}", f.name, f.type_name))
                            .collect();
                        row.set("fields", fields_str.join(", "));
                        row.set("field_count", sym.fields.len().to_string());
                    }
                }
            }
        }

        Enrichment::Methods => {
            let name = row.get("name").cloned();
            if let Some(ref n) = name {
                let methods: Vec<&Symbol> = index
                    .symbols
                    .iter()
                    .filter(|s| {
                        s.kind == "method"
                            && s.parent.as_deref() == Some(n.as_str())
                    })
                    .collect();
                if !methods.is_empty() {
                    let methods_str: Vec<String> = methods
                        .iter()
                        .map(|m| {
                            if let Some(ref sig) = m.signature {
                                format!("{}{}", m.name, sig.strip_prefix(&m.name).unwrap_or(sig))
                            } else {
                                m.name.clone()
                            }
                        })
                        .collect();
                    row.set("methods", methods_str.join(", "));
                    row.set("method_count", methods.len().to_string());
                }
            }
        }

        Enrichment::Params => {
            if let Some(ref qn) = qn {
                if let Some(sym) = index.by_qualified(qn) {
                    if !sym.params.is_empty() {
                        let params_str: Vec<String> = sym
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
                        row.set("params", params_str.join(", "));
                        row.set("param_count", sym.params.len().to_string());
                    }
                }
            }
        }

        Enrichment::Deps => {
            if let Some(ref qn) = qn {
                let deps = index.deps_of(qn);
                if !deps.is_empty() {
                    let deps_str: Vec<String> = deps
                        .iter()
                        .map(|d| format!("{} -> {} ({})", d.from_qualified, d.to_name, d.kind))
                        .collect();
                    row.set("deps", deps_str.join("; "));
                    row.set("dep_count", deps.len().to_string());
                }
            }
        }

        Enrichment::Refs => {
            // For refs enrichment, look up by short name (refs_to works on name)
            let name = row.get("name").cloned();
            if let Some(ref n) = name {
                let refs = index.refs_to(n);
                if !refs.is_empty() {
                    let refs_str: Vec<String> = refs
                        .iter()
                        .map(|r| format!("{} ({}) from {}", r.to_name, r.kind, r.from_qualified))
                        .collect();
                    row.set("refs", refs_str.join("; "));
                    row.set("ref_count", refs.len().to_string());
                }
            }
        }

        Enrichment::Signature => {
            if let Some(ref qn) = qn {
                if let Some(sym) = index.by_qualified(qn) {
                    if let Some(ref sig) = sym.signature {
                        row.set("signature", sig.clone());
                    }
                }
            }
        }
    }
}

/// Filter rows by DNF conditions (OR of AND groups).
fn filter_rows(rows: Vec<Row>, or_groups: &[Vec<Condition>]) -> Vec<Row> {
    rows.into_iter()
        .filter(|row| {
            or_groups.iter().any(|group| {
                group.iter().all(|c| matches_condition(row, c))
            })
        })
        .collect()
}

/// Check if a row matches a condition.
fn matches_condition(row: &Row, condition: &Condition) -> bool {
    let field_val = row.get(&condition.field);

    match &condition.op {
        Op::Eq => {
            let target = condition.value.as_str();
            if let Some(val) = field_val {
                val.to_lowercase() == target.to_lowercase()
                    || val == target
            } else {
                false
            }
        }
        Op::NotEq => {
            let target = condition.value.as_str();
            if let Some(val) = field_val {
                val.to_lowercase() != target.to_lowercase()
            } else {
                true
            }
        }
        Op::Contains => {
            let target = condition.value.as_str();
            if let Some(val) = field_val {
                val.to_lowercase().contains(&target.to_lowercase())
            } else {
                false
            }
        }
        Op::StartsWith => {
            let target = condition.value.as_str();
            if let Some(val) = field_val {
                val.to_lowercase().starts_with(&target.to_lowercase())
            } else {
                false
            }
        }
        Op::EndsWith => {
            let target = condition.value.as_str();
            if let Some(val) = field_val {
                val.to_lowercase().ends_with(&target.to_lowercase())
            } else {
                false
            }
        }
        Op::Gt | Op::Lt | Op::Gte | Op::Lte => {
            let target_num = condition.value.as_number();
            let field_num = field_val.and_then(|v| v.parse::<i64>().ok());

            match (field_num, target_num) {
                (Some(fv), Some(tv)) => match condition.op {
                    Op::Gt => fv > tv,
                    Op::Lt => fv < tv,
                    Op::Gte => fv >= tv,
                    Op::Lte => fv <= tv,
                    _ => unreachable!(),
                },
                _ => false,
            }
        }
    }
}

/// Format rows into output text.
fn format_rows(rows: &[Row], query: &Query, format: &str) -> String {
    if rows.is_empty() {
        return "No results.".to_string();
    }

    match format {
        "json" => format_json(rows),
        _ => format_text(rows, query),
    }
}

/// Format rows as JSON.
fn format_json(rows: &[Row]) -> String {
    let values: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            let map: serde_json::Map<String, serde_json::Value> = row
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect();
            serde_json::Value::Object(map)
        })
        .collect();
    serde_json::to_string_pretty(&values).unwrap_or_else(|_| "[]".to_string())
}

/// Format rows as aligned text columns.
fn format_text(rows: &[Row], query: &Query) -> String {
    if rows.is_empty() {
        return "No results.".to_string();
    }

    // Determine which columns to show
    let columns = determine_columns(rows, query);
    if columns.is_empty() {
        return "No results.".to_string();
    }

    // Compute path alias if "file" column is present
    let has_file_col = columns.contains(&"file".to_string());
    let alias = if has_file_col {
        let file_paths: Vec<&str> = rows
            .iter()
            .filter_map(|row| row.get("file").map(|v| v.as_str()))
            .collect();
        path_alias::compute_path_alias(&file_paths)
    } else {
        None
    };

    // Apply alias to file values for display (create shortened copies)
    let display_rows: Vec<Row> = if let Some(ref a) = alias {
        rows.iter()
            .map(|row| {
                let mut new_row = row.clone();
                if let Some(file_val) = row.get("file") {
                    new_row.set("file", a.shorten(file_val));
                }
                new_row
            })
            .collect()
    } else {
        rows.to_vec()
    };

    // Compute column widths using display rows
    let widths: Vec<usize> = columns
        .iter()
        .map(|col| {
            display_rows
                .iter()
                .map(|row| row.get(col).map(|v| v.len()).unwrap_or(0))
                .max()
                .unwrap_or(0)
                .max(col.len())
        })
        .collect();

    let mut lines = Vec::new();

    // Emit alias header if applicable
    if let Some(ref a) = alias {
        lines.push(a.header());
        lines.push(String::new());
    }

    for row in &display_rows {
        let parts: Vec<String> = columns
            .iter()
            .zip(widths.iter())
            .map(|(col, width)| {
                let val = row.get(col).map(|v| v.as_str()).unwrap_or("");
                format!("{:<width$}", val, width = width)
            })
            .collect();
        lines.push(parts.join("  ").trim_end().to_string());
    }

    lines.join("\n")
}

/// Determine which columns to display based on the query and row data.
fn determine_columns(rows: &[Row], query: &Query) -> Vec<String> {
    // If there's an explicit "show" stage, use those columns
    for stage in &query.stages {
        if let Stage::Show { columns } = stage {
            return columns.clone();
        }
    }

    // Otherwise, pick smart defaults based on what's in the rows
    let sample = &rows[0];

    // Check if these are symbol rows or dep rows
    let is_dep_row = sample.get("from").is_some() && sample.get("to").is_some();

    if is_dep_row {
        let mut cols = vec!["dep_kind".to_string(), "from".to_string(), "to".to_string()];
        cols.push("file".to_string());
        cols.push("line".to_string());
        cols
    } else {
        let mut cols = vec!["kind".to_string(), "display_name".to_string()];
        cols.push("file".to_string());
        cols.push("line".to_string());

        // Add enrichment columns if present (check any row, not just first)
        let has_col = |col: &str| rows.iter().any(|r| r.get(col).is_some());
        if has_col("fields") {
            cols.push("fields".to_string());
        }
        if has_col("methods") {
            cols.push("methods".to_string());
        }
        if has_col("params") {
            cols.push("params".to_string());
        }
        if has_col("signature") && !cols.contains(&"signature".to_string()) {
            cols.push("signature".to_string());
        }
        if has_col("deps") {
            cols.push("deps".to_string());
        }
        if has_col("refs") {
            cols.push("refs".to_string());
        }
        cols
    }
}
