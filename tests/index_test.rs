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

// ---------------------------------------------------------------------------
// Type references derived by the builder (TypeRef / FieldType)
// ---------------------------------------------------------------------------

fn sym(name: &str, qn: &str, kind: &str, file: &str, line: usize) -> Symbol {
    Symbol::new(
        name.to_string(),
        qn.to_string(),
        kind,
        SourceLoc { file: PathBuf::from(file), line, col: 1 },
        Visibility::Public,
    )
}

fn callable(qn: &str, kind: &str, file: &str, params: &[&str], ret: Option<&str>) -> Symbol {
    let name = qn.rsplit(|c| c == ':' || c == '.').next().unwrap();
    let mut s = sym(name, qn, kind, file, 10);
    s.params = params
        .iter()
        .enumerate()
        .map(|(i, t)| Param { name: format!("p{}", i), type_name: t.to_string() })
        .collect();
    s.return_type = ret.map(str::to_string);
    s
}

fn with_fields(mut s: Symbol, types: &[&str]) -> Symbol {
    s.fields = types
        .iter()
        .enumerate()
        .map(|(i, t)| Field { name: format!("f{}", i), type_name: t.to_string(), visibility: Visibility::Public })
        .collect();
    s
}

/// `(kind, to_name)` of the derived deps from `qn`.
fn type_deps_of(index: &Index, qn: &str) -> Vec<(String, String)> {
    index
        .deps_of(qn)
        .into_iter()
        .filter(|d| matches!(d.kind, DepKind::TypeRef | DepKind::FieldType))
        .map(|d| (d.kind.to_string(), d.to_name.clone()))
        .collect()
}

fn tr(to: &str) -> (String, String) {
    ("type_ref".to_string(), to.to_string())
}

fn ft(to: &str) -> (String, String) {
    ("field_type".to_string(), to.to_string())
}

#[test]
fn type_refs_rust_references_and_wrappers() {
    let f = "src/commands/deps.rs";
    let ir = Ir {
        symbols: vec![
            sym("Index", "crate::index::types::Index", "struct", "src/index/types.rs", 1),
            sym("Symbol", "crate::ir::types::Symbol", "struct", "src/ir/types.rs", 1),
            callable("crate::commands::deps::collect_deps", "fn", f, &["&Index", "&str"], Some("Vec<DepsGroup<'a>>")),
            callable("crate::x::find", "fn", f, &["&mut crate::index::types::Index"], Some("Option<&Symbol>")),
            callable("crate::x::maybe", "fn", f, &["Option<Index>", "&self"], Some("Result<(), String>")),
        ],
        dependencies: vec![],
    };
    let index = builder::build(&ir);
    assert_eq!(type_deps_of(&index, "crate::commands::deps::collect_deps"), vec![tr("Index")]);
    assert_eq!(
        type_deps_of(&index, "crate::x::find"),
        vec![tr("crate::index::types::Index"), tr("Symbol")]
    );
    assert_eq!(type_deps_of(&index, "crate::x::maybe"), vec![tr("Index")]);

    // refs by bare and qualified name; the dep carries the symbol's location.
    let refs: Vec<&str> = index.refs_to("Index").iter().map(|d| d.from_qualified.as_str()).collect();
    assert_eq!(refs, vec!["crate::commands::deps::collect_deps", "crate::x::find", "crate::x::maybe"]);
    assert_eq!(index.refs_to("types::Index").len(), 1);
    let d = index.refs_to("Symbol")[0];
    assert_eq!((d.loc.file.to_str().unwrap(), d.loc.line), (f, 10));
}

#[test]
fn type_refs_java_generics() {
    let f = "src/com/example/UserService.java";
    let ir = Ir {
        symbols: vec![
            sym("User", "com.example.User", "class", "src/com/example/User.java", 3),
            with_fields(sym("UserService", "com.example.UserService", "class", f, 8), &["Map<Long, User>", "long"]),
            callable("com.example.UserService.listAll", "method", f, &[], Some("List<User>")),
            callable("com.example.UserService.save", "method", f, &["com.example.User"], Some("long")),
        ],
        dependencies: vec![],
    };
    let index = builder::build(&ir);
    assert_eq!(type_deps_of(&index, "com.example.UserService"), vec![ft("User")]);
    assert_eq!(type_deps_of(&index, "com.example.UserService.listAll"), vec![tr("User")]);
    assert_eq!(type_deps_of(&index, "com.example.UserService.save"), vec![tr("com.example.User")]);
    assert!(index.refs_to("example.User").iter().all(|d| d.to_name == "com.example.User"));
}

#[test]
fn type_refs_python_subscripts() {
    let f = "src/shop/services/user_service.py";
    let ir = Ir {
        symbols: vec![
            sym("User", "shop.models.user.User", "class", "src/shop/models/user.py", 15),
            callable("shop.services.user_service.paginate", "def", f, &["Iterable[User]", "int"], Some("list[User]")),
            callable("shop.services.user_service.UserService.__init__", "method", f, &["Repository[User]"], Some("None")),
            with_fields(sym("Box", "shop.box.Box", "class", f, 40), &["dict[str, list[User]]", ""]),
        ],
        dependencies: vec![],
    };
    let index = builder::build(&ir);
    // Deduped: `User` in both a param and the return type is one dep.
    assert_eq!(type_deps_of(&index, "shop.services.user_service.paginate"), vec![tr("User")]);
    // `Repository` is not a project type here, so only `User` counts.
    assert_eq!(type_deps_of(&index, "shop.services.user_service.UserService.__init__"), vec![tr("User")]);
    assert_eq!(type_deps_of(&index, "shop.box.Box"), vec![ft("User")]);
}

