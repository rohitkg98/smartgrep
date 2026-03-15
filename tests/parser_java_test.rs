use std::path::Path;

use smartgrep::ir::types::*;
use smartgrep::parser::java::parse_file;

fn parse_fixture() -> Ir {
    let source = include_str!("fixtures/Sample.java");
    parse_file(Path::new("tests/fixtures/Sample.java"), source).unwrap()
}

#[test]
fn fixture_has_classes() {
    let ir = parse_fixture();
    let structs: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "class" && s.parent.is_none())
        .collect();
    // Config, InternalHelper, Container (top-level classes)
    assert_eq!(structs.len(), 3);
    let names: Vec<&str> = structs.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Config"));
    assert!(names.contains(&"InternalHelper"));
    assert!(names.contains(&"Container"));
}

#[test]
fn fixture_config_has_fields() {
    let ir = parse_fixture();
    let config = ir
        .symbols
        .iter()
        .find(|s| s.name == "Config" && s.kind == "class")
        .expect("should find Config");
    assert_eq!(config.fields.len(), 3);
    let field_names: Vec<&str> = config.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(field_names.contains(&"name"));
    assert!(field_names.contains(&"values"));
    assert!(field_names.contains(&"timeout"));
}

#[test]
fn fixture_config_field_visibility() {
    let ir = parse_fixture();
    let config = ir
        .symbols
        .iter()
        .find(|s| s.name == "Config" && s.kind == "class")
        .unwrap();
    let name_field = config.fields.iter().find(|f| f.name == "name").unwrap();
    assert_eq!(name_field.visibility, Visibility::Private);
    let values_field = config.fields.iter().find(|f| f.name == "values").unwrap();
    assert_eq!(values_field.visibility, Visibility::Public);
    // timeout has no modifier -> package-private -> Crate
    let timeout_field = config.fields.iter().find(|f| f.name == "timeout").unwrap();
    assert_eq!(timeout_field.visibility, Visibility::Crate);
}

#[test]
fn fixture_has_interface() {
    let ir = parse_fixture();
    let traits: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "interface")
        .collect();
    // Processor<T>, Action (sealed), NestedInterface (inside Container)
    assert_eq!(traits.len(), 3);
    let names: Vec<&str> = traits.iter().map(|s| s.name.as_str()).collect();
    assert!(names.iter().any(|n| n.starts_with("Processor")));
    assert!(names.contains(&"Action"));
    assert!(names.contains(&"NestedInterface"));
}

#[test]
fn fixture_has_enum() {
    let ir = parse_fixture();
    let enums: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "enum")
        .collect();
    assert_eq!(enums.len(), 2);
    let names: Vec<&str> = enums.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Status"));
    assert!(names.contains(&"NestedEnum"));
}

#[test]
fn fixture_has_record() {
    let ir = parse_fixture();
    let point = ir
        .symbols
        .iter()
        .find(|s| s.name == "Point" && s.kind == "record")
        .expect("should find Point record");
    // Record components -> fields and params
    assert_eq!(point.fields.len(), 2);
    assert_eq!(point.params.len(), 2);
    assert_eq!(point.fields[0].name, "x");
    assert_eq!(point.fields[1].name, "y");
}

#[test]
fn fixture_has_methods() {
    let ir = parse_fixture();
    let methods: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "method")
        .collect();
    let method_names: Vec<&str> = methods.iter().map(|s| s.name.as_str()).collect();
    // Config: Config (constructor), getName, addValue, maxSize
    assert!(method_names.contains(&"Config"));
    assert!(method_names.contains(&"getName"));
    assert!(method_names.contains(&"addValue"));
    assert!(method_names.contains(&"maxSize"));
    // Status: label
    assert!(method_names.contains(&"label"));
    // Point: distance
    assert!(method_names.contains(&"distance"));
    // Processor: process, name (interface methods)
    // InternalHelper: InternalHelper (constructor), process, name
}

#[test]
fn fixture_method_has_correct_parent() {
    let ir = parse_fixture();
    let get_name = ir
        .symbols
        .iter()
        .find(|s| s.name == "getName" && s.kind == "method")
        .unwrap();
    assert_eq!(get_name.parent.as_deref(), Some("Config"));
}

