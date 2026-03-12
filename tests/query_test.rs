/// Integration tests for the query DSL.
/// Tests use hand-built Index data following the project's testing patterns.

use std::path::PathBuf;

use smartgrep::ir::types::*;
use smartgrep::index::builder;
use smartgrep::query::{parser, engine};

/// Build a test IR with known symbols and dependencies.
fn test_ir() -> Ir {
    let file_a = PathBuf::from("src/alpha.rs");
    let file_b = PathBuf::from("src/beta.rs");
    let file_c = PathBuf::from("src/commands/run.rs");

    let symbols = vec![
        Symbol {
            name: "foo".to_string(),
            qualified_name: "crate::alpha::foo".to_string(),
            kind: SymbolKind::Function,
            loc: SourceLoc { file: file_a.clone(), line: 10, col: 1 },
            visibility: Visibility::Public,
            signature: Some("pub fn foo(x: i32) -> i32".to_string()),
            parent: None,
            attributes: vec![],
            fields: vec![],
            params: vec![Param { name: "x".to_string(), type_name: "i32".to_string() }],
            return_type: Some("-> i32".to_string()),
        },
        Symbol {
            name: "Bar".to_string(),
            qualified_name: "crate::alpha::Bar".to_string(),
            kind: SymbolKind::Struct,
            loc: SourceLoc { file: file_a.clone(), line: 20, col: 1 },
            visibility: Visibility::Public,
            signature: None,
            parent: None,
            attributes: vec!["#[derive(Debug)]".to_string()],
            fields: vec![
                Field { name: "x".to_string(), type_name: "i32".to_string(), visibility: Visibility::Public },
                Field { name: "y".to_string(), type_name: "String".to_string(), visibility: Visibility::Private },
            ],
            params: vec![],
            return_type: None,
        },
        Symbol {
            name: "foo".to_string(),
            qualified_name: "crate::beta::foo".to_string(),
            kind: SymbolKind::Function,
            loc: SourceLoc { file: file_b.clone(), line: 5, col: 1 },
            visibility: Visibility::Private,
            signature: Some("fn foo()".to_string()),
            parent: None,
            attributes: vec![],
            fields: vec![],
            params: vec![],
            return_type: None,
        },
        Symbol {
            name: "Baz".to_string(),
            qualified_name: "crate::beta::Baz".to_string(),
            kind: SymbolKind::Trait,
            loc: SourceLoc { file: file_b.clone(), line: 15, col: 1 },
            visibility: Visibility::Public,
            signature: None,
            parent: None,
            attributes: vec![],
            fields: vec![],
            params: vec![],
            return_type: None,
        },
        Symbol {
            name: "process".to_string(),
            qualified_name: "crate::alpha::Bar::process".to_string(),
            kind: SymbolKind::Method,
            loc: SourceLoc { file: file_a.clone(), line: 32, col: 5 },
            visibility: Visibility::Public,
            signature: Some("pub fn process(&self)".to_string()),
            parent: Some("Bar".to_string()),
            attributes: vec![],
            fields: vec![],
            params: vec![Param { name: "self".to_string(), type_name: "&self".to_string() }],
            return_type: None,
        },
        Symbol {
            name: "run".to_string(),
            qualified_name: "crate::commands::run::run".to_string(),
            kind: SymbolKind::Function,
            loc: SourceLoc { file: file_c.clone(), line: 1, col: 1 },
            visibility: Visibility::Public,
            signature: Some("pub fn run() -> Result<()>".to_string()),
            parent: None,
            attributes: vec![],
            fields: vec![],
            params: vec![],
            return_type: Some("-> Result<()>".to_string()),
        },
        Symbol {
            name: "BigStruct".to_string(),
            qualified_name: "crate::alpha::BigStruct".to_string(),
            kind: SymbolKind::Struct,
            loc: SourceLoc { file: file_a.clone(), line: 50, col: 1 },
            visibility: Visibility::Public,
            signature: None,
            parent: None,
            attributes: vec![],
            fields: vec![
                Field { name: "a".to_string(), type_name: "i32".to_string(), visibility: Visibility::Public },
                Field { name: "b".to_string(), type_name: "i32".to_string(), visibility: Visibility::Public },
                Field { name: "c".to_string(), type_name: "i32".to_string(), visibility: Visibility::Public },
                Field { name: "d".to_string(), type_name: "i32".to_string(), visibility: Visibility::Public },
                Field { name: "e".to_string(), type_name: "i32".to_string(), visibility: Visibility::Public },
                Field { name: "f".to_string(), type_name: "i32".to_string(), visibility: Visibility::Public },
            ],
            params: vec![],
            return_type: None,
        },
    ];

    let dependencies = vec![
        Dependency {
            from_qualified: "crate::beta::foo".to_string(),
            to_name: "Bar".to_string(),
            kind: DepKind::TypeReference,
            loc: SourceLoc { file: file_b.clone(), line: 6, col: 10 },
        },
        Dependency {
            from_qualified: "crate::alpha::Bar".to_string(),
            to_name: "Baz".to_string(),
            kind: DepKind::TraitImpl,
            loc: SourceLoc { file: file_a.clone(), line: 30, col: 1 },
        },
        Dependency {
            from_qualified: "crate::alpha".to_string(),
            to_name: "std::collections::HashMap".to_string(),
            kind: DepKind::Import,
            loc: SourceLoc { file: file_a.clone(), line: 1, col: 1 },
        },
        Dependency {
            from_qualified: "crate::commands::run::run".to_string(),
            to_name: "Bar".to_string(),
            kind: DepKind::FunctionCall,
            loc: SourceLoc { file: file_c.clone(), line: 3, col: 5 },
        },
    ];

    Ir { symbols, dependencies }
}

