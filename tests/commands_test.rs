use std::path::PathBuf;

use smartgrep::commands::show;
use smartgrep::index::builder;
use smartgrep::ir::types::*;

fn test_ir() -> Ir {
    let file_a = PathBuf::from("src/alpha.rs");
    let file_b = PathBuf::from("src/beta.rs");

    let symbols = vec![
        Symbol {
            name: "foo".to_string(),
            qualified_name: "crate::alpha::foo".to_string(),
            kind: "fn".to_string(),
            loc: SourceLoc { file: file_a.clone(), line: 10, col: 1 },
            visibility: Visibility::Public,
            signature: Some("pub fn foo(x: i32) -> i32".to_string()),
            parent: None,
            attributes: vec!["#[inline]".to_string()],
            fields: vec![],
            params: vec![Param { name: "x".to_string(), type_name: "i32".to_string() }],
            return_type: Some("-> i32".to_string()),
        },
        Symbol {
            name: "Bar".to_string(),
            qualified_name: "crate::alpha::Bar".to_string(),
            kind: "struct".to_string(),
            loc: SourceLoc { file: file_a.clone(), line: 20, col: 1 },
            visibility: Visibility::Public,
            signature: None,
            parent: None,
            attributes: vec!["#[derive(Debug)]".to_string(), "#[derive(Clone)]".to_string()],
            fields: vec![
                Field { name: "x".to_string(), type_name: "i32".to_string(), visibility: Visibility::Public },
                Field { name: "name".to_string(), type_name: "String".to_string(), visibility: Visibility::Private },
            ],
            params: vec![],
            return_type: None,
        },
        Symbol {
            name: "process".to_string(),
            qualified_name: "crate::alpha::Bar::process".to_string(),
            kind: "method".to_string(),
            loc: SourceLoc { file: file_a.clone(), line: 32, col: 5 },
            visibility: Visibility::Public,
            signature: Some("pub fn process(&self, count: usize)".to_string()),
            parent: Some("Bar".to_string()),
            attributes: vec![],
            fields: vec![],
            params: vec![
                Param { name: "self".to_string(), type_name: "&self".to_string() },
                Param { name: "count".to_string(), type_name: "usize".to_string() },
            ],
            return_type: None,
        },
        Symbol {
            name: "foo".to_string(),
            qualified_name: "crate::beta::foo".to_string(),
            kind: "fn".to_string(),
            loc: SourceLoc { file: file_b.clone(), line: 5, col: 1 },
            visibility: Visibility::Private,
            signature: Some("fn foo()".to_string()),
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
fn show_function_displays_all_fields() {
    let ir = test_ir();
    let index = builder::build(&ir);
    let symbols = index.by_name("foo");
    let output = show::format_text(&symbols);

    // Should contain both foo symbols
    assert!(output.contains("crate::alpha::foo"), "should contain qualified name");
    assert!(output.contains("crate::beta::foo"), "should contain second foo");
    // Separator between multiple matches
    assert!(output.contains("---"), "should separate multiple matches");
    // First foo details
    assert!(output.contains("pub fn foo(x: i32) -> i32"), "should contain signature");
    assert!(output.contains("x: i32"), "should contain param");
    assert!(output.contains("-> i32"), "should contain return type");
    assert!(output.contains("#[inline]"), "should contain attributes");
    assert!(output.contains("pub"), "should contain visibility");
    assert!(output.contains("src/alpha.rs:10"), "should contain location");
    // Second foo details
    assert!(output.contains("private"), "should contain private visibility");
    assert!(output.contains("src/beta.rs:5"), "should contain second location");
}

#[test]
fn show_struct_displays_fields() {
    let ir = test_ir();
    let index = builder::build(&ir);
    let symbols = index.by_name("Bar");
    let output = show::format_text(&symbols);

    assert!(output.contains("struct crate::alpha::Bar"), "should show kind and qualified name");
    assert!(output.contains("fields:"), "should have fields section");
    assert!(output.contains("pub x: i32"), "should show field with visibility and type");
    assert!(output.contains("private name: String"), "should show private field");
    assert!(output.contains("#[derive(Debug)]"), "should show attributes");
    assert!(output.contains("#[derive(Clone)]"), "should show all attributes");
}

#[test]
fn show_method_displays_parent() {
    let ir = test_ir();
    let index = builder::build(&ir);
    let symbols = index.by_name("process");
    let output = show::format_text(&symbols);

    assert!(output.contains("parent: Bar"), "should show parent");
    assert!(output.contains("method crate::alpha::Bar::process"), "should show kind and qualified name");
    assert!(output.contains("count: usize"), "should show non-self params");
    assert!(output.contains("self: &self"), "should include self param");
}

#[test]
fn show_no_match_returns_empty() {
    let ir = test_ir();
    let index = builder::build(&ir);
    let symbols = index.by_name("nonexistent");
    let output = show::format_text(&symbols);
    // Empty vec produces empty string
    assert!(output.is_empty());
}

#[test]
fn show_json_format() {
    let ir = test_ir();
    let index = builder::build(&ir);
    let symbols = index.by_name("Bar");

    // Test that JSON output parses correctly
    let json_output = serde_json::to_string_pretty(&symbols).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json_output).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"], "Bar");
    assert_eq!(arr[0]["kind"], "struct");
}