#[test]
fn fixture_constructor_is_method() {
    let ir = parse_fixture();
    let ctor = ir
        .symbols
        .iter()
        .find(|s| s.name == "Config" && s.kind == "method")
        .expect("should find Config constructor");
    assert_eq!(ctor.parent.as_deref(), Some("Config"));
    assert_eq!(ctor.params.len(), 1);
    assert_eq!(ctor.params[0].name, "name");
}

#[test]
fn fixture_has_imports() {
    let ir = parse_fixture();
    let imports: Vec<_> = ir
        .dependencies
        .iter()
        .filter(|d| d.kind == DepKind::Import)
        .collect();
    assert_eq!(imports.len(), 2);
    let import_names: Vec<&str> = imports.iter().map(|d| d.to_name.as_str()).collect();
    assert!(import_names.contains(&"java.util.List"));
    assert!(import_names.contains(&"java.io.Serializable"));
}

#[test]
fn fixture_has_trait_impl_deps() {
    let ir = parse_fixture();
    let trait_impls: Vec<_> = ir
        .dependencies
        .iter()
        .filter(|d| d.kind == DepKind::Implements)
        .collect();
    // Config implements Serializable
    // InternalHelper extends Config, implements Processor<String>
    let dep_names: Vec<&str> = trait_impls.iter().map(|d| d.to_name.as_str()).collect();
    assert!(dep_names.contains(&"Serializable"));
    assert!(dep_names.contains(&"Config"));
}

#[test]
fn fixture_qualified_names_use_package() {
    let ir = parse_fixture();
    let config = ir
        .symbols
        .iter()
        .find(|s| s.name == "Config" && s.kind == "class")
        .unwrap();
    assert_eq!(config.qualified_name, "com.example.demo.Config");
}

#[test]
fn fixture_private_method_visibility() {
    let ir = parse_fixture();
    let max_size = ir
        .symbols
        .iter()
        .find(|s| s.name == "maxSize")
        .unwrap();
    assert_eq!(max_size.visibility, Visibility::Private);
}

#[test]
fn fixture_annotations_collected() {
    let ir = parse_fixture();
    let helper = ir
        .symbols
        .iter()
        .find(|s| s.name == "InternalHelper" && s.kind == "class")
        .expect("should find InternalHelper");
    assert!(!helper.attributes.is_empty());
    assert!(helper.attributes[0].contains("Deprecated"));
}

#[test]
fn fixture_method_has_return_type() {
    let ir = parse_fixture();
    let get_name = ir
        .symbols
        .iter()
        .find(|s| s.name == "getName" && s.kind == "method")
        .unwrap();
    assert_eq!(get_name.return_type.as_deref(), Some("String"));
}

#[test]
fn fixture_method_has_params() {
    let ir = parse_fixture();
    let add_value = ir
        .symbols
        .iter()
        .find(|s| s.name == "addValue" && s.kind == "method")
        .unwrap();
    assert_eq!(add_value.params.len(), 1);
    assert_eq!(add_value.params[0].name, "v");
}

#[test]
fn fixture_enum_method_has_parent() {
    let ir = parse_fixture();
    let label = ir
        .symbols
        .iter()
        .find(|s| s.name == "label" && s.parent.as_deref() == Some("Status"))
        .expect("should find label method on Status enum");
    assert_eq!(label.kind, "method");
}

#[test]
fn fixture_class_visibility() {
    let ir = parse_fixture();
    let config = ir
        .symbols
        .iter()
        .find(|s| s.name == "Config" && s.kind == "class")
        .unwrap();
    assert_eq!(config.visibility, Visibility::Public);

    let helper = ir
        .symbols
        .iter()
        .find(|s| s.name == "InternalHelper" && s.kind == "class")
        .unwrap();
    // No public modifier -> package-private -> Crate
    assert_eq!(helper.visibility, Visibility::Crate);
}

// ---------------------------------------------------------------------------
// Nested / inner type tests
// ---------------------------------------------------------------------------

#[test]
fn fixture_inner_records_in_sealed_interface_found() {
    let ir = parse_fixture();
    let create = ir
        .symbols
        .iter()
        .find(|s| s.name == "Create" && s.kind == "record")
        .expect("should find inner record Create");
    assert_eq!(create.kind, "record");

    let delete = ir
        .symbols
        .iter()
        .find(|s| s.name == "Delete" && s.kind == "record")
        .expect("should find inner record Delete");
    assert_eq!(delete.kind, "record");
}