fn build_test_index() -> smartgrep::index::types::Index {
    let ir = test_ir();
    builder::build(&ir)
}

fn run_query(query_str: &str) -> Vec<engine::Row> {
    let index = build_test_index();
    let batch = parser::parse(query_str).unwrap();
    assert_eq!(batch.queries.len(), 1, "expected single query");
    engine::execute_query(&batch.queries[0], &index).unwrap()
}

// --- Source clause tests ---

#[test]
fn query_all_structs() {
    let rows = run_query("structs");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.get("kind").unwrap() == "struct"));
}

#[test]
fn query_all_functions() {
    let rows = run_query("functions");
    // foo (alpha), foo (beta), run (commands)
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|r| r.get("kind").unwrap() == "fn"));
}

#[test]
fn query_all_symbols() {
    let rows = run_query("symbols");
    assert_eq!(rows.len(), 7);
}

#[test]
fn query_symbol_by_name() {
    let rows = run_query("symbol Bar");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("name").unwrap(), "Bar");
    assert_eq!(rows[0].get("kind").unwrap(), "struct");
}

#[test]
fn query_symbol_by_name_duplicate() {
    let rows = run_query("symbol foo");
    assert_eq!(rows.len(), 2);
}

#[test]
fn query_symbols_in_file() {
    let rows = run_query("symbols in 'src/beta.rs'");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.get("file").unwrap().contains("beta")));
}

#[test]
fn query_traits() {
    let rows = run_query("traits");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("name").unwrap(), "Baz");
}

#[test]
fn query_methods() {
    let rows = run_query("methods");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("name").unwrap(), "process");
    assert_eq!(rows[0].get("parent").unwrap(), "Bar");
}

// --- Where clause tests ---

#[test]
fn query_where_name_eq() {
    let rows = run_query("functions where name = 'foo'");
    assert_eq!(rows.len(), 2);
}

#[test]
fn query_where_visibility_eq() {
    let rows = run_query("symbols where visibility = public");
    // All except beta::foo (private)
    assert!(rows.iter().all(|r| r.get("visibility").unwrap() == "public"));
}

