use std::path::Path;

use smartgrep::ir::types::*;
use smartgrep::parser::go::parse_file;

fn parse_fixture() -> Ir {
    let source = include_str!("fixtures/Sample.go");
    parse_file(Path::new("tests/fixtures/Sample.go"), source).unwrap()
}

// ---------------------------------------------------------------------------
// Package
// ---------------------------------------------------------------------------

#[test]
fn fixture_qualified_names_use_package() {
    let ir = parse_fixture();
    let config = ir
        .symbols
        .iter()
        .find(|s| s.name == "Config" && s.kind == "struct")
        .unwrap();
    assert_eq!(config.qualified_name, "sample.Config");
}

// ---------------------------------------------------------------------------
// Imports
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_imports() {
    let ir = parse_fixture();
    let imports: Vec<_> = ir
        .dependencies
        .iter()
        .filter(|d| d.kind == DepKind::Import)
        .collect();
    assert_eq!(imports.len(), 2);
    let names: Vec<&str> = imports.iter().map(|d| d.to_name.as_str()).collect();
    assert!(names.contains(&"fmt"));
    assert!(names.contains(&"io"));
}

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_structs() {
    let ir = parse_fixture();
    let structs: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "struct")
        .collect();
    let names: Vec<&str> = structs.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Config"));
    assert!(names.contains(&"Server"));
}

#[test]
fn fixture_config_has_fields() {
    let ir = parse_fixture();
    let config = ir
        .symbols
        .iter()
        .find(|s| s.name == "Config" && s.kind == "struct")
        .expect("should find Config");
    assert_eq!(config.fields.len(), 3);
    let field_names: Vec<&str> = config.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(field_names.contains(&"Name"));
    assert!(field_names.contains(&"Values"));
    assert!(field_names.contains(&"timeout"));
}

#[test]
fn fixture_struct_visibility_public() {
    let ir = parse_fixture();
    let config = ir
        .symbols
        .iter()
        .find(|s| s.name == "Config" && s.kind == "struct")
        .unwrap();
    assert_eq!(config.visibility, Visibility::Public);
}

#[test]
fn fixture_field_visibility_from_capitalization() {
    let ir = parse_fixture();
    let config = ir
        .symbols
        .iter()
        .find(|s| s.name == "Config" && s.kind == "struct")
        .unwrap();

    let name_field = config.fields.iter().find(|f| f.name == "Name").unwrap();
    assert_eq!(name_field.visibility, Visibility::Public);

    let timeout_field = config.fields.iter().find(|f| f.name == "timeout").unwrap();
    assert_eq!(timeout_field.visibility, Visibility::Private);
}

#[test]
fn fixture_server_has_embedded_field() {
    let ir = parse_fixture();
    let server = ir
        .symbols
        .iter()
        .find(|s| s.name == "Server" && s.kind == "struct")
        .expect("should find Server");
    let field_names: Vec<&str> = server.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(field_names.contains(&"Reader"));
    assert!(field_names.contains(&"Host"));
    assert!(field_names.contains(&"Port"));
}

// ---------------------------------------------------------------------------
// Interfaces (Traits)
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_interfaces() {
    let ir = parse_fixture();
    let traits: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "interface")
        .collect();
    let names: Vec<&str> = traits.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Handler"));
    assert!(names.contains(&"writer"));
}

#[test]
fn fixture_interface_visibility() {
    let ir = parse_fixture();
    let handler = ir
        .symbols
        .iter()
        .find(|s| s.name == "Handler" && s.kind == "interface")
        .unwrap();
    assert_eq!(handler.visibility, Visibility::Public);

    let writer = ir
        .symbols
        .iter()
        .find(|s| s.name == "writer" && s.kind == "interface")
        .unwrap();
    assert_eq!(writer.visibility, Visibility::Private);
}

#[test]
fn fixture_interface_methods_extracted() {
    let ir = parse_fixture();
    let handle = ir
        .symbols
        .iter()
        .find(|s| s.name == "Handle" && s.kind == "method" && s.parent.as_deref() == Some("Handler"))
        .expect("should find Handle method on Handler interface");
    assert_eq!(handle.parent.as_deref(), Some("Handler"));

    let name_method = ir
        .symbols
        .iter()
        .find(|s| s.name == "Name" && s.kind == "method" && s.parent.as_deref() == Some("Handler"))
        .expect("should find Name method on Handler interface");
    assert_eq!(name_method.parent.as_deref(), Some("Handler"));
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_functions() {
    let ir = parse_fixture();
    let funcs: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "func")
        .collect();
    let names: Vec<&str> = funcs.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"NewConfig"));
    assert!(names.contains(&"helper"));
}

