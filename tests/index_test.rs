use std::path::PathBuf;

use smartgrep::ir::types::*;
use smartgrep::index::builder;
use smartgrep::index::types::Index;

/// Helper to build a test IR with known symbols and dependencies.
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
            attributes: vec![],
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
            attributes: vec![],
            fields: vec![
                Field { name: "x".to_string(), type_name: "i32".to_string(), visibility: Visibility::Public },
            ],
            params: vec![],
            return_type: None,
        },
        Symbol {
            name: "foo".to_string(), // duplicate name, different qualified
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
        Symbol {
            name: "Baz".to_string(),
            qualified_name: "crate::beta::Baz".to_string(),
            kind: "trait".to_string(),
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
            name: "impl Baz for Bar".to_string(),
            qualified_name: "crate::alpha::Bar".to_string(), // shares qualified with struct
            kind: "impl".to_string(),
            loc: SourceLoc { file: file_a.clone(), line: 30, col: 1 },
            visibility: Visibility::Private,
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
            kind: "method".to_string(),
            loc: SourceLoc { file: file_a.clone(), line: 32, col: 5 },
            visibility: Visibility::Public,
            signature: Some("pub fn process(&self)".to_string()),
            parent: Some("Bar".to_string()),
            attributes: vec![],
            fields: vec![],
            params: vec![Param { name: "self".to_string(), type_name: "&self".to_string() }],
            return_type: None,
        },
    ];

    let dependencies = vec![
        Dependency {
            from_qualified: "crate::beta::foo".to_string(),
            to_name: "crate::alpha::Bar".to_string(),
            kind: DepKind::TypeRef,
            loc: SourceLoc { file: file_b.clone(), line: 6, col: 10 },
        },
        Dependency {
            from_qualified: "crate::alpha::Bar".to_string(),
            to_name: "Baz".to_string(),
            kind: DepKind::Implements,
            loc: SourceLoc { file: file_a.clone(), line: 30, col: 1 },
        },
        Dependency {
            from_qualified: "crate::alpha".to_string(),
            to_name: "std::collections::HashMap".to_string(),
            kind: DepKind::Import,
            loc: SourceLoc { file: file_a.clone(), line: 1, col: 1 },
        },
    ];

    Ir { symbols, dependencies }
}

fn build_test_index() -> Index {
    let ir = test_ir();
    builder::build(&ir)
}

#[test]
fn by_name_returns_matching_symbols() {
    let index = build_test_index();
    let foos = index.by_name("foo");
    assert_eq!(foos.len(), 2, "two symbols named 'foo'");
    assert!(foos.iter().all(|s| s.name == "foo"));
}

#[test]
fn by_name_single_result() {
    let index = build_test_index();
    let bars = index.by_name("Bar");
    assert_eq!(bars.len(), 1);
    assert_eq!(bars[0].kind, "struct");
}

#[test]
fn by_name_no_match_returns_empty() {
    let index = build_test_index();
    let result = index.by_name("nonexistent");
    assert!(result.is_empty());
}

#[test]
fn by_file_returns_symbols_in_file() {
    let index = build_test_index();
    let file_a = PathBuf::from("src/alpha.rs");
    let syms = index.by_file(&file_a);
    // foo, Bar, impl Baz for Bar, process
    assert_eq!(syms.len(), 4);
}

#[test]
fn by_file_other_file() {
    let index = build_test_index();
    let file_b = PathBuf::from("src/beta.rs");
    let syms = index.by_file(&file_b);
    // foo, Baz
    assert_eq!(syms.len(), 2);
}

#[test]
fn by_qualified_finds_unique_symbol() {
    let index = build_test_index();
    let sym = index.by_qualified("crate::beta::Baz");
    assert!(sym.is_some());
    assert_eq!(sym.unwrap().name, "Baz");
    assert_eq!(sym.unwrap().kind, "trait");
}

#[test]
fn by_qualified_no_match() {
    let index = build_test_index();
    assert!(index.by_qualified("crate::gamma::Quux").is_none());
}

#[test]
fn by_kind_functions() {
    let index = build_test_index();
    let fns = index.by_kind("fn");
    assert_eq!(fns.len(), 2);
    assert!(fns.iter().all(|s| s.kind == "fn"));
}

#[test]
fn by_kind_structs() {
    let index = build_test_index();
    let structs = index.by_kind("struct");
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "Bar");
}

#[test]
fn by_kind_methods() {
    let index = build_test_index();
    let methods = index.by_kind("method");
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "process");
}

#[test]
fn by_kind_traits() {
    let index = build_test_index();
    let traits = index.by_kind("trait");
    assert_eq!(traits.len(), 1);
    assert_eq!(traits[0].name, "Baz");
}

#[test]
fn by_kind_impls() {
    let index = build_test_index();
    let impls = index.by_kind("impl");
    assert_eq!(impls.len(), 1);
}

#[test]
fn deps_of_returns_outgoing_deps() {
    let index = build_test_index();
    let deps = index.deps_of("crate::alpha::Bar");
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].to_name, "Baz");
    assert_eq!(deps[0].kind, DepKind::Implements);
}

#[test]
fn deps_of_no_match() {
    let index = build_test_index();
    let deps = index.deps_of("crate::nonexistent");
    assert!(deps.is_empty());
}

#[test]
fn refs_to_returns_incoming_deps() {
    let index = build_test_index();
    let refs = index.refs_to("Baz");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].from_qualified, "crate::alpha::Bar");
}

#[test]
fn refs_to_type_reference() {
    let index = build_test_index();
    let refs = index.refs_to("crate::alpha::Bar");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].kind, DepKind::TypeRef);
}

#[test]
fn reverse_deps_import() {
    let index = build_test_index();
    let refs = index.refs_to("std::collections::HashMap");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].kind, DepKind::Import);
}

#[test]
fn duplicate_names_return_multiple_results() {
    let index = build_test_index();
    let foos = index.by_name("foo");
    assert_eq!(foos.len(), 2);
    // They should have different qualified names
    let qnames: Vec<&str> = foos.iter().map(|s| s.qualified_name.as_str()).collect();
    assert!(qnames.contains(&"crate::alpha::foo"));
    assert!(qnames.contains(&"crate::beta::foo"));
}

#[test]
fn index_has_correct_symbol_count() {
    let index = build_test_index();
    assert_eq!(index.symbols.len(), 6);
}

#[test]
fn index_has_correct_dep_count() {
    let index = build_test_index();
    assert_eq!(index.deps.len(), 3);
}
