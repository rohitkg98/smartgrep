use anyhow::{anyhow, Result};

use crate::ir::types::SymbolKind;

use super::ast::*;

/// Parse a query string into a Batch of queries.
pub fn parse(input: &str) -> Result<Batch> {
    let input = input.trim();
    if input.is_empty() {
        return Err(anyhow!("empty query"));
    }

    // Split on ";" for batch queries, but not inside quoted strings
    let query_strings = split_respecting_quotes(input, ';');
    let mut queries = Vec::new();

    for qs in &query_strings {
        let q = parse_query(qs.trim())?;
        queries.push(q);
    }

    Ok(Batch { queries })
}

/// Split a string on a delimiter character, respecting quoted strings.
fn split_respecting_quotes(input: &str, delimiter: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut quote_char = '"';

    for ch in input.chars() {
        if !in_quote && (ch == '\'' || ch == '"') {
            in_quote = true;
            quote_char = ch;
            current.push(ch);
        } else if in_quote && ch == quote_char {
            in_quote = false;
            current.push(ch);
        } else if !in_quote && ch == delimiter {
            parts.push(current.clone());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

/// Parse a single query (no ";" separators).
fn parse_query(input: &str) -> Result<Query> {
    // Split on "|" respecting quotes
    let segments = split_respecting_quotes(input, '|');
    if segments.is_empty() {
        return Err(anyhow!("empty query"));
    }

    let source = parse_source(segments[0].trim())?;
    let mut stages = Vec::new();

    for seg in &segments[1..] {
        let stage = parse_stage(seg.trim())?;
        stages.push(stage);
    }

    Ok(Query { source, stages })
}

/// Tokenize respecting quoted strings.
fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut quote_char = '"';

    for ch in input.chars() {
        if !in_quote && (ch == '\'' || ch == '"') {
            in_quote = true;
            quote_char = ch;
            // Don't include the quote character in the token
        } else if in_quote && ch == quote_char {
            in_quote = false;
            // Push the completed quoted string
            tokens.push(current.clone());
            current.clear();
        } else if !in_quote && ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else if !in_quote && ch == ',' {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            tokens.push(",".to_string());
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Parse the source clause (everything before the first "|").
fn parse_source(input: &str) -> Result<Source> {
    let tokens = tokenize(input);
    if tokens.is_empty() {
        return Err(anyhow!("empty source clause"));
    }

    let keyword = tokens[0].to_lowercase();
    let rest = &tokens[1..];

    match keyword.as_str() {
        // "symbols" optionally "in 'file'"
        "symbols" => parse_symbols_source(rest, None),

        // Kind-specific: "structs", "functions", etc.
        "structs" | "struct" => parse_symbols_source(rest, Some(SymbolKind::Struct)),
        "functions" | "function" | "fn" => parse_symbols_source(rest, Some(SymbolKind::Function)),
        "methods" | "method" => parse_symbols_source(rest, Some(SymbolKind::Method)),
        "traits" | "trait" | "interfaces" | "interface" => parse_symbols_source(rest, Some(SymbolKind::Trait)),
        "enums" | "enum" => parse_symbols_source(rest, Some(SymbolKind::Enum)),
        "impls" | "impl" => parse_symbols_source(rest, Some(SymbolKind::Impl)),
        "consts" | "const" => parse_symbols_source(rest, Some(SymbolKind::Const)),
        "types" | "type" => parse_symbols_source(rest, Some(SymbolKind::TypeAlias)),
        "modules" | "module" | "mod" => parse_symbols_source(rest, Some(SymbolKind::Module)),

        // "symbol <name>" — specific symbol lookup
        "symbol" => {
            if rest.is_empty() {
                return Err(anyhow!("'symbol' requires a name argument"));
            }
            let name = rest[0].clone();
            let where_clause = parse_where_from_tokens(&rest[1..])?;
            Ok(Source::Symbol { name, where_clause })
        }

        // "deps [name]"
        "deps" => {
            let (name, where_start) = if !rest.is_empty() && rest[0].to_lowercase() != "where" {
                (Some(rest[0].clone()), 1)
            } else {
                (None, 0)
            };
            let where_clause = parse_where_from_tokens(&rest[where_start..])?;
            Ok(Source::Deps { name, where_clause })
        }

        // "refs [name]"
        "refs" => {
            let (name, where_start) = if !rest.is_empty() && rest[0].to_lowercase() != "where" {
                (Some(rest[0].clone()), 1)
            } else {
                (None, 0)
            };
            let where_clause = parse_where_from_tokens(&rest[where_start..])?;
            Ok(Source::Refs { name, where_clause })
        }

        _ => Err(anyhow!(
            "unknown source '{}'. Expected: symbols, structs, functions, methods, traits, \
             interfaces, enums, impls, consts, types, modules, symbol, deps, refs",
            keyword
        )),
    }
}

/// Parse a symbols source with optional "in 'file'" and "where" clauses.
fn parse_symbols_source(tokens: &[String], kind_filter: Option<SymbolKind>) -> Result<Source> {
    let mut i = 0;
    let mut in_file = None;
    let mut where_clause = Vec::new();

    // Check for "in 'file'"
    if i < tokens.len() && tokens[i].to_lowercase() == "in" {
        i += 1;
        if i >= tokens.len() {
            return Err(anyhow!("'in' requires a file path argument"));
        }
        in_file = Some(tokens[i].clone());
        i += 1;
    }

    // Check for "where" clause
    if i < tokens.len() && tokens[i].to_lowercase() == "where" {
        where_clause = parse_where_conditions(&tokens[i..])?;
    }

    Ok(Source::Symbols {
        kind_filter,
        in_file,
        where_clause,
    })
}

/// Parse "where cond1 and cond2 ..." from a token slice that starts with "where".
fn parse_where_from_tokens(tokens: &[String]) -> Result<Vec<Vec<Condition>>> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }
    if tokens[0].to_lowercase() != "where" {
        return Ok(Vec::new());
    }
    parse_where_conditions(tokens)
}

/// Parse conditions from tokens starting at "where".
/// Returns DNF: Vec of AND-groups, OR'd together.
fn parse_where_conditions(tokens: &[String]) -> Result<Vec<Vec<Condition>>> {
    if tokens.is_empty() || tokens[0].to_lowercase() != "where" {
        return Err(anyhow!("expected 'where'"));
    }

    let mut or_groups: Vec<Vec<Condition>> = Vec::new();
    let mut current_group: Vec<Condition> = Vec::new();
    let mut i = 1; // skip "where"

    loop {
        if i >= tokens.len() {
            break;
        }

        // Skip commas
        if tokens[i] == "," {
            i += 1;
            continue;
        }

        // field op value
        if i + 2 >= tokens.len() {
            return Err(anyhow!(
                "incomplete condition at '{}': expected field op value",
                tokens[i]
            ));
        }

        let field = tokens[i].to_lowercase();
        let op = parse_op(&tokens[i + 1])?;
        let value = parse_value(&tokens[i + 2]);
        i += 3;

        current_group.push(Condition { field, op, value });

        // Check for "and" or "or" connector
        if i < tokens.len() {
            match tokens[i].to_lowercase().as_str() {
                "and" => {
                    i += 1;
                    // continue adding to current AND group
                }
                "or" => {
                    i += 1;
                    // finish current AND group, start new one
                    or_groups.push(current_group);
                    current_group = Vec::new();
                }
                _ => {}
            }
        }
    }

    // Push the last group
    if !current_group.is_empty() {
        or_groups.push(current_group);
    }

    Ok(or_groups)
}

/// Parse a pipeline stage.
fn parse_stage(input: &str) -> Result<Stage> {
    let tokens = tokenize(input);
    if tokens.is_empty() {
        return Err(anyhow!("empty pipeline stage"));
    }

    let keyword = tokens[0].to_lowercase();
    let rest = &tokens[1..];

    match keyword.as_str() {
        "with" => parse_with_stage(rest),
        "show" => parse_show_stage(rest),
        "where" => {
            let conditions = parse_where_conditions(&tokens)?;
            Ok(Stage::Where { conditions })
        }
        "sort" => parse_sort_stage(rest),
        "limit" => parse_limit_stage(rest),
        _ => Err(anyhow!(
            "unknown stage '{}'. Expected: with, show, where, sort, limit",
            keyword
        )),
    }
}

/// Parse "with fields, deps, refs, ..."
fn parse_with_stage(tokens: &[String]) -> Result<Stage> {
    let mut enrichments = Vec::new();

    for tok in tokens {
        if tok == "," {
            continue;
        }
        let enrichment = match tok.to_lowercase().as_str() {
            "fields" => Enrichment::Fields,
            "params" => Enrichment::Params,
            "deps" | "dependencies" => Enrichment::Deps,
            "refs" | "references" => Enrichment::Refs,
            "signature" | "sig" => Enrichment::Signature,
            _ => return Err(anyhow!(
                "unknown enrichment '{}'. Expected: fields, params, deps, refs, signature",
                tok
            )),
        };
        enrichments.push(enrichment);
    }

    if enrichments.is_empty() {
        return Err(anyhow!("'with' requires at least one enrichment"));
    }

    Ok(Stage::With { enrichments })
}

/// Parse "show col1, col2, ..."
fn parse_show_stage(tokens: &[String]) -> Result<Stage> {
    let columns: Vec<String> = tokens
        .iter()
        .filter(|t| t.as_str() != ",")
        .map(|t| t.to_lowercase())
        .collect();

    if columns.is_empty() {
        return Err(anyhow!("'show' requires at least one column"));
    }

    Ok(Stage::Show { columns })
}

/// Parse "sort field [asc|desc]"
fn parse_sort_stage(tokens: &[String]) -> Result<Stage> {
    if tokens.is_empty() {
        return Err(anyhow!("'sort' requires a field name"));
    }

    let field = tokens[0].to_lowercase();
    let descending = if tokens.len() > 1 {
        matches!(tokens[1].to_lowercase().as_str(), "desc" | "descending")
    } else {
        false
    };

    Ok(Stage::Sort { field, descending })
}

/// Parse "limit N"
fn parse_limit_stage(tokens: &[String]) -> Result<Stage> {
    if tokens.is_empty() {
        return Err(anyhow!("'limit' requires a number"));
    }

    let count: usize = tokens[0].parse().map_err(|_| {
        anyhow!("'limit' expected a number, got '{}'", tokens[0])
    })?;

    Ok(Stage::Limit { count })
}

/// Parse an operator string.
fn parse_op(s: &str) -> Result<Op> {
    match s {
        "=" | "==" | "is" => Ok(Op::Eq),
        "!=" | "is_not" => Ok(Op::NotEq),
        "contains" | "has" | "includes" | "~" => Ok(Op::Contains),
        ">" => Ok(Op::Gt),
        "<" => Ok(Op::Lt),
        ">=" => Ok(Op::Gte),
        "<=" => Ok(Op::Lte),
        "starts_with" | "startswith" => Ok(Op::StartsWith),
        "ends_with" | "endswith" => Ok(Op::EndsWith),
        _ => Err(anyhow!(
            "unknown operator '{}'. Expected: =, !=, contains, >, <, >=, <=, starts_with, ends_with",
            s
        )),
    }
}

/// Parse a value — number or string.
fn parse_value(s: &str) -> Value {
    if let Ok(n) = s.parse::<i64>() {
        Value::Number(n)
    } else {
        Value::String(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_structs() {
        let batch = parse("structs").unwrap();
        assert_eq!(batch.queries.len(), 1);
        let q = &batch.queries[0];
        assert_eq!(
            q.source,
            Source::Symbols {
                kind_filter: Some(SymbolKind::Struct),
                in_file: None,
                where_clause: vec![],
            }
        );
        assert!(q.stages.is_empty());
    }

    #[test]
    fn parse_functions_with_where() {
        let batch = parse("functions where name = 'run' and file contains 'commands/'").unwrap();
        let q = &batch.queries[0];
        match &q.source {
            Source::Symbols { kind_filter, where_clause, .. } => {
                assert_eq!(*kind_filter, Some(SymbolKind::Function));
                // One AND group with 2 conditions
                assert_eq!(where_clause.len(), 1);
                assert_eq!(where_clause[0].len(), 2);
                assert_eq!(where_clause[0][0].field, "name");
                assert_eq!(where_clause[0][0].op, Op::Eq);
                assert_eq!(where_clause[0][0].value, Value::String("run".to_string()));
                assert_eq!(where_clause[0][1].field, "file");
                assert_eq!(where_clause[0][1].op, Op::Contains);
                assert_eq!(where_clause[0][1].value, Value::String("commands/".to_string()));
            }
            _ => panic!("expected Symbols source"),
        }
    }

    #[test]
    fn parse_symbol_with_pipe() {
        let batch = parse("symbol SymbolKind | with deps, refs").unwrap();
        let q = &batch.queries[0];
        assert_eq!(
            q.source,
            Source::Symbol {
                name: "SymbolKind".to_string(),
                where_clause: vec![],
            }
        );
        assert_eq!(q.stages.len(), 1);
        match &q.stages[0] {
            Stage::With { enrichments } => {
                assert_eq!(enrichments.len(), 2);
                assert_eq!(enrichments[0], Enrichment::Deps);
                assert_eq!(enrichments[1], Enrichment::Refs);
            }
            _ => panic!("expected With stage"),
        }
    }

    #[test]
    fn parse_symbols_in_file() {
        let batch = parse("symbols in 'src/ir/types.rs' | with deps").unwrap();
        let q = &batch.queries[0];
        match &q.source {
            Source::Symbols { in_file, .. } => {
                assert_eq!(in_file, &Some("src/ir/types.rs".to_string()));
            }
            _ => panic!("expected Symbols source"),
        }
    }

    #[test]
    fn parse_deps_with_name() {
        let batch = parse("deps Config").unwrap();
        let q = &batch.queries[0];
        assert_eq!(
            q.source,
            Source::Deps {
                name: Some("Config".to_string()),
                where_clause: vec![],
            }
        );
    }

    #[test]
    fn parse_refs_no_name_with_where() {
        let batch = parse("refs where kind = function").unwrap();
        let q = &batch.queries[0];
        match &q.source {
            Source::Refs { name, where_clause } => {
                assert!(name.is_none());
                assert_eq!(where_clause.len(), 1); // one AND group
                assert_eq!(where_clause[0].len(), 1); // with one condition
            }
            _ => panic!("expected Refs source"),
        }
    }

    #[test]
    fn parse_batch_query() {
        let batch = parse("structs; functions where file contains 'commands/'").unwrap();
        assert_eq!(batch.queries.len(), 2);
    }

    #[test]
    fn parse_post_filter_where() {
        let batch = parse("structs | with fields | where field_count > 5").unwrap();
        let q = &batch.queries[0];
        assert_eq!(q.stages.len(), 2);
        match &q.stages[1] {
            Stage::Where { conditions } => {
                assert_eq!(conditions.len(), 1); // one AND group
                assert_eq!(conditions[0][0].field, "field_count");
                assert_eq!(conditions[0][0].op, Op::Gt);
                assert_eq!(conditions[0][0].value, Value::Number(5));
            }
            _ => panic!("expected Where stage"),
        }
    }

    #[test]
    fn parse_show_stage() {
        let batch = parse("deps Config | show from, to, dep_kind").unwrap();
        let q = &batch.queries[0];
        match &q.stages[0] {
            Stage::Show { columns } => {
                assert_eq!(columns, &["from", "to", "dep_kind"]);
            }
            _ => panic!("expected Show stage"),
        }
    }

    #[test]
    fn parse_sort_and_limit() {
        let batch = parse("structs | sort name asc | limit 10").unwrap();
        let q = &batch.queries[0];
        assert_eq!(q.stages.len(), 2);
        match &q.stages[0] {
            Stage::Sort { field, descending } => {
                assert_eq!(field, "name");
                assert!(!descending);
            }
            _ => panic!("expected Sort stage"),
        }
        match &q.stages[1] {
            Stage::Limit { count } => {
                assert_eq!(*count, 10);
            }
            _ => panic!("expected Limit stage"),
        }
    }

    #[test]
    fn parse_show_signature() {
        let batch = parse("functions | show signature").unwrap();
        let q = &batch.queries[0];
        match &q.stages[0] {
            Stage::Show { columns } => {
                assert_eq!(columns, &["signature"]);
            }
            _ => panic!("expected Show stage"),
        }
    }

    #[test]
    fn error_on_empty_query() {
        assert!(parse("").is_err());
    }

    #[test]
    fn error_on_unknown_source() {
        assert!(parse("foobar").is_err());
    }

    #[test]
    fn error_on_unknown_stage() {
        assert!(parse("structs | foobar").is_err());
    }
}