#[test]
fn query_where_file_contains() {
    let rows = run_query("functions where file contains 'commands/'");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("name").unwrap(), "run");
}

#[test]
fn query_where_combined() {
    let rows = run_query("functions where name = 'run' and file contains 'commands/'");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("qualified_name").unwrap(), "crate::commands::run::run");
}

#[test]
fn query_where_not_eq() {
    let rows = run_query("functions where visibility != public");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("qualified_name").unwrap(), "crate::beta::foo");
}

// --- Enrichment tests ---

#[test]
fn query_with_fields() {
    let rows = run_query("symbol Bar | with fields");
    assert_eq!(rows.len(), 1);
    let fields = rows[0].get("fields").unwrap();
    assert!(fields.contains("x: i32"));
    assert!(fields.contains("y: String"));
}

#[test]
fn query_with_deps() {
    let rows = run_query("symbol Bar | with deps");
    assert_eq!(rows.len(), 1);
    let deps = rows[0].get("deps");
    assert!(deps.is_some());
    assert!(deps.unwrap().contains("Baz"));
}

#[test]
fn query_with_refs() {
    let rows = run_query("symbol Bar | with refs");
    assert_eq!(rows.len(), 1);
    let refs = rows[0].get("refs");
    assert!(refs.is_some());
    // Bar is referenced by beta::foo and commands::run
    assert!(refs.unwrap().contains("crate::beta::foo"));
}

#[test]
fn query_with_signature() {
    let rows = run_query("symbol foo | with signature");
    assert_eq!(rows.len(), 2);
    // alpha::foo has a signature
    let alpha_foo = rows.iter().find(|r| r.get("qualified_name").unwrap().contains("alpha")).unwrap();
    assert_eq!(alpha_foo.get("signature").unwrap(), "pub fn foo(x: i32) -> i32");
}

#[test]
fn query_with_multiple_enrichments() {
    let rows = run_query("symbol Bar | with fields, deps, refs");
    assert_eq!(rows.len(), 1);
    assert!(rows[0].get("fields").is_some());
    assert!(rows[0].get("deps").is_some());
    assert!(rows[0].get("refs").is_some());
}

// --- Pipeline stage tests ---

#[test]
fn query_show_columns() {
    let rows = run_query("structs | show name, file");
    assert_eq!(rows.len(), 2);
    // Should only have name and file columns
    for row in &rows {
        assert!(row.get("name").is_some());
        assert!(row.get("file").is_some());
        assert!(row.get("kind").is_none()); // filtered out by show
    }
}

#[test]
fn query_sort_by_name() {
    let rows = run_query("symbols | sort name");
    let names: Vec<&str> = rows.iter().map(|r| r.get("name").unwrap().as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
}

#[test]
fn query_sort_desc() {
    let rows = run_query("structs | sort name desc");
    let names: Vec<&str> = rows.iter().map(|r| r.get("name").unwrap().as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    sorted.reverse();
    assert_eq!(names, sorted);
}

#[test]
fn query_limit() {
    let rows = run_query("symbols | limit 3");
    assert_eq!(rows.len(), 3);
}

#[test]
fn query_sort_then_limit() {
    let rows = run_query("symbols | sort name | limit 2");
    assert_eq!(rows.len(), 2);
    // First two alphabetically
    assert_eq!(rows[0].get("name").unwrap(), "Bar");
    assert_eq!(rows[1].get("name").unwrap(), "Baz");
}

#[test]
fn query_post_filter_field_count() {
    let rows = run_query("structs | with fields | where field_count > 5");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("name").unwrap(), "BigStruct");
}

#[test]
fn query_post_filter_field_count_gte() {
    let rows = run_query("structs | with fields | where field_count >= 2");
    // Bar has 2 fields, BigStruct has 6
    assert_eq!(rows.len(), 2);
}

// --- Deps/Refs source tests ---

#[test]
fn query_deps_of_symbol() {
    let rows = run_query("deps Bar");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("to").unwrap(), "Baz");
    assert_eq!(rows[0].get("kind").unwrap(), "trait_impl");
}

