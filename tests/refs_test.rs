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
            kind: "struct".to_string(),
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
            kind: "fn".to_string(),
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
            kind: "fn".to_string(),
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

    // `run(cfg: &Config)` and `init() -> Config` get their TypeRef deps from
    // the builder (derived from params / return types).
    let dependencies = vec![
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
            kind: DepKind::Call,
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
    // 2 type refs + the `crate::alpha::Config` import (matched on last segment)
    assert_eq!(refs.len(), 3, "Config should have 2 type refs and 1 import");
    assert!(refs.iter().all(|d| d.to_name.ends_with("Config")));
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
    assert_eq!(refs[0].kind, DepKind::Call);
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
    let type_refs = refs.iter().filter(|d| d.kind == DepKind::TypeRef).count();
    let imports = refs.iter().filter(|d| d.kind == DepKind::Import).count();
    assert_eq!((type_refs, imports), (2, 1));
}

#[test]
fn refs_to_locations_are_correct() {
    let index = build_refs_test_index();
    let refs = index.refs_to("run");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].loc.file, PathBuf::from("src/gamma.rs"));
    assert_eq!(refs[0].loc.line, 5);
}

// ---------------------------------------------------------------------------
// Normalized matching (bare / qualified / generic / owner)
// ---------------------------------------------------------------------------

fn dep(from: &str, to: &str, kind: DepKind, line: usize) -> Dependency {
    Dependency {
        from_qualified: from.to_string(),
        to_name: to.to_string(),
        kind,
        loc: SourceLoc { file: PathBuf::from("src/x.rs"), line, col: 1 },
    }
}

fn matching_index() -> smartgrep::index::types::Index {
    builder::build(&Ir {
        symbols: vec![],
        dependencies: vec![
            dep("crate::a", "crate::ir::types::Symbol", DepKind::Import, 1),
            dep("crate::a::f", "Symbol::new", DepKind::Call, 2),
            dep("crate::a::g", "crate::ir::types::Symbol::new", DepKind::Call, 3),
            dep("crate::a::h", "new", DepKind::Call, 4),
            dep("crate::a::Foo", "Processor<String>", DepKind::Implements, 5),
            dep("crate::a::Bar", "std::fmt::Display", DepKind::Implements, 6),
            dep("main.run", "fmt.Println", DepKind::Call, 7),
            dep("main", "github.com/acme/app/models", DepKind::Import, 8),
            dep("crate::a::k", "x.items.collect::<Vec<_>>", DepKind::Call, 9),
        ],
    })
}

fn lines(refs: &[&Dependency]) -> Vec<usize> {
    let mut v: Vec<usize> = refs.iter().map(|d| d.loc.line).collect();
    v.sort();
    v
}

#[test]
fn refs_bare_name_matches_last_segment_of_paths() {
    let index = matching_index();
    // import + `Symbol::new` / `...::Symbol::new` calls (owner key)
    assert_eq!(lines(&index.refs_to("Symbol")), vec![1, 2, 3]);
    assert_eq!(lines(&index.refs_to("new")), vec![2, 3, 4]);
    assert_eq!(lines(&index.refs_to("Println")), vec![7]);
    assert_eq!(lines(&index.refs_to("models")), vec![8]);
}

#[test]
fn refs_qualified_name_requires_path_suffix() {
    let index = matching_index();
    assert_eq!(lines(&index.refs_to("Symbol::new")), vec![2, 3]);
    assert_eq!(lines(&index.refs_to("types::Symbol::new")), vec![3]);
    assert_eq!(lines(&index.refs_to("ir::types::Symbol")), vec![1]);
    // `.` and `::` are interchangeable separators
    assert_eq!(lines(&index.refs_to("fmt::Println")), vec![7]);
    assert_eq!(lines(&index.refs_to("fmt.Println")), vec![7]);
    assert!(index.refs_to("Other::new").is_empty());
}

#[test]
fn refs_ignore_generics() {
    let index = matching_index();
    assert_eq!(lines(&index.refs_to("Processor")), vec![5]);
    assert_eq!(lines(&index.refs_to("Processor<T>")), vec![5]);
    assert_eq!(lines(&index.refs_to("collect")), vec![9]);
    assert_eq!(lines(&index.refs_to("Display")), vec![6]);
    assert_eq!(lines(&index.refs_to("fmt::Display")), vec![6]);
}
