use std::path::PathBuf;

use smartgrep::ir::types::*;
use smartgrep::index::builder;
use smartgrep::commands::deps;

/// Build a test IR with known symbols and dependencies for deps testing.
fn test_ir() -> Ir {
    let file_a = PathBuf::from("src/alpha.rs");
    let file_b = PathBuf::from("src/beta.rs");

    let symbols = vec![
        Symbol {
            name: "foo".to_string(),
            qualified_name: "crate::alpha::foo".to_string(),
            kind: SymbolKind::Function,
            loc: SourceLoc { file: file_a.clone(), line: 10, col: 1 },
            visibility: Visibility::Public,
            signature: Some("pub fn foo(x: Bar) -> i32".to_string()),
            parent: None,
            attributes: vec![],
            fields: vec![],
            params: vec![Param { name: "x".to_string(), type_name: "Bar".to_string() }],
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
            attributes: vec![],
            fields: vec![
                Field { name: "x".to_string(), type_name: "i32".to_string(), visibility: Visibility::Public },
            ],
            params: vec![],
            return_type: None,
        },
        Symbol {
            name: "foo".to_string(), // duplicate name in different module
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
    ];

    let dependencies = vec![
        Dependency {
            from_qualified: "crate::alpha::foo".to_string(),
            to_name: "crate::alpha::Bar".to_string(),
            kind: DepKind::TypeReference,
            loc: SourceLoc { file: file_a.clone(), line: 10, col: 15 },
        },
        Dependency {
            from_qualified: "crate::alpha::foo".to_string(),
            to_name: "std::fmt::Display".to_string(),
            kind: DepKind::TraitImpl,
            loc: SourceLoc { file: file_a.clone(), line: 11, col: 5 },
        },
        Dependency {
            from_qualified: "crate::beta::foo".to_string(),
            to_name: "crate::alpha::Bar".to_string(),
            kind: DepKind::FunctionCall,
            loc: SourceLoc { file: file_b.clone(), line: 6, col: 10 },
        },
        Dependency {
            from_qualified: "crate::alpha::Bar".to_string(),
            to_name: "i32".to_string(),
            kind: DepKind::FieldType,
            loc: SourceLoc { file: file_a.clone(), line: 21, col: 5 },
        },
    ];

    Ir { symbols, dependencies }
}

#[test]
fn deps_single_symbol_returns_deps() {
    let ir = test_ir();
    let index = builder::build(&ir);
    let results = deps::collect_deps(&index, "Bar");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].qualified_name, "crate::alpha::Bar");
    assert_eq!(results[0].deps.len(), 1);
    assert_eq!(results[0].deps[0].to_name, "i32");
    assert_eq!(results[0].deps[0].kind, DepKind::FieldType);
}

#[test]
fn deps_duplicate_name_returns_multiple_groups() {
    let ir = test_ir();
    let index = builder::build(&ir);
    let results = deps::collect_deps(&index, "foo");
    assert_eq!(results.len(), 2);

    // Find the alpha::foo group
    let alpha = results.iter().find(|g| g.qualified_name == "crate::alpha::foo").unwrap();
    assert_eq!(alpha.deps.len(), 2);
    let dep_names: Vec<&str> = alpha.deps.iter().map(|d| d.to_name.as_str()).collect();
    assert!(dep_names.contains(&"crate::alpha::Bar"));
    assert!(dep_names.contains(&"std::fmt::Display"));

    // Find the beta::foo group
    let beta = results.iter().find(|g| g.qualified_name == "crate::beta::foo").unwrap();
    assert_eq!(beta.deps.len(), 1);
    assert_eq!(beta.deps[0].to_name, "crate::alpha::Bar");
    assert_eq!(beta.deps[0].kind, DepKind::FunctionCall);
}

#[test]
fn deps_no_match_returns_empty() {
    let ir = test_ir();
    let index = builder::build(&ir);
    let results = deps::collect_deps(&index, "nonexistent");
    assert!(results.is_empty());
}

#[test]
fn deps_symbol_with_no_deps_returns_empty_group() {
    let ir = Ir {
        symbols: vec![Symbol {
            name: "Lonely".to_string(),
            qualified_name: "crate::lonely::Lonely".to_string(),
            kind: SymbolKind::Struct,
            loc: SourceLoc { file: PathBuf::from("src/lonely.rs"), line: 1, col: 1 },
            visibility: Visibility::Public,
            signature: None,
            parent: None,
            attributes: vec![],
            fields: vec![],
            params: vec![],
            return_type: None,
        }],
        dependencies: vec![],
    };
    let index = builder::build(&ir);
    let results = deps::collect_deps(&index, "Lonely");
    assert_eq!(results.len(), 1);
    assert!(results[0].deps.is_empty());
}