#[test]
fn query_all_deps() {
    let rows = run_query("deps");
    assert_eq!(rows.len(), 4);
}

#[test]
fn query_deps_with_where() {
    let rows = run_query("deps where kind = type_ref");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("from").unwrap(), "crate::beta::foo");
}

#[test]
fn query_refs_to_symbol() {
    let rows = run_query("refs Bar");
    // Two refs to "Bar": from beta::foo (TypeReference) and from commands::run (FunctionCall)
    assert_eq!(rows.len(), 2);
}

#[test]
fn query_refs_with_where() {
    let rows = run_query("refs Bar | where kind = call");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("from").unwrap(), "crate::commands::run::run");
}

#[test]
fn query_deps_show_columns() {
    let rows = run_query("deps Bar | show from, to, kind");
    assert_eq!(rows.len(), 1);
    assert!(rows[0].get("from").is_some());
    assert!(rows[0].get("to").is_some());
    assert!(rows[0].get("kind").is_some());
    assert!(rows[0].get("file").is_none());
}

// --- Batch query tests ---

#[test]
fn query_batch() {
    let index = build_test_index();
    let batch = parser::parse("structs; traits").unwrap();
    assert_eq!(batch.queries.len(), 2);
    let output = engine::execute_batch(&batch, &index, "text").unwrap();
    assert!(output.contains("# Query 1"));
    assert!(output.contains("# Query 2"));
    assert!(output.contains("Bar"));
    assert!(output.contains("Baz"));
}

// --- Complex composed queries ---

#[test]
fn query_public_structs_in_alpha_with_fields() {
    let rows = run_query("structs where file contains 'alpha' and visibility = public | with fields");
    assert_eq!(rows.len(), 2); // Bar and BigStruct
    for row in &rows {
        assert!(row.get("fields").is_some());
    }
}

#[test]
fn query_functions_named_run_with_signature() {
    let rows = run_query("functions where name = 'run' | with signature");
    assert_eq!(rows.len(), 1);
    let sig = rows[0].get("signature").unwrap();
    assert!(sig.contains("pub fn run"));
}

#[test]
fn query_complex_pipeline() {
    // Find all functions, enrich with signature, sort by name, limit to 2
    let rows = run_query("functions | with signature | sort name | limit 2");
    assert_eq!(rows.len(), 2);
    // Alphabetically: foo, foo, run → first two are both "foo"
    assert_eq!(rows[0].get("name").unwrap(), "foo");
    assert_eq!(rows[1].get("name").unwrap(), "foo");
}

// --- JSON output test ---

#[test]
fn query_json_output() {
    let index = build_test_index();
    let batch = parser::parse("symbol Bar").unwrap();
    let output = engine::execute_batch(&batch, &index, "json").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"], "Bar");
    assert_eq!(arr[0]["kind"], "struct");
}

// --- Text output formatting test ---

#[test]
fn query_text_output_no_results() {
    let index = build_test_index();
    let batch = parser::parse("structs where name = 'NonExistent'").unwrap();
    let output = engine::execute_batch(&batch, &index, "text").unwrap();
    assert_eq!(output, "No results.");
}

// --- Error handling tests ---

#[test]
fn query_parse_error_empty() {
    assert!(parser::parse("").is_err());
}

#[test]
fn query_parse_error_unknown_source() {
    assert!(parser::parse("foobar").is_err());
}

#[test]
fn query_parse_error_unknown_stage() {
    assert!(parser::parse("structs | foobar").is_err());
}

#[test]
fn query_parse_error_bad_operator() {
    assert!(parser::parse("structs where name like 'foo'").is_err());
}

#[test]
fn query_parse_error_incomplete_condition() {
    assert!(parser::parse("structs where name").is_err());
}

// --- OR support tests ---

