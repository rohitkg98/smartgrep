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
    // `import { EventEmitter } from 'events'` is recorded per name;
    // `import * as path from 'path'` stays module-level.
    assert!(names.contains(&"events/EventEmitter"));
    assert!(names.contains(&"path"));
}

fn import_targets(src: &str) -> Vec<String> {
    let ir = parse_file(Path::new("src/app/main.ts"), src).unwrap();
    ir.dependencies
        .iter()
        .filter(|d| d.kind == DepKind::Import)
        .map(|d| d.to_name.clone())
        .collect()
}

#[test]
fn named_imports_one_dep_per_original_name() {
    assert_eq!(
        import_targets("import { A, B as C } from '../models';\n"),
        vec!["../models/A", "../models/B"]
    );
}

#[test]
fn default_namespace_and_side_effect_imports() {
    assert_eq!(import_targets("import Def from './def';\n"), vec!["./def/Def"]);
    assert_eq!(import_targets("import * as ns from 'ns-mod';\n"), vec!["ns-mod"]);
    assert_eq!(import_targets("import './polyfill';\n"), vec!["./polyfill"]);
    assert_eq!(
        import_targets("import React, { useState } from 'react';\n"),
        vec!["react/React", "react/useState"]
    );
}

#[test]
fn type_only_imports_are_imports() {
    assert_eq!(
        import_targets("import type { T1 } from './types';\nimport { type T2, D } from './types';\n"),
        vec!["./types/T1", "./types/T2", "./types/D"]
    );
}

#[test]
fn import_module_extension_dropped_from_per_name_target() {
    assert_eq!(import_targets("import { User } from './user.js';\n"), vec!["./user/User"]);
}

#[test]
fn named_import_targets_normalize_to_imported_name() {
    use smartgrep::ir::names::{dep_matches, dep_target_key};
    assert_eq!(dep_target_key("../models/User"), "User");
    assert_eq!(dep_target_key("@scope/pkg/Thing"), "Thing");
    assert!(dep_matches("../models/User", "models/User"));
    assert!(dep_matches("../models/User", "models.User"));
    assert!(!dep_matches("../models/User", "other/User"));
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

// ---------------------------------------------------------------------------
// Call deps
// ---------------------------------------------------------------------------

fn calls_of<'a>(ir: &'a Ir, from: &str) -> Vec<&'a str> {
    ir.dependencies
        .iter()
        .filter(|d| d.kind == DepKind::Call && d.from_qualified == from)
        .map(|d| d.to_name.as_str())
        .collect()
}

const RUN: &str = "services.Dispatcher.run";

#[test]
fn call_deps_full_list_for_method() {
    let ir = parse_fixture();
    assert_eq!(
        calls_of(&ir, RUN),
        vec![
            "helper",
            "path.join",
            "Math.max",
            "JSON.stringify",
            "push",
            "log",
            "Map",
            "Validation.Checker",
            "forEach",
            "process",
            "identity",
            "build",
            "finish",
            "emit",
        ]
    );
}

#[test]
fn call_plain_and_namespace_qualified() {
    let ir = parse_fixture();
    let calls = calls_of(&ir, RUN);
    assert!(calls.contains(&"helper"));
    // `path` is an `import * as path` binding; Math/JSON are PascalCase-ish globals
    assert!(calls.contains(&"path.join"));
    assert!(calls.contains(&"Math.max"));
    assert!(calls.contains(&"JSON.stringify"));
}

#[test]
fn call_instance_receiver_reduced_to_method_name() {
    let ir = parse_fixture();
    let calls = calls_of(&ir, RUN);
    for name in ["push", "log", "finish", "emit"] {
        assert!(calls.contains(&name), "missing {}", name);
    }
    assert!(!calls.iter().any(|c| c.starts_with("this") || c.starts_with("console")));
    assert_eq!(calls_of(&ir, "services.Dispatcher.constructor"), vec!["setup"]);
}

#[test]
fn call_constructor_and_generics_stripped() {
    let ir = parse_fixture();
    let calls = calls_of(&ir, RUN);
    assert!(calls.contains(&"Map"));
    assert!(calls.contains(&"Validation.Checker"));
    assert!(calls.contains(&"identity")); // identity<string>(...)
    assert!(!calls.iter().any(|c| c.contains('<')));
    assert_eq!(calls_of(&ir, "services.fetchUser"), vec!["Promise.resolve", "UserService"]);
}

#[test]
fn call_in_arrow_and_nested_function_attributed_to_enclosing() {
    let ir = parse_fixture();
    assert!(calls_of(&ir, RUN).contains(&"process")); // xs.forEach((x) => this.process(x))
    // runAll is an arrow-function const: its body is walked, nested fn attributed to it
    assert_eq!(calls_of(&ir, "services.runAll"), vec!["forEach", "run", "greet", "nestedCall"]);
}

#[test]
fn call_deps_deduped_first_loc_kept() {
    let ir = parse_fixture();
    let helper: Vec<_> = ir
        .dependencies
        .iter()
        .filter(|d| d.kind == DepKind::Call && d.from_qualified == RUN && d.to_name == "helper")
        .collect();
    assert_eq!(helper.len(), 1);
    assert_eq!(helper[0].loc.line, 154);
}

#[test]
fn super_call_skipped_and_no_module_level_calls() {
    let ir = parse_fixture();
    assert!(!ir.dependencies.iter().any(|d| d.kind == DepKind::Call && d.to_name == "super"));
    // `new Map()` / `registry.set()` at module level have no enclosing function
    assert!(!ir.dependencies.iter().any(|d| d.kind == DepKind::Call && d.to_name == "set"));
    let froms: Vec<&str> = ir
        .dependencies
        .iter()
        .filter(|d| d.kind == DepKind::Call)
        .map(|d| d.from_qualified.as_str())
        .collect();
    for f in froms {
        let sym = ir.symbols.iter().find(|s| s.qualified_name == f).expect(f);
        assert!(sym.kind == "function" || sym.kind == "method", "{} is {}", f, sym.kind);
    }
}

#[test]
fn tsx_calls() {
    let src = "import React from 'react';\nexport function App() {\n  const [x, setX] = useState(0);\n  return <div onClick={() => setX(inc(x))}>{React.createElement('b')}</div>;\n}\n";
    let ir = parse_file(Path::new("src/App.tsx"), src).unwrap();
    assert_eq!(
        calls_of(&ir, "App"),
        vec!["useState", "setX", "inc", "React.createElement"]
    );
}
