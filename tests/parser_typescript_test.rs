use std::path::Path;

use smartgrep::ir::types::*;
use smartgrep::parser::typescript::parse_file;

fn parse_fixture() -> Ir {
    let source = include_str!("fixtures/Sample.ts");
    parse_file(Path::new("src/services/Sample.ts"), source).unwrap()
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
    assert!(names.contains(&"events"));
    assert!(names.contains(&"path"));
}

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_type_aliases() {
    let ir = parse_fixture();
    let types: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "type")
        .collect();
    let names: Vec<&str> = types.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Status"));
    assert!(names.contains(&"UserId"));
}

#[test]
fn exported_type_is_public() {
    let ir = parse_fixture();
    let status = ir.symbols.iter().find(|s| s.name == "Status").unwrap();
    assert_eq!(status.visibility, Visibility::Public);
}

#[test]
fn unexported_type_is_private() {
    let ir = parse_fixture();
    let user_id = ir.symbols.iter().find(|s| s.name == "UserId").unwrap();
    assert_eq!(user_id.visibility, Visibility::Private);
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_enums() {
    let ir = parse_fixture();
    let enums: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "enum")
        .collect();
    let names: Vec<&str> = enums.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Color"));
    assert!(names.contains(&"Direction"));
}

#[test]
fn const_enum_has_attribute() {
    let ir = parse_fixture();
    let direction = ir.symbols.iter().find(|s| s.name == "Direction").unwrap();
    assert!(direction.attributes.contains(&"const".to_string()));
}

#[test]
fn exported_enum_is_public() {
    let ir = parse_fixture();
    let color = ir.symbols.iter().find(|s| s.name == "Color").unwrap();
    assert_eq!(color.visibility, Visibility::Public);
}

// ---------------------------------------------------------------------------
// Interfaces
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_interfaces() {
    let ir = parse_fixture();
    let interfaces: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "interface")
        .collect();
    let names: Vec<&str> = interfaces.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Serializable"));
    assert!(names.contains(&"Config"));
    assert!(names.contains(&"Repository"));
}

#[test]
fn interface_has_method_signatures() {
    let ir = parse_fixture();
    let methods: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "method" && s.parent.as_deref() == Some("Serializable"))
        .collect();
    let names: Vec<&str> = methods.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"serialize"));
    assert!(names.contains(&"deserialize"));
}

#[test]
fn interface_has_property_signatures() {
    let ir = parse_fixture();
    let config = ir.symbols.iter().find(|s| s.name == "Config").unwrap();
    let field_names: Vec<&str> = config.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(field_names.contains(&"host"));
    assert!(field_names.contains(&"port"));
    assert!(field_names.contains(&"debug"));
}

#[test]
fn interface_extends_generates_dep() {
    let ir = parse_fixture();
    let extends: Vec<_> = ir
        .dependencies
        .iter()
        .filter(|d| d.kind == DepKind::Implements && d.from_qualified.contains("Repository"))
        .collect();
    assert!(!extends.is_empty());
    assert!(extends.iter().any(|d| d.to_name == "Serializable"));
}

// ---------------------------------------------------------------------------
// Classes
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_classes() {
    let ir = parse_fixture();
    let classes: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "class")
        .collect();
    let names: Vec<&str> = classes.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"UserService"));
    assert!(names.contains(&"BaseRepository"));
    assert!(names.contains(&"InternalHelper"));
}

#[test]
fn class_has_fields() {
    let ir = parse_fixture();
    let user_service = ir.symbols.iter().find(|s| s.name == "UserService").unwrap();
    let field_names: Vec<&str> = user_service.fields.iter().map(|f| f.name.as_str()).collect();
    assert!(field_names.contains(&"id"));
    assert!(field_names.contains(&"name"));
    assert!(field_names.contains(&"email"));
    assert!(field_names.contains(&"createdAt"));
}

#[test]
fn class_field_visibility() {
    let ir = parse_fixture();
    let user_service = ir.symbols.iter().find(|s| s.name == "UserService").unwrap();

    let id_field = user_service.fields.iter().find(|f| f.name == "id").unwrap();
    assert_eq!(id_field.visibility, Visibility::Private);

    let name_field = user_service.fields.iter().find(|f| f.name == "name").unwrap();
    assert_eq!(name_field.visibility, Visibility::Public);

    let email_field = user_service.fields.iter().find(|f| f.name == "email").unwrap();
    assert_eq!(email_field.visibility, Visibility::Crate); // protected
}

