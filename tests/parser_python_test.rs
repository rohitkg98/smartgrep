use std::path::Path;

use smartgrep::ir::types::*;
use smartgrep::parser::python::parse_file;

fn parse_fixture() -> Ir {
    let source = include_str!("fixtures/sample.py");
    parse_file(Path::new("src/app/sample.py"), source).unwrap()
}

fn find<'a>(ir: &'a Ir, name: &str) -> &'a Symbol {
    ir.symbols
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("symbol {} not found", name))
}

fn names_of_kind<'a>(ir: &'a Ir, kind: &str) -> Vec<&'a str> {
    ir.symbols
        .iter()
        .filter(|s| s.kind == kind)
        .map(|s| s.name.as_str())
        .collect()
}

// ---------------------------------------------------------------------------
// Imports
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_imports() {
    let ir = parse_fixture();
    let imports: Vec<&str> = ir
        .dependencies
        .iter()
        .filter(|d| d.kind == DepKind::Import)
        .map(|d| d.to_name.as_str())
        .collect();
    for expected in [
        "os",
        "collections.abc",
        "typing.Protocol",
        "dataclasses.dataclass",
        "dataclasses.field",
        "ntpath",     // inside `if`
        "posixpath",  // inside `else`
    ] {
        assert!(imports.contains(&expected), "missing import {}: {:?}", expected, imports);
    }
}

#[test]
fn future_import_is_ignored() {
    let ir = parse_fixture();
    assert!(!ir.dependencies.iter().any(|d| d.to_name.contains("__future__")));
}

#[test]
fn relative_imports_resolve_against_package() {
    let ir = parse_fixture();
    let imports: Vec<&str> = ir.dependencies.iter().map(|d| d.to_name.as_str()).collect();
    // file is src/app/sample.py → package `app`
    assert!(imports.contains(&"app.helpers"), "{:?}", imports);
    assert!(imports.contains(&"app.base.BaseModel"));
    assert!(imports.contains(&"app.base.Validator"), "aliased import records original name");
    assert!(imports.contains(&"core.errors.*"), "wildcard from parent package");
}

#[test]
fn imports_come_from_module_path() {
    let ir = parse_fixture();
    let dep = ir.dependencies.iter().find(|d| d.to_name == "os").unwrap();
    assert_eq!(dep.from_qualified, "app.sample");
}

// ---------------------------------------------------------------------------
// Module-level functions
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_defs() {
    let ir = parse_fixture();
    let defs = names_of_kind(&ir, "def");
    assert_eq!(defs, vec!["top_level", "fetch", "get_user", "_private_fn"]);
}

#[test]
fn nested_functions_are_skipped() {
    let ir = parse_fixture();
    assert!(!ir.symbols.iter().any(|s| s.name == "nested_helper"));
    assert!(!ir.symbols.iter().any(|s| s.name == "_inner"));
}

#[test]
fn def_qualified_name() {
    let ir = parse_fixture();
    assert_eq!(find(&ir, "top_level").qualified_name, "app.sample.top_level");
}

#[test]
fn def_params_and_signature() {
    let ir = parse_fixture();
    let f = find(&ir, "top_level");
    let params: Vec<(&str, &str)> = f
        .params
        .iter()
        .map(|p| (p.name.as_str(), p.type_name.as_str()))
        .collect();
    assert_eq!(
        params,
        vec![
            ("a", ""),
            ("b", "int"),
            ("c", ""),
            ("d", "str"),
            ("*rest", ""),
            ("**opts", ""),
        ]
    );
    assert_eq!(f.return_type.as_deref(), Some("dict[str, int]"));
    assert_eq!(
        f.signature.as_deref(),
        Some("def top_level(a, b: int, c=1, d: str = \"x\", *rest, **opts) -> dict[str, int]")
    );
}

#[test]
fn async_def_signature() {
    let ir = parse_fixture();
    let f = find(&ir, "fetch");
    assert_eq!(f.kind, "def");
    assert!(f.signature.as_deref().unwrap().starts_with("async def fetch("));
    assert!(f.attributes.contains(&"async".to_string()));
}

