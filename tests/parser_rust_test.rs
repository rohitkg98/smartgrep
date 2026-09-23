use std::path::Path;

use smartgrep::ir::types::*;
use smartgrep::parser::rust::parse_file;

fn parse_fixture() -> Ir {
    let source = include_str!("fixtures/sample.rs");
    parse_file(Path::new("tests/fixtures/sample.rs"), source).unwrap()
}

#[test]
fn fixture_has_struct() {
    let ir = parse_fixture();
    let structs: Vec<_> = ir.symbols.iter().filter(|s| s.kind == "struct").collect();
    assert_eq!(structs.len(), 1);
    assert_eq!(structs[0].name, "Config");
    assert_eq!(structs[0].fields.len(), 3);
    assert_eq!(structs[0].fields[0].name, "name");
    assert_eq!(structs[0].fields[1].name, "values");
    assert_eq!(structs[0].fields[2].name, "timeout");
    assert_eq!(structs[0].visibility, Visibility::Public);
}

#[test]
fn fixture_has_enum() {
    let ir = parse_fixture();
    let enums: Vec<_> = ir.symbols.iter().filter(|s| s.kind == "enum").collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "Status");
}

#[test]
fn fixture_has_trait() {
    let ir = parse_fixture();
    let traits: Vec<_> = ir.symbols.iter().filter(|s| s.kind == "trait").collect();
    assert_eq!(traits.len(), 1);
    assert_eq!(traits[0].name, "Processor");
}

#[test]
fn fixture_has_impl_blocks() {
    let ir = parse_fixture();
    let impls: Vec<_> = ir.symbols.iter().filter(|s| s.kind == "impl").collect();
    assert_eq!(impls.len(), 2);
    // One is "impl Config", the other is "impl Processor for Config"
    let names: Vec<&str> = impls.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"impl Config"));
    assert!(names.contains(&"impl Processor for Config"));
}

#[test]
fn fixture_has_methods() {
    let ir = parse_fixture();
    let methods: Vec<_> = ir.symbols.iter().filter(|s| s.kind == "method").collect();
    // new, add_value from impl Config; process, name from impl Processor for Config
    assert_eq!(methods.len(), 4);
    let method_names: Vec<&str> = methods.iter().map(|s| s.name.as_str()).collect();
    assert!(method_names.contains(&"new"));
    assert!(method_names.contains(&"add_value"));
    assert!(method_names.contains(&"process"));
}

#[test]
fn fixture_has_standalone_functions() {
    let ir = parse_fixture();
    let fns: Vec<_> = ir.symbols.iter().filter(|s| s.kind == "fn").collect();
    assert_eq!(fns.len(), 2);
    let fn_names: Vec<&str> = fns.iter().map(|s| s.name.as_str()).collect();
    assert!(fn_names.contains(&"standalone_function"));
    assert!(fn_names.contains(&"private_helper"));
}

#[test]
fn fixture_has_const() {
    let ir = parse_fixture();
    let consts: Vec<_> = ir.symbols.iter().filter(|s| s.kind == "const").collect();
    assert_eq!(consts.len(), 1);
    assert_eq!(consts[0].name, "MAX_SIZE");
}

#[test]
fn fixture_has_type_alias() {
    let ir = parse_fixture();
    let types: Vec<_> = ir.symbols.iter().filter(|s| s.kind == "type").collect();
    assert_eq!(types.len(), 1);
    assert_eq!(types[0].name, "Callback");
}

#[test]
fn fixture_has_imports() {
    let ir = parse_fixture();
    let imports: Vec<_> = ir.dependencies.iter().filter(|d| d.kind == DepKind::Import).collect();
    assert_eq!(imports.len(), 2);
}

#[test]
fn fixture_has_trait_impl_dep() {
    let ir = parse_fixture();
    let trait_impls: Vec<_> = ir.dependencies.iter().filter(|d| d.kind == DepKind::Implements).collect();
    assert_eq!(trait_impls.len(), 1);
    assert_eq!(trait_impls[0].to_name, "Processor");
}

#[test]
fn method_has_correct_parent() {
    let ir = parse_fixture();
    let new_method = ir.symbols.iter().find(|s| s.name == "new" && s.kind == "method").unwrap();
    assert_eq!(new_method.parent.as_deref(), Some("Config"));
}

#[test]
fn qualified_names_use_file_path() {
    let ir = parse_fixture();
    let config = ir.symbols.iter().find(|s| s.name == "Config" && s.kind == "struct").unwrap();
    assert!(config.qualified_name.starts_with("crate::"));
    assert!(config.qualified_name.contains("sample"));
}

#[test]
fn function_has_params() {
    let ir = parse_fixture();
    let f = ir.symbols.iter().find(|s| s.name == "standalone_function").unwrap();
    assert_eq!(f.params.len(), 2);
    assert_eq!(f.params[0].name, "x");
    assert_eq!(f.params[0].type_name, "i32");
    assert_eq!(f.params[1].name, "y");
}