#[test]
fn class_has_methods() {
    let ir = parse_fixture();
    let methods: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "method" && s.parent.as_deref() == Some("UserService"))
        .collect();
    let names: Vec<&str> = methods.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"constructor"));
    assert!(names.contains(&"serialize"));
    assert!(names.contains(&"getName"));
    assert!(names.contains(&"validate"));
    assert!(names.contains(&"create"));
}

#[test]
fn class_method_visibility() {
    let ir = parse_fixture();

    let get_name = ir.symbols.iter()
        .find(|s| s.name == "getName" && s.parent.as_deref() == Some("UserService"))
        .unwrap();
    assert_eq!(get_name.visibility, Visibility::Public);

    let validate = ir.symbols.iter()
        .find(|s| s.name == "validate" && s.parent.as_deref() == Some("UserService"))
        .unwrap();
    assert_eq!(validate.visibility, Visibility::Private);
}

#[test]
fn static_method_has_attribute() {
    let ir = parse_fixture();
    let create = ir.symbols.iter()
        .find(|s| s.name == "create" && s.parent.as_deref() == Some("UserService"))
        .unwrap();
    assert!(create.attributes.contains(&"static".to_string()));
}

#[test]
fn abstract_class_has_attribute() {
    let ir = parse_fixture();
    let base = ir.symbols.iter().find(|s| s.name == "BaseRepository").unwrap();
    assert!(base.attributes.contains(&"abstract".to_string()));
}

#[test]
fn abstract_methods_have_attribute() {
    let ir = parse_fixture();
    let find_by_id = ir.symbols.iter()
        .find(|s| s.name == "findById" && s.parent.as_deref() == Some("BaseRepository"))
        .unwrap();
    assert!(find_by_id.attributes.contains(&"abstract".to_string()));
}

#[test]
fn unexported_class_is_private() {
    let ir = parse_fixture();
    let helper = ir.symbols.iter().find(|s| s.name == "InternalHelper").unwrap();
    assert_eq!(helper.visibility, Visibility::Private);
}

#[test]
fn class_decorator_captured() {
    let ir = parse_fixture();
    let user_service = ir.symbols.iter().find(|s| s.name == "UserService").unwrap();
    assert!(user_service.attributes.iter().any(|a| a.contains("Injectable")));
}

#[test]
fn method_decorator_captured() {
    let ir = parse_fixture();
    let get_name = ir.symbols.iter()
        .find(|s| s.name == "getName" && s.parent.as_deref() == Some("UserService"))
        .unwrap();
    assert!(get_name.attributes.iter().any(|a| a.contains("Log")));
}

#[test]
fn class_extends_generates_dep() {
    let ir = parse_fixture();
    let extends: Vec<_> = ir
        .dependencies
        .iter()
        .filter(|d| d.kind == DepKind::Implements && d.from_qualified.contains("UserService"))
        .collect();
    let names: Vec<&str> = extends.iter().map(|d| d.to_name.as_str()).collect();
    assert!(names.contains(&"EventEmitter"));
    assert!(names.contains(&"Serializable"));
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_functions() {
    let ir = parse_fixture();
    let functions: Vec<_> = ir
        .symbols
        .iter()
        .filter(|s| s.kind == "function" && s.parent.is_none())
        .collect();
    let names: Vec<&str> = functions.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"greet"));
    assert!(names.contains(&"helper"));
}

#[test]
fn exported_function_is_public() {
    let ir = parse_fixture();
    let greet = ir.symbols.iter().find(|s| s.name == "greet").unwrap();
    assert_eq!(greet.visibility, Visibility::Public);
    assert_eq!(greet.kind, "function");
}

#[test]
fn unexported_function_is_private() {
    let ir = parse_fixture();
    let helper = ir.symbols.iter()
        .find(|s| s.name == "helper" && s.kind == "function")
        .unwrap();
    assert_eq!(helper.visibility, Visibility::Private);
}