#[test]
fn decorated_def_has_decorator() {
    let ir = parse_fixture();
    let f = find(&ir, "get_user");
    assert_eq!(f.attributes, vec!["@app.route(\"/users/<id>\")".to_string()]);
}

#[test]
fn private_def_visibility() {
    let ir = parse_fixture();
    assert_eq!(find(&ir, "_private_fn").visibility, Visibility::Private);
    assert_eq!(find(&ir, "top_level").visibility, Visibility::Public);
}

// ---------------------------------------------------------------------------
// Classes
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_classes() {
    let ir = parse_fixture();
    let classes = names_of_kind(&ir, "class");
    assert_eq!(classes, vec!["Repository", "User", "Cache", "Entry", "_Hidden"]);
}

#[test]
fn class_qualified_name_and_signature() {
    let ir = parse_fixture();
    let user = find(&ir, "User");
    assert_eq!(user.qualified_name, "app.sample.User");
    assert_eq!(user.signature.as_deref(), Some("class User(BaseModel)"));
}

#[test]
fn dataclass_decorator() {
    let ir = parse_fixture();
    let user = find(&ir, "User");
    assert_eq!(user.attributes, vec!["@dataclass(frozen=True)".to_string()]);
}

#[test]
fn class_fields_from_body_and_init() {
    let ir = parse_fixture();
    let user = find(&ir, "User");
    let fields: Vec<(&str, &str)> = user
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.type_name.as_str()))
        .collect();
    assert_eq!(
        fields,
        vec![
            ("name", "str"),
            ("age", "int"),
            ("tags", "list[str]"),
            ("_secret", "str"),
            ("kind", ""),
            ("email", "str | None"), // self.email: str | None = None
            ("_cache", ""),          // self._cache = {}
        ]
    );
}

#[test]
fn private_field_visibility() {
    let ir = parse_fixture();
    let user = find(&ir, "User");
    let secret = user.fields.iter().find(|f| f.name == "_secret").unwrap();
    assert_eq!(secret.visibility, Visibility::Private);
    let name = user.fields.iter().find(|f| f.name == "name").unwrap();
    assert_eq!(name.visibility, Visibility::Public);
}

#[test]
fn private_class_visibility() {
    let ir = parse_fixture();
    assert_eq!(find(&ir, "_Hidden").visibility, Visibility::Private);
}

#[test]
fn nested_class_has_parent() {
    let ir = parse_fixture();
    let entry = find(&ir, "Entry");
    assert_eq!(entry.parent.as_deref(), Some("Cache"));
    assert_eq!(entry.qualified_name, "app.sample.Cache.Entry");
}

// ---------------------------------------------------------------------------
// Inheritance
// ---------------------------------------------------------------------------

fn implements_of<'a>(ir: &'a Ir, qn: &str) -> Vec<&'a str> {
    ir.dependencies
        .iter()
        .filter(|d| d.kind == DepKind::Implements && d.from_qualified == qn)
        .map(|d| d.to_name.as_str())
        .collect()
}

#[test]
fn base_classes_are_implements_deps() {
    let ir = parse_fixture();
    assert_eq!(implements_of(&ir, "app.sample.User"), vec!["BaseModel"]);
    assert_eq!(implements_of(&ir, "app.sample.Repository"), vec!["Protocol"]);
}

#[test]
fn generic_and_dotted_bases_strip_to_name_and_skip_kwargs() {
    let ir = parse_fixture();
    // class Cache(Generic[T], cabc.Mapping, metaclass=type)
    assert_eq!(implements_of(&ir, "app.sample.Cache"), vec!["Generic", "Mapping"]);
}

// ---------------------------------------------------------------------------
// Methods
// ---------------------------------------------------------------------------

#[test]
fn methods_have_parent_and_qualified_name() {
    let ir = parse_fixture();
    let save = find(&ir, "save");
    assert_eq!(save.kind, "method");
    assert_eq!(save.parent.as_deref(), Some("User"));
    assert_eq!(save.qualified_name, "app.sample.User.save");
}