#[test]
fn struct_has_attributes() {
    let ir = parse_fixture();
    let config = ir.symbols.iter().find(|s| s.name == "Config" && s.kind == "struct").unwrap();
    assert!(!config.attributes.is_empty());
    assert!(config.attributes[0].contains("derive"));
}

#[test]
fn private_function_visibility() {
    let ir = parse_fixture();
    let helper = ir.symbols.iter().find(|s| s.name == "private_helper").unwrap();
    assert_eq!(helper.visibility, Visibility::Private);
}

// ---------------------------------------------------------------------------
// Call deps + grouped imports (tests/fixtures/calls.rs)
// ---------------------------------------------------------------------------

fn parse_calls_fixture() -> Ir {
    let source = include_str!("fixtures/calls.rs");
    parse_file(Path::new("src/calls.rs"), source).unwrap()
}

fn calls_from<'a>(ir: &'a Ir, from: &str) -> Vec<&'a str> {
    ir.dependencies
        .iter()
        .filter(|d| d.kind == DepKind::Call && d.from_qualified == from)
        .map(|d| d.to_name.as_str())
        .collect()
}

#[test]
fn fixture_method_calls_in_source_order_deduped() {
    let ir = parse_calls_fixture();
    let calls = calls_from(&ir, "crate::calls::Registry::build");
    assert_eq!(
        calls,
        vec![
            "HashMap::new", // static path kept as written
            "push",         // `self.items.push()` → method name only, deduped
            "iter",
            "map",
            "transform", // inside a closure → attributed to `build`
            "collect",   // turbofish stripped
            "parse",
            "deep_call", // inside a nested fn → attributed to `build`
            "crate::store::load",
        ]
    );
    // first occurrence's location wins
    let push = ir
        .dependencies
        .iter()
        .find(|d| d.to_name == "push")
        .unwrap();
    assert_eq!(push.loc.line, 28);
}

#[test]
fn fixture_skips_macros_struct_literals_and_variant_ctors() {
    let ir = parse_calls_fixture();
    let all: Vec<&str> = ir
        .dependencies
        .iter()
        .filter(|d| d.kind == DepKind::Call)
        .map(|d| d.to_name.as_str())
        .collect();
    for skipped in ["println", "println!", "ignored_in_macro", "write", "Registry", "Some"] {
        assert!(!all.contains(&skipped), "{} should not be a call dep", skipped);
    }
    // `Registry { items: Vec::new() }`: the literal is skipped, calls inside it are not
    assert_eq!(calls_from(&ir, "crate::calls::Registry::new"), vec!["Vec::new"]);
}

#[test]
fn fixture_free_fn_and_trait_default_method_calls() {
    let ir = parse_calls_fixture();
    assert_eq!(
        calls_from(&ir, "crate::calls::helper"),
        vec!["Registry::new", "to_uppercase"]
    );
    // trait default methods are emitted as methods, and their calls recorded
    let describe = ir
        .symbols
        .iter()
        .find(|s| s.name == "describe")
        .expect("trait default method should be a symbol");
    assert_eq!(describe.kind, "method");
    assert_eq!(describe.parent.as_deref(), Some("Describe"));
    assert_eq!(
        calls_from(&ir, "crate::calls::Describe::describe"),
        vec!["label", "helper"]
    );
    // required trait methods (no body) are not symbols
    assert!(!ir.symbols.iter().any(|s| s.name == "label"));
}

#[test]
fn fixture_grouped_imports_are_split_per_leaf() {
    let ir = parse_calls_fixture();
    let imports: Vec<&str> = ir
        .dependencies
        .iter()
        .filter(|d| d.kind == DepKind::Import)
        .map(|d| d.to_name.as_str())
        .collect();
    assert_eq!(
        imports,
        vec![
            "std::collections::HashMap",
            "std::collections::hash_map::Entry",
            "std::fmt",
            "std::fmt::Display",
            "crate::store::load",
            "crate::ir::types::*",
        ]
    );
}

#[test]
fn fixture_implements_recorded_for_trait_impl() {
    let ir = parse_calls_fixture();
    let imp: Vec<_> = ir
        .dependencies
        .iter()
        .filter(|d| d.kind == DepKind::Implements)
        .collect();
    assert_eq!(imp.len(), 1);
    assert_eq!(imp[0].from_qualified, "crate::calls::Registry");
    assert_eq!(imp[0].to_name, "Display");
}

#[test]
fn sample_fixture_method_call_dep() {
    let ir = parse_fixture();
    let calls: Vec<_> = ir
        .dependencies
        .iter()
        .filter(|d| d.kind == DepKind::Call)
        .collect();
    // `self.values.push(v)` in add_value; `format!` is a macro and skipped
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].to_name, "push");
    assert!(calls[0].from_qualified.ends_with("Config::add_value"));
}
