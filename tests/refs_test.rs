use std::path::PathBuf;

use smartgrep::ir::types::*;
use smartgrep::index::builder;

/// Build a test IR with known reverse dependencies for testing refs.
fn refs_test_ir() -> Ir {
    let file_a = PathBuf::from("src/alpha.rs");
    let file_b = PathBuf::from("src/beta.rs");
    let file_c = PathBuf::from("src/gamma.rs");

    let symbols = vec![
        Symbol {
            name: "Config".to_string(),
            qualified_name: "crate::alpha::Config".to_string(),
            kind: SymbolKind::Struct,
            loc: SourceLoc { file: file_a.clone(), line: 5, col: 1 },
            visibility: Visibility::Public,
            signature: None,
            parent: None,
            attributes: vec![],
            fields: vec![
                Field { name: "host".to_string(), type_name: "String".to_string(), visibility: Visibility::Public },
            ],
            params: vec![],
            return_type: None,
        },
        Symbol {
            name: "run".to_string(),
            qualified_name: "crate::beta::run".to_string(),
            kind: SymbolKind::Function,
            loc: SourceLoc { file: file_b.clone(), line: 10, col: 1 },
            visibility: Visibility::Public,
            signature: Some("pub fn run(cfg: &Config)".to_string()),
            parent: None,
            attributes: vec![],
            fields: vec![],
            params: vec![Param { name: "cfg".to_string(), type_name: "&Config".to_string() }],
            return_type: None,
        },
        Symbol {
            name: "init".to_string(),
            qualified_name: "crate::gamma::init".to_string(),
            kind: SymbolKind::Function,
            loc: SourceLoc { file: file_c.clone(), line: 3, col: 1 },
            visibility: Visibility::Public,
            signature: Some("pub fn init() -> Config".to_string()),
            parent: None,
            attributes: vec![],
            fields: vec![],
            params: vec![],
            return_type: Some("-> Config".to_string()),
        },
    ];

    let dependencies = vec![
        // beta::run references Config as a type
        Dependency {
            from_qualified: "crate::beta::run".to_string(),
            to_name: "Config".to_string(),
            kind: DepKind::TypeReference,
            loc: SourceLoc { file: file_b.clone(), line: 10, col: 20 },
        },
        // gamma::init also references Config as a type
        Dependency {
            from_qualified: "crate::gamma::init".to_string(),
            to_name: "Config".to_string(),
            kind: DepKind::TypeReference,
            loc: SourceLoc { file: file_c.clone(), line: 3, col: 30 },
        },
        // beta::run imports alpha::Config
        Dependency {
            from_qualified: "crate::beta".to_string(),
            to_name: "crate::alpha::Config".to_string(),
            kind: DepKind::Import,
            loc: SourceLoc { file: file_b.clone(), line: 1, col: 1 },
        },
        // gamma::init calls run
        Dependency {
            from_qualified: "crate::gamma::init".to_string(),
            to_name: "run".to_string(),
            kind: DepKind::FunctionCall,
            loc: SourceLoc { file: file_c.clone(), line: 5, col: 5 },
        },
    ];

    Ir { symbols, dependencies }
}

fn build_refs_test_index() -> smartgrep::index::types::Index {
    let ir = refs_test_ir();
    builder::build(&ir)
}

#[test]
fn refs_to_returns_all_references() {
    let index = build_refs_test_index();
    let refs = index.refs_to("Config");
    assert_eq!(refs.len(), 2, "Config should have 2 type references");
    assert!(refs.iter().all(|d| d.to_name == "Config"));
}

#[test]
fn refs_to_returns_correct_referrers() {
    let index = build_refs_test_index();
    let refs = index.refs_to("Config");
    let from_names: Vec<&str> = refs.iter().map(|d| d.from_qualified.as_str()).collect();
    assert!(from_names.contains(&"crate::beta::run"));
    assert!(from_names.contains(&"crate::gamma::init"));
}

#[test]
fn refs_to_qualified_name() {
    let index = build_refs_test_index();
    let refs = index.refs_to("crate::alpha::Config");
    assert_eq!(refs.len(), 1, "qualified Config should have 1 import reference");
    assert_eq!(refs[0].kind, DepKind::Import);
    assert_eq!(refs[0].from_qualified, "crate::beta");
}

#[test]
fn refs_to_function_call() {
    let index = build_refs_test_index();
    let refs = index.refs_to("run");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].kind, DepKind::FunctionCall);
    assert_eq!(refs[0].from_qualified, "crate::gamma::init");
}

#[test]
fn refs_to_nonexistent_returns_empty() {
    let index = build_refs_test_index();
    let refs = index.refs_to("Nonexistent");
    assert!(refs.is_empty());
}

#[test]
fn refs_to_dep_kinds_are_correct() {
    let index = build_refs_test_index();
    let refs = index.refs_to("Config");
    assert!(refs.iter().all(|d| d.kind == DepKind::TypeReference));
}

#[test]
fn refs_to_locations_are_correct() {
    let index = build_refs_test_index();
    let refs = index.refs_to("run");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].loc.file, PathBuf::from("src/gamma.rs"));
    assert_eq!(refs[0].loc.line, 5);
}