#[test]
fn method_params_exclude_self_and_cls() {
    let ir = parse_fixture();
    let init = find(&ir, "__init__");
    let names: Vec<&str> = init.params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["name", "age"]);

    let create = find(&ir, "create");
    let names: Vec<&str> = create.params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["name"]);
}

#[test]
fn staticmethod_keeps_first_param() {
    let ir = parse_fixture();
    let v = find(&ir, "validate");
    let names: Vec<&str> = v.params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["value", "*args", "strict", "**kwargs"]);
    assert!(v.attributes.contains(&"@staticmethod".to_string()));
}

#[test]
fn method_decorators() {
    let ir = parse_fixture();
    assert_eq!(find(&ir, "display_name").attributes, vec!["@property".to_string()]);
    assert_eq!(find(&ir, "create").attributes, vec!["@classmethod".to_string()]);
}

#[test]
fn async_method_signature() {
    let ir = parse_fixture();
    let save = find(&ir, "save");
    assert_eq!(save.signature.as_deref(), Some("async def save(self, *, force=False) -> None"));
    let names: Vec<&str> = save.params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["force"]);
}

#[test]
fn positional_only_separator_is_not_a_param() {
    let ir = parse_fixture();
    let lookup = find(&ir, "lookup");
    let names: Vec<&str> = lookup.params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["key", "default"]);
}

#[test]
fn dunder_method_is_public_private_method_is_private() {
    let ir = parse_fixture();
    assert_eq!(find(&ir, "__repr__").visibility, Visibility::Public);
    assert_eq!(find(&ir, "_private_helper").visibility, Visibility::Private);
}

#[test]
fn protocol_stub_methods() {
    let ir = parse_fixture();
    let get = find(&ir, "get");
    assert_eq!(get.parent.as_deref(), Some("Repository"));
    assert_eq!(get.return_type.as_deref(), Some("T | None"));
}

// ---------------------------------------------------------------------------
// Constants and type aliases
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_constants() {
    let ir = parse_fixture();
    let consts = names_of_kind(&ir, "const");
    assert_eq!(consts, vec!["MAX_RETRIES", "DEFAULT_TIMEOUT", "_INTERNAL_LIMIT"]);
    assert_eq!(find(&ir, "_INTERNAL_LIMIT").visibility, Visibility::Private);
}

#[test]
fn lowercase_assignments_typevars_and_dunder_all_are_not_symbols() {
    let ir = parse_fixture();
    for name in ["logger", "__all__", "T", "Lookup"] {
        assert!(!ir.symbols.iter().any(|s| s.name == name), "{} should be skipped", name);
    }
}

#[test]
fn fixture_has_type_aliases() {
    let ir = parse_fixture();
    let types = names_of_kind(&ir, "type");
    assert_eq!(types, vec!["AccountId", "UserId", "JsonValue", "Callback", "Pair"]);
    assert_eq!(
        find(&ir, "Pair").signature.as_deref(),
        Some("type Pair[K, V] = tuple[K, V]")
    );
}

// ---------------------------------------------------------------------------
// Module paths
// ---------------------------------------------------------------------------

#[test]
fn init_module_uses_package_name() {
    let ir = parse_file(Path::new("src/pkg/__init__.py"), "class A:\n    pass\n").unwrap();
    assert_eq!(ir.symbols[0].qualified_name, "pkg.A");
}

#[test]
fn pyi_stub_is_parsed() {
    let ir = parse_file(Path::new("pkg/mod.pyi"), "def f(x: int) -> str: ...\n").unwrap();
    let f = &ir.symbols[0];
    assert_eq!(f.kind, "def");
    assert_eq!(f.qualified_name, "pkg.mod.f");
}

#[test]
fn dispatch_by_extension() {
    let ir = smartgrep::parser::parse_by_extension(Path::new("a.py"), "def f(): pass\n").unwrap();
    assert_eq!(ir.symbols[0].kind, "def");
}