#[test]
fn function_has_params() {
    let ir = parse_fixture();
    let greet = ir.symbols.iter().find(|s| s.name == "greet").unwrap();
    assert_eq!(greet.params.len(), 1);
    assert_eq!(greet.params[0].name, "name");
    assert_eq!(greet.params[0].type_name, "string");
}

#[test]
fn function_has_return_type() {
    let ir = parse_fixture();
    let greet = ir.symbols.iter().find(|s| s.name == "greet").unwrap();
    assert_eq!(greet.return_type.as_deref(), Some("string"));
}

// ---------------------------------------------------------------------------
// Arrow functions
// ---------------------------------------------------------------------------

#[test]
fn arrow_functions_are_captured_as_functions() {
    let ir = parse_fixture();
    let fetch_user = ir.symbols.iter().find(|s| s.name == "fetchUser").unwrap();
    assert_eq!(fetch_user.kind, "function");
    assert_eq!(fetch_user.visibility, Visibility::Public);
}

#[test]
fn arrow_function_has_params() {
    let ir = parse_fixture();
    let fetch_user = ir.symbols.iter().find(|s| s.name == "fetchUser").unwrap();
    assert_eq!(fetch_user.params.len(), 1);
    assert_eq!(fetch_user.params[0].name, "id");
}

#[test]
fn unexported_arrow_function_is_private() {
    let ir = parse_fixture();
    let util = ir.symbols.iter().find(|s| s.name == "internalUtil").unwrap();
    assert_eq!(util.visibility, Visibility::Private);
    assert_eq!(util.kind, "function");
}

// ---------------------------------------------------------------------------
// Consts (non-function)
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
    assert!(names.contains(&"MAX_RETRIES"));
    assert!(names.contains(&"DEFAULT_PORT"));
}

#[test]
fn exported_const_is_public() {
    let ir = parse_fixture();
    let max = ir.symbols.iter().find(|s| s.name == "MAX_RETRIES").unwrap();
    assert_eq!(max.visibility, Visibility::Public);
}

// ---------------------------------------------------------------------------
// Namespaces
// ---------------------------------------------------------------------------

#[test]
fn fixture_has_namespace() {
    let ir = parse_fixture();
    let ns = ir.symbols.iter().find(|s| s.kind == "namespace").unwrap();
    assert_eq!(ns.name, "Validation");
    assert_eq!(ns.visibility, Visibility::Public);
}

#[test]
fn namespace_inner_function_has_parent() {
    let ir = parse_fixture();
    let is_valid = ir.symbols.iter()
        .find(|s| s.name == "isValid" && s.kind == "function")
        .unwrap();
    assert_eq!(is_valid.parent.as_deref(), Some("Validation"));
}

#[test]
fn namespace_inner_interface() {
    let ir = parse_fixture();
    let validator = ir.symbols.iter()
        .find(|s| s.name == "Validator" && s.kind == "interface")
        .unwrap();
    assert!(validator.qualified_name.contains("Validation"));
}

// ---------------------------------------------------------------------------
// Qualified names
// ---------------------------------------------------------------------------

#[test]
fn qualified_names_use_module_prefix() {
    // parse_fixture uses path "src/services/Sample.ts" → prefix "services"
    let ir = parse_fixture();
    let greet = ir.symbols.iter().find(|s| s.name == "greet").unwrap();
    assert_eq!(greet.qualified_name, "services.greet");
}

#[test]
fn class_qualified_name() {
    let ir = parse_fixture();
    let user_service = ir.symbols.iter().find(|s| s.name == "UserService").unwrap();
    assert_eq!(user_service.qualified_name, "services.UserService");
}

#[test]
fn method_qualified_name() {
    let ir = parse_fixture();
    let get_name = ir.symbols.iter()
        .find(|s| s.name == "getName" && s.parent.as_deref() == Some("UserService"))
        .unwrap();
    assert_eq!(get_name.qualified_name, "services.UserService.getName");
}

#[test]
fn namespace_function_qualified_name() {
    let ir = parse_fixture();
    let is_valid = ir.symbols.iter()
        .find(|s| s.name == "isValid" && s.kind == "function")
        .unwrap();
    assert_eq!(is_valid.qualified_name, "services.Validation.isValid");
}