#[test]
fn type_refs_go_pointers_slices_and_packages() {
    let f = "service.go";
    let ir = Ir {
        symbols: vec![
            with_fields(sym("User", "main.User", "struct", "model.go", 10), &["int64", "string"]),
            with_fields(sym("UserService", "main.UserService", "struct", f, 14), &["map[int64]*User", "int64"]),
            callable("main.NewUser", "func", "model.go", &["int64", "string"], Some("*User")),
            callable("main.UserService.FindByID", "method", f, &["int64"], Some("(*User, error)")),
            callable("main.Batch", "func", f, &["[]User", "[]*models.User"], None),
        ],
        dependencies: vec![],
    };
    let index = builder::build(&ir);
    assert!(type_deps_of(&index, "main.User").is_empty(), "primitives only");
    assert_eq!(type_deps_of(&index, "main.UserService"), vec![ft("User")]);
    assert_eq!(type_deps_of(&index, "main.NewUser"), vec![tr("User")]);
    assert_eq!(type_deps_of(&index, "main.UserService.FindByID"), vec![tr("User")]);
    assert_eq!(type_deps_of(&index, "main.Batch"), vec![tr("User"), tr("models.User")]);
}

#[test]
fn type_refs_typescript_arrays_unions_and_promises() {
    let f = "src/services/user-service.ts";
    let ir = Ir {
        symbols: vec![
            sym("User", "User", "class", "src/models.ts", 17),
            sym("PageRequest", "Pagination.PageRequest", "interface", "src/utils.ts", 22),
            callable("services.UserService.listAll", "method", f, &[], Some("User[]")),
            callable("services.UserService.findById", "method", f, &["number"], Some("User | null")),
            callable("services.load", "function", f, &["PageRequest"], Some("Promise<User>")),
            callable("services.ext", "function", f, &["Request", "Response"], Some("Promise<void>")),
        ],
        dependencies: vec![],
    };
    let index = builder::build(&ir);
    assert_eq!(type_deps_of(&index, "services.UserService.listAll"), vec![tr("User")]);
    assert_eq!(type_deps_of(&index, "services.UserService.findById"), vec![tr("User")]);
    assert_eq!(type_deps_of(&index, "services.load"), vec![tr("PageRequest"), tr("User")]);
    assert!(type_deps_of(&index, "services.ext").is_empty(), "external types ignored");
}

#[test]
fn type_refs_ignore_primitives_external_types_and_non_type_symbols() {
    let ir = Ir {
        symbols: vec![
            sym("Config", "crate::Config", "struct", "src/lib.rs", 1),
            // A function named like a type does not make `load` a type name.
            sym("load", "crate::load", "fn", "src/lib.rs", 5),
            callable("crate::run", "fn", "src/lib.rs", &["u32", "&str", "HashMap<String, Vec<u8>>", "load"], Some("std::io::Result<()>")),
        ],
        dependencies: vec![],
    };
    let index = builder::build(&ir);
    assert!(index.deps.is_empty(), "{:?}", index.deps);
}

#[test]
fn type_refs_recursive_field_kept_and_deduped() {
    let ir = Ir {
        symbols: vec![with_fields(
            sym("Node", "crate::Node", "struct", "src/lib.rs", 1),
            &["Option<Box<Node>>", "Vec<Node>", "u32"],
        )],
        dependencies: vec![],
    };
    let index = builder::build(&ir);
    assert_eq!(type_deps_of(&index, "crate::Node"), vec![ft("Node")]);
    assert_eq!(index.refs_to("Node").len(), 1);
}

#[test]
fn type_refs_grouped_with_their_file() {
    let a = "src/a.rs";
    let b = "src/b.rs";
    let import = |file: &str| Dependency {
        from_qualified: "m".to_string(),
        to_name: "crate::T".to_string(),
        kind: DepKind::Import,
        loc: SourceLoc { file: PathBuf::from(file), line: 1, col: 1 },
    };
    let ir = Ir {
        symbols: vec![
            sym("T", "crate::T", "struct", a, 1),
            callable("crate::a::f", "fn", a, &["T"], None),
            callable("crate::b::g", "fn", b, &["T"], None),
        ],
        dependencies: vec![import(a), import(b)],
    };
    let index = builder::build(&ir);
    let order: Vec<(&str, String)> = index
        .deps
        .iter()
        .map(|d| (d.loc.file.to_str().unwrap(), d.kind.to_string()))
        .collect();
    assert_eq!(
        order,
        vec![
            (a, "import".to_string()),
            (a, "type_ref".to_string()),
            (b, "import".to_string()),
            (b, "type_ref".to_string()),
        ]
    );
}

#[test]
fn type_name_tokens_split_paths() {
    use smartgrep::index::builder::type_name_tokens;
    assert_eq!(type_name_tokens("&mut crate::x::Index"), vec!["mut", "crate::x::Index"]);
    assert_eq!(type_name_tokens("map[int64]*models.User"), vec!["map", "int64", "models.User"]);
    assert_eq!(type_name_tokens("Promise<User[]>"), vec!["Promise", "User"]);
    assert_eq!(type_name_tokens("{ a: User }"), vec!["a", "User"]);
    assert_eq!(type_name_tokens("Vec<DepsGroup<'a>>"), vec!["Vec", "DepsGroup", "a"]);
    assert_eq!(type_name_tokens("[u8; 32]"), vec!["u8"]);
    assert_eq!(type_name_tokens("x::"), vec!["x"]);
}