#[test]
fn query_simple_or() {
    // "foo" is a function (2 matches), "Bar" is a struct (1 match)
    let rows = run_query("symbols where name = 'foo' or name = 'Bar'");
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|r| {
        let name = r.get("name").unwrap();
        name == "foo" || name == "Bar"
    }));
}

#[test]
fn query_mixed_and_or() {
    // (name = 'foo' AND visibility = public) OR (name = 'run')
    // public foo = alpha::foo, run = commands::run
    let rows = run_query("functions where name = 'foo' and visibility = public or name = 'run'");
    assert_eq!(rows.len(), 2);
    let names: Vec<&str> = rows.iter().map(|r| r.get("name").unwrap().as_str()).collect();
    assert!(names.contains(&"foo"));
    assert!(names.contains(&"run"));
}

#[test]
fn query_multiple_or() {
    // name = 'foo' OR name = 'Bar' OR name = 'Baz'
    let rows = run_query("symbols where name = 'foo' or name = 'Bar' or name = 'Baz'");
    assert_eq!(rows.len(), 4); // 2 foo + 1 Bar + 1 Baz
}

#[test]
fn query_or_in_pipeline_stage() {
    // Use OR in a post-filter where stage
    let rows = run_query("symbols | where name = 'Bar' or name = 'Baz'");
    assert_eq!(rows.len(), 2);
    let names: Vec<&str> = rows.iter().map(|r| r.get("name").unwrap().as_str()).collect();
    assert!(names.contains(&"Bar"));
    assert!(names.contains(&"Baz"));
}

#[test]
fn query_or_backward_compat_and_only() {
    // Existing and-only queries still work
    let rows = run_query("functions where name = 'run' and file contains 'commands/'");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("name").unwrap(), "run");
}

#[test]
fn query_or_deps_source() {
    // OR in deps where clause
    let rows = run_query("deps where kind = type_ref or kind = call");
    assert_eq!(rows.len(), 2);
}

#[test]
fn query_or_with_contains() {
    // Simulates the motivating use case: matching multiple attribute patterns
    let rows = run_query("structs where name contains 'Bar' or name contains 'Big'");
    assert_eq!(rows.len(), 2);
    let names: Vec<&str> = rows.iter().map(|r| r.get("name").unwrap().as_str()).collect();
    assert!(names.contains(&"Bar"));
    assert!(names.contains(&"BigStruct"));
}

// --- Path alias tests ---

// --- Attribute filtering tests ---

fn run_java_query(query_str: &str) -> Vec<engine::Row> {
    let ir = java_test_ir();
    let index = builder::build(&ir);
    let batch = parser::parse(query_str).unwrap();
    assert_eq!(batch.queries.len(), 1, "expected single query");
    engine::execute_query(&batch.queries[0], &index).unwrap()
}

#[test]
fn query_where_attributes_contains() {
    let rows = run_java_query("methods where attributes contains '@PostMapping'");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("name").unwrap(), "listProducts");
}

#[test]
fn query_where_attributes_contains_no_match() {
    let rows = run_java_query("methods where attributes contains '@DeleteMapping'");
    assert_eq!(rows.len(), 0);
}

#[test]
fn query_where_attributes_contains_partial() {
    // Should match because contains checks substrings
    let rows = run_java_query("symbols where attributes contains '@Timed'");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("name").unwrap(), "listProducts");
}

// --- Path alias tests (continued) ---