#[test]
fn fixture_function_visibility() {
    let ir = parse_fixture();
    let new_config = ir
        .symbols
        .iter()
        .find(|s| s.name == "NewConfig")
        .unwrap();
    assert_eq!(new_config.visibility, Visibility::Public);

    let helper_fn = ir
        .symbols
        .iter()
        .find(|s| s.name == "helper" && s.kind == "func")
        .unwrap();
    assert_eq!(helper_fn.visibility, Visibility::Private);
}

#[test]
fn fixture_function_has_params() {
    let ir = parse_fixture();
    let new_config = ir
        .symbols
        .iter()
        .find(|s| s.name == "NewConfig")
        .unwrap();
    assert_eq!(new_config.params.len(), 2);
    assert_eq!(new_config.params[0].name, "name");
    assert_eq!(new_config.params[1].name, "timeout");
}

#[test]
fn fixture_function_has_return_type() {
    let ir = parse_fixture();
    let new_config = ir
        .symbols
        .iter()
        .find(|s| s.name == "NewConfig")
        .unwrap();
    assert!(new_config.return_type.is_some());
    assert!(new_config.return_type.as_ref().unwrap().contains("Config"));
}

#[test]
fn fixture_function_has_signature() {
    let ir = parse_fixture();
    let new_config = ir
        .symbols
        .iter()
        .find(|s| s.name == "NewConfig")
        .unwrap();
    assert!(new_config.signature.is_some());
    let sig = new_config.signature.as_ref().unwrap();
    assert!(sig.contains("func NewConfig"));
}

// ---------------------------------------------------------------------------
// Methods
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_methods() {
    let ir = parse_fixture();
    let methods: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "method" && s.parent.is_some())
        .filter(|s| {
            // Exclude interface method specs
            let parent = s.parent.as_deref().unwrap_or("");
            parent == "Config" || parent == "Server"
        })
        .collect();
    let names: Vec<&str> = methods.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"GetName"));
    assert!(names.contains(&"addValue"));
    assert!(names.contains(&"Serve"));
}

#[test]
fn fixture_method_has_correct_parent() {
    let ir = parse_fixture();
    let get_name = ir
        .symbols
        .iter()
        .find(|s| s.name == "GetName" && s.kind == "method")
        .unwrap();
    assert_eq!(get_name.parent.as_deref(), Some("Config"));
}

#[test]
fn fixture_method_visibility() {
    let ir = parse_fixture();
    let get_name = ir
        .symbols
        .iter()
        .find(|s| s.name == "GetName" && s.kind == "method")
        .unwrap();
    assert_eq!(get_name.visibility, Visibility::Public);

    let add_value = ir
        .symbols
        .iter()
        .find(|s| s.name == "addValue" && s.kind == "method")
        .unwrap();
    assert_eq!(add_value.visibility, Visibility::Private);
}

#[test]
fn fixture_method_qualified_name() {
    let ir = parse_fixture();
    let get_name = ir
        .symbols
        .iter()
        .find(|s| s.name == "GetName" && s.kind == "method")
        .unwrap();
    assert_eq!(get_name.qualified_name, "sample.Config.GetName");
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_consts() {
    let ir = parse_fixture();
    let consts: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "const")
        .collect();
    let names: Vec<&str> = consts.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"MaxSize"));
    assert!(names.contains(&"ModeRead"));
    assert!(names.contains(&"ModeWrite"));
    assert!(names.contains(&"ModeReadWrite"));
    assert!(names.contains(&"internalVersion"));
}

#[test]
fn fixture_const_visibility() {
    let ir = parse_fixture();
    let max_size = ir
        .symbols
        .iter()
        .find(|s| s.name == "MaxSize")
        .unwrap();
    assert_eq!(max_size.visibility, Visibility::Public);

    let internal = ir
        .symbols
        .iter()
        .find(|s| s.name == "internalVersion")
        .unwrap();
    assert_eq!(internal.visibility, Visibility::Private);
}

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_type_aliases() {
    let ir = parse_fixture();
    let aliases: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "type")
        .collect();
    let names: Vec<&str> = aliases.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"StatusCode"));
    assert!(names.contains(&"Mode"));
}

#[test]
fn fixture_type_alias_has_return_type() {
    let ir = parse_fixture();
    let status_code = ir
        .symbols
        .iter()
        .find(|s| s.name == "StatusCode" && s.kind == "type")
        .unwrap();
    assert_eq!(status_code.return_type.as_deref(), Some("int"));
}
