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
    let structs: Vec<_> = ir.symbols.iter().filter(|s| s.kind == SymbolKind::Struct).collect();
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
    let enums: Vec<_> = ir.symbols.iter().filter(|s| s.kind == SymbolKind::Enum).collect();
    assert_eq!(enums.len(), 1);
    assert_eq!(enums[0].name, "Status");
}

#[test]
fn fixture_has_trait() {
    let ir = parse_fixture();
    let traits: Vec<_> = ir.symbols.iter().filter(|s| s.kind == SymbolKind::Trait).collect();
    assert_eq!(traits.len(), 1);
    assert_eq!(traits[0].name, "Processor");
}

#[test]
fn fixture_has_impl_blocks() {
    let ir = parse_fixture();
    let impls: Vec<_> = ir.symbols.iter().filter(|s| s.kind == SymbolKind::Impl).collect();
    assert_eq!(impls.len(), 2);
    // One is "impl Config", the other is "impl Processor for Config"
    let names: Vec<&str> = impls.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"impl Config"));
    assert!(names.contains(&"impl Processor for Config"));
}

#[test]
fn fixture_has_methods() {
    let ir = parse_fixture();
    let methods: Vec<_> = ir.symbols.iter().filter(|s| s.kind == SymbolKind::Method).collect();
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
    let fns: Vec<_> = ir.symbols.iter().filter(|s| s.kind == SymbolKind::Function).collect();
    assert_eq!(fns.len(), 2);
    let fn_names: Vec<&str> = fns.iter().map(|s| s.name.as_str()).collect();
    assert!(fn_names.contains(&"standalone_function"));
    assert!(fn_names.contains(&"private_helper"));
}

#[test]
fn fixture_has_const() {
    let ir = parse_fixture();
    let consts: Vec<_> = ir.symbols.iter().filter(|s| s.kind == SymbolKind::Const).collect();
    assert_eq!(consts.len(), 1);
    assert_eq!(consts[0].name, "MAX_SIZE");
}

#[test]
fn fixture_has_type_alias() {
    let ir = parse_fixture();
    let types: Vec<_> = ir.symbols.iter().filter(|s| s.kind == SymbolKind::TypeAlias).collect();
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
    let trait_impls: Vec<_> = ir.dependencies.iter().filter(|d| d.kind == DepKind::TraitImpl).collect();
    assert_eq!(trait_impls.len(), 1);
    assert_eq!(trait_impls[0].to_name, "Processor");
}

#[test]
fn method_has_correct_parent() {
    let ir = parse_fixture();
    let new_method = ir.symbols.iter().find(|s| s.name == "new" && s.kind == SymbolKind::Method).unwrap();
    assert_eq!(new_method.parent.as_deref(), Some("Config"));
}

#[test]
fn qualified_names_use_file_path() {
    let ir = parse_fixture();
    let config = ir.symbols.iter().find(|s| s.name == "Config" && s.kind == SymbolKind::Struct).unwrap();
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
    let config = ir.symbols.iter().find(|s| s.name == "Config" && s.kind == SymbolKind::Struct).unwrap();
    assert!(!config.attributes.is_empty());
    assert!(config.attributes[0].contains("derive"));
}

#[test]
fn private_function_visibility() {
    let ir = parse_fixture();
    let helper = ir.symbols.iter().find(|s| s.name == "private_helper").unwrap();
    assert_eq!(helper.visibility, Visibility::Private);
}