#[test]
fn fixture_inner_record_qualified_name_includes_outer() {
    let ir = parse_fixture();
    let create = ir
        .symbols
        .iter()
        .find(|s| s.name == "Create" && s.kind == "record")
        .unwrap();
    assert_eq!(create.qualified_name, "com.example.demo.Action.Create");

    let delete = ir
        .symbols
        .iter()
        .find(|s| s.name == "Delete" && s.kind == "record")
        .unwrap();
    assert_eq!(delete.qualified_name, "com.example.demo.Action.Delete");
}

#[test]
fn fixture_inner_record_has_parent() {
    let ir = parse_fixture();
    let create = ir
        .symbols
        .iter()
        .find(|s| s.name == "Create" && s.kind == "record")
        .unwrap();
    assert_eq!(create.parent.as_deref(), Some("Action"));

    let delete = ir
        .symbols
        .iter()
        .find(|s| s.name == "Delete" && s.kind == "record")
        .unwrap();
    assert_eq!(delete.parent.as_deref(), Some("Action"));
}

#[test]
fn fixture_inner_record_has_fields_and_params() {
    let ir = parse_fixture();
    let create = ir
        .symbols
        .iter()
        .find(|s| s.name == "Create" && s.kind == "record")
        .unwrap();
    assert_eq!(create.fields.len(), 1);
    assert_eq!(create.fields[0].name, "name");
    assert_eq!(create.params.len(), 1);
    assert_eq!(create.params[0].name, "name");
}

#[test]
fn fixture_inner_record_implements_dep() {
    let ir = parse_fixture();
    let create_impl = ir
        .dependencies
        .iter()
        .find(|d| {
            d.kind == DepKind::Implements
                && d.from_qualified == "com.example.demo.Action.Create"
                && d.to_name == "Action"
        })
        .expect("should find TraitImpl dep from Create to Action");
    assert_eq!(create_impl.kind, DepKind::Implements);
}

#[test]
fn fixture_inner_class_in_class() {
    let ir = parse_fixture();
    let nested = ir
        .symbols
        .iter()
        .find(|s| s.name == "Nested" && s.kind == "class")
        .expect("should find inner class Nested");
    assert_eq!(nested.parent.as_deref(), Some("Container"));
    assert_eq!(nested.qualified_name, "com.example.demo.Container.Nested");
}

#[test]
fn fixture_inner_class_method_found() {
    let ir = parse_fixture();
    let method = ir
        .symbols
        .iter()
        .find(|s| s.name == "nestedMethod" && s.kind == "method")
        .expect("should find nestedMethod in inner class Nested");
    assert_eq!(method.parent.as_deref(), Some("Nested"));
}

#[test]
fn fixture_inner_enum_in_class() {
    let ir = parse_fixture();
    let nested_enum = ir
        .symbols
        .iter()
        .find(|s| s.name == "NestedEnum" && s.kind == "enum")
        .expect("should find inner enum NestedEnum");
    assert_eq!(nested_enum.parent.as_deref(), Some("Container"));
    assert_eq!(nested_enum.qualified_name, "com.example.demo.Container.NestedEnum");
}

#[test]
fn fixture_inner_interface_in_class() {
    let ir = parse_fixture();
    let nested_iface = ir
        .symbols
        .iter()
        .find(|s| s.name == "NestedInterface" && s.kind == "interface")
        .expect("should find inner interface NestedInterface");
    assert_eq!(nested_iface.parent.as_deref(), Some("Container"));
    assert_eq!(nested_iface.qualified_name, "com.example.demo.Container.NestedInterface");
}

#[test]
fn fixture_inner_record_in_class() {
    let ir = parse_fixture();
    let nested_rec = ir
        .symbols
        .iter()
        .find(|s| s.name == "NestedRecord" && s.kind == "record")
        .expect("should find inner record NestedRecord");
    assert_eq!(nested_rec.parent.as_deref(), Some("Container"));
    assert_eq!(nested_rec.qualified_name, "com.example.demo.Container.NestedRecord");
    assert_eq!(nested_rec.params.len(), 1);
    assert_eq!(nested_rec.params[0].name, "val");
}