/// Build a test IR with long Java-like paths to exercise path aliasing.
fn java_test_ir() -> Ir {
    let file_a = PathBuf::from("src/main/java/com/example/ecommerce/catalog/controller/ProductController.java");
    let file_b = PathBuf::from("src/main/java/com/example/ecommerce/catalog/service/InventoryService.java");

    let symbols = vec![
        Symbol {
            name: "ProductController".to_string(),
            qualified_name: "com.example.ecommerce.catalog.controller.ProductController".to_string(),
            kind: SymbolKind::Struct,
            loc: SourceLoc { file: file_a.clone(), line: 5, col: 1 },
            visibility: Visibility::Public,
            signature: None,
            parent: None,
            attributes: vec![],
            fields: vec![],
            params: vec![],
            return_type: None,
        },
        Symbol {
            name: "listProducts".to_string(),
            qualified_name: "com.example.ecommerce.catalog.controller.ProductController.listProducts".to_string(),
            kind: SymbolKind::Method,
            loc: SourceLoc { file: file_a.clone(), line: 12, col: 5 },
            visibility: Visibility::Public,
            signature: None,
            parent: Some("ProductController".to_string()),
            attributes: vec!["@PostMapping(\"/execute-commands\")".to_string(), "@Timed".to_string()],
            fields: vec![],
            params: vec![],
            return_type: None,
        },
        Symbol {
            name: "InventoryService".to_string(),
            qualified_name: "com.example.ecommerce.catalog.service.InventoryService".to_string(),
            kind: SymbolKind::Struct,
            loc: SourceLoc { file: file_b.clone(), line: 3, col: 1 },
            visibility: Visibility::Public,
            signature: None,
            parent: None,
            attributes: vec![],
            fields: vec![],
            params: vec![],
            return_type: None,
        },
    ];

    Ir { symbols, dependencies: vec![] }
}

#[test]
fn query_text_output_has_path_alias_header() {
    let ir = java_test_ir();
    let index = builder::build(&ir);
    let batch = parser::parse("symbols").unwrap();
    let output = engine::execute_batch(&batch, &index, "text").unwrap();
    // Should contain the alias header
    assert!(output.contains("[paths]"), "output should contain [paths] header:\n{}", output);
    assert!(output.contains("[P]"), "output should contain [P] alias:\n{}", output);
    assert!(output.contains("src/main/java/com/example/ecommerce/catalog/"),
        "alias header should contain the common prefix:\n{}", output);
}

#[test]
fn query_text_output_uses_short_paths() {
    let ir = java_test_ir();
    let index = builder::build(&ir);
    let batch = parser::parse("symbols").unwrap();
    let output = engine::execute_batch(&batch, &index, "text").unwrap();
    // Result lines should use [P] prefix, not full path
    // In query output, file and line are separate columns
    assert!(output.contains("[P]controller/ProductController.java"),
        "should use shortened path for controller:\n{}", output);
    assert!(output.contains("[P]service/InventoryService.java"),
        "should use shortened path for service:\n{}", output);
    // Full paths should NOT appear in result lines (only in the alias header)
    let lines: Vec<&str> = output.lines().collect();
    // Skip the header line and blank line, check data lines don't have the full prefix
    for line in lines.iter().skip(2) {
        if !line.is_empty() {
            assert!(!line.contains("src/main/java/com/example/ecommerce/catalog/controller/"),
                "data line should not contain full path:\n{}", line);
        }
    }
}

#[test]
fn query_json_output_keeps_full_paths() {
    let ir = java_test_ir();
    let index = builder::build(&ir);
    let batch = parser::parse("symbols").unwrap();
    let output = engine::execute_batch(&batch, &index, "json").unwrap();
    // JSON should keep full paths, no [P] alias
    assert!(!output.contains("[P]"), "JSON output should not contain [P]:\n{}", output);
    assert!(output.contains("src/main/java/com/example/ecommerce/catalog/controller/ProductController.java"),
        "JSON should have full path:\n{}", output);
}

#[test]
fn query_text_output_no_alias_for_short_paths() {
    // The default test_ir uses short paths like "src/alpha.rs" — no alias should be generated
    let index = build_test_index();
    let batch = parser::parse("symbols").unwrap();
    let output = engine::execute_batch(&batch, &index, "text").unwrap();
    assert!(!output.contains("[paths]"), "short paths should not trigger alias:\n{}", output);
    assert!(!output.contains("[P]"), "short paths should not use [P]:\n{}", output);
}
