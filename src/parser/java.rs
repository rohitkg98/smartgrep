use std::path::Path;

use anyhow::Result;
use tree_sitter::{Node, Parser};

use crate::ir::types::*;
use crate::parser::common::{find_child_by_kind, loc, node_text};

/// Derive a qualified package prefix from a Java file.
/// If the file contains a `package` declaration, we use that.
/// Otherwise we derive from the file path:
///   `src/main/java/com/example/Foo.java` -> `com.example`
fn package_prefix_from_path(path: &Path) -> String {
    let path_str = path.to_string_lossy();

    // Strip common Java source roots
    let stripped = path_str
        .strip_prefix("src/main/java/")
        .or_else(|| path_str.strip_prefix("src/test/java/"))
        .or_else(|| path_str.strip_prefix("src/"))
        .unwrap_or(&path_str);

    // Remove .java extension
    let without_ext = stripped.strip_suffix(".java").unwrap_or(stripped);

    // Take directory part (drop the filename)
    if let Some(pos) = without_ext.rfind('/') {
        without_ext[..pos].replace('/', ".")
    } else {
        String::new()
    }
}

/// Parse a Java source file and return the IR.
pub fn parse_file(path: &Path, source: &str) -> Result<Ir> {
    let mut parser = Parser::new();
    let language = tree_sitter_java::LANGUAGE;
    parser.set_language(&language.into())?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter failed to parse {}", path.display()))?;

    let mut ir = Ir::default();

    // First pass: extract the package declaration if present
    let prefix = extract_package(tree.root_node(), source)
        .unwrap_or_else(|| package_prefix_from_path(path));

    extract_items(tree.root_node(), source, path, &prefix, None, &mut ir);

    Ok(ir)
}

/// Extract package declaration from the root node.
fn extract_package(root: Node, source: &str) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "package_declaration" {
            // The package name is the scoped_identifier or identifier child
            let mut inner = child.walk();
            for c in child.children(&mut inner) {
                match c.kind() {
                    "scoped_identifier" | "identifier" => {
                        return Some(node_text(&c, source).to_string());
                    }
                    _ => {}
                }
            }
        }
    }
    None
}

fn extract_items(
    node: Node,
    source: &str,
    path: &Path,
    prefix: &str,
    _parent: Option<&str>,
    ir: &mut Ir,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "package_declaration" => {
                // Already handled in extract_package
            }
            "import_declaration" => {
                if let Some(dep) = extract_import(&child, source, path, prefix) {
                    ir.dependencies.push(dep);
                }
            }
            "class_declaration" => {
                extract_class(&child, source, path, prefix, None, ir);
            }
            "interface_declaration" => {
                extract_interface(&child, source, path, prefix, None, ir);
            }
            "enum_declaration" => {
                extract_enum(&child, source, path, prefix, None, ir);
            }
            "record_declaration" => {
                extract_record(&child, source, path, prefix, None, ir);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

fn get_name(node: &Node, source: &str) -> Option<String> {
    node.child_by_field_name("name")
        .map(|n| node_text(&n, source).to_string())
}

/// Get the identifier child node's text (used when there's no "name" field).
fn get_identifier(node: &Node, source: &str) -> Option<String> {
    find_child_by_kind(node, "identifier")
        .map(|n| node_text(&n, source).to_string())
}

// ---------------------------------------------------------------------------
// Modifiers: visibility + annotations
// ---------------------------------------------------------------------------

/// Extract visibility and annotation list from a `modifiers` child.
fn extract_modifiers(node: &Node, source: &str) -> (Visibility, Vec<String>) {
    let mut vis = Visibility::Crate; // Java default (package-private)
    let mut attrs = Vec::new();

    if let Some(mods) = find_child_by_kind(node, "modifiers") {
        let mut cursor = mods.walk();
        for child in mods.children(&mut cursor) {
            match child.kind() {
                "public" => vis = Visibility::Public,
                "private" => vis = Visibility::Private,
                "protected" => vis = Visibility::Crate,
                "marker_annotation" | "annotation" => {
                    attrs.push(node_text(&child, source).to_string());
                }
                _ => {} // static, final, abstract, etc. — we skip
            }
        }
    }
    (vis, attrs)
}

// ---------------------------------------------------------------------------
// Imports
// ---------------------------------------------------------------------------

fn extract_import(node: &Node, source: &str, path: &Path, prefix: &str) -> Option<Dependency> {
    let text = node_text(node, source).to_string();
    let import_path = text
        .strip_prefix("import ")
        .unwrap_or(&text)
        .trim_end_matches(';')
        .trim()
        .to_string();

    // Strip "static " prefix if present (static imports)
    let import_path = import_path
        .strip_prefix("static ")
        .unwrap_or(&import_path)
        .to_string();

    Some(Dependency {
        from_qualified: prefix.to_string(),
        to_name: import_path,
        kind: DepKind::Import,
        loc: loc(node, path),
    })
}

// ---------------------------------------------------------------------------
// Class
// ---------------------------------------------------------------------------

fn extract_class(
    node: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    outer_name: Option<&str>,
    ir: &mut Ir,
) {
    let name = match get_name(node, source) {
        Some(n) => n,
        None => return,
    };
    let (vis, attrs) = extract_modifiers(node, source);
    let qualified_name = if prefix.is_empty() {
        name.clone()
    } else {
        format!("{}.{}", prefix, name)
    };

    let fields = extract_class_fields(node, source);

    // Extract extends/implements dependencies
    extract_superclass_deps(node, source, path, &qualified_name, ir);
    extract_super_interfaces_deps(node, source, path, &qualified_name, ir);

    let mut sym = Symbol::new(name.clone(), qualified_name.clone(), "class", loc(node, path), vis);
    sym.parent = outer_name.map(|s| s.to_string());
    sym.attributes = attrs;
    sym.fields = fields;
    ir.symbols.push(sym);

    // Extract methods and constructors from class body
    if let Some(body) = find_child_by_kind(node, "class_body") {
        extract_class_body_members(&body, source, path, prefix, &name, &qualified_name, ir);
    }
}

// ---------------------------------------------------------------------------
// Interface
// ---------------------------------------------------------------------------

fn extract_interface(
    node: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    outer_name: Option<&str>,
    ir: &mut Ir,
) {
    let name = match get_name(node, source) {
        Some(n) => n,
        None => return,
    };
    let (vis, attrs) = extract_modifiers(node, source);
    let qualified_name = if prefix.is_empty() {
        name.clone()
    } else {
        format!("{}.{}", prefix, name)
    };

    // Include type parameters in name if present
    let display_name = if let Some(tp) = find_child_by_kind(node, "type_parameters") {
        format!("{}{}", name, node_text(&tp, source))
    } else {
        name.clone()
    };

    let mut sym = Symbol::new(display_name, qualified_name.clone(), "interface", loc(node, path), vis);
    sym.parent = outer_name.map(|s| s.to_string());
    sym.attributes = attrs;
    ir.symbols.push(sym);

    // Extract interface methods
    if let Some(body) = find_child_by_kind(node, "interface_body") {
        extract_interface_body_members(&body, source, path, prefix, &name, &qualified_name, ir);
    }
}

// ---------------------------------------------------------------------------
// Enum
// ---------------------------------------------------------------------------

fn extract_enum(
    node: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    outer_name: Option<&str>,
    ir: &mut Ir,
) {
    let name = match get_name(node, source) {
        Some(n) => n,
        None => return,
    };
    let (vis, attrs) = extract_modifiers(node, source);
    let qualified_name = if prefix.is_empty() {
        name.clone()
    } else {
        format!("{}.{}", prefix, name)
    };

    // Extract super interfaces for enum
    extract_super_interfaces_deps(node, source, path, &qualified_name, ir);

    let mut sym = Symbol::new(name.clone(), qualified_name.clone(), "enum", loc(node, path), vis);
    sym.parent = outer_name.map(|s| s.to_string());
    sym.attributes = attrs;
    ir.symbols.push(sym);

    // Extract methods from enum body declarations
    if let Some(body) = find_child_by_kind(node, "enum_body") {
        // Methods live inside enum_body_declarations
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "enum_body_declarations" {
                extract_class_body_members(&child, source, path, prefix, &name, &qualified_name, ir);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Record
// ---------------------------------------------------------------------------

fn extract_record(
    node: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    outer_name: Option<&str>,
    ir: &mut Ir,
) {
    let name = match get_name(node, source) {
        Some(n) => n,
        None => return,
    };
    let (vis, attrs) = extract_modifiers(node, source);
    let qualified_name = if prefix.is_empty() {
        name.clone()
    } else {
        format!("{}.{}", prefix, name)
    };

    // Record components become both fields and params
    let components = extract_formal_params(node, source);
    let fields: Vec<Field> = components
        .iter()
        .map(|p| Field {
            name: p.name.clone(),
            type_name: p.type_name.clone(),
            visibility: Visibility::Private,
        })
        .collect();

    // Extract super interfaces for record
    extract_super_interfaces_deps(node, source, path, &qualified_name, ir);

    let mut sym = Symbol::new(name.clone(), qualified_name.clone(), "record", loc(node, path), vis);
    sym.parent = outer_name.map(|s| s.to_string());
    sym.attributes = attrs;
    sym.fields = fields;
    sym.params = components;
    ir.symbols.push(sym);

    // Extract methods from record body (uses class_body)
    if let Some(body) = find_child_by_kind(node, "class_body") {
        extract_class_body_members(&body, source, path, prefix, &name, &qualified_name, ir);
    }
}

// ---------------------------------------------------------------------------
// Class/interface body member extraction
// ---------------------------------------------------------------------------

fn extract_class_body_members(
    body: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    parent_name: &str,
    parent_qualified: &str,
    ir: &mut Ir,
) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        match child.kind() {
            "method_declaration" => {
                if let Some(sym) = extract_method(&child, source, path, prefix, parent_name) {
                    ir.symbols.push(sym);
                }
            }
            "constructor_declaration" => {
                if let Some(sym) = extract_constructor(&child, source, path, prefix, parent_name) {
                    ir.symbols.push(sym);
                }
            }
            "class_declaration" => {
                extract_class(&child, source, path, parent_qualified, Some(parent_name), ir);
            }
            "interface_declaration" => {
                extract_interface(&child, source, path, parent_qualified, Some(parent_name), ir);
            }
            "enum_declaration" => {
                extract_enum(&child, source, path, parent_qualified, Some(parent_name), ir);
            }
            "record_declaration" => {
                extract_record(&child, source, path, parent_qualified, Some(parent_name), ir);
            }
            _ => {}
        }
    }
}

fn extract_interface_body_members(
    body: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    parent_name: &str,
    parent_qualified: &str,
    ir: &mut Ir,
) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        match child.kind() {
            "method_declaration" => {
                if let Some(sym) = extract_method(&child, source, path, prefix, parent_name) {
                    ir.symbols.push(sym);
                }
            }
            "class_declaration" => {
                extract_class(&child, source, path, parent_qualified, Some(parent_name), ir);
            }
            "interface_declaration" => {
                extract_interface(&child, source, path, parent_qualified, Some(parent_name), ir);
            }
            "enum_declaration" => {
                extract_enum(&child, source, path, parent_qualified, Some(parent_name), ir);
            }
            "record_declaration" => {
                extract_record(&child, source, path, parent_qualified, Some(parent_name), ir);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Method & Constructor
// ---------------------------------------------------------------------------

fn extract_method(
    node: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    parent_name: &str,
) -> Option<Symbol> {
    let name = get_name(node, source)
        .or_else(|| get_identifier(node, source))?;
    let (vis, attrs) = extract_modifiers(node, source);
    let qualified_name = if prefix.is_empty() {
        format!("{}.{}", parent_name, name)
    } else {
        format!("{}.{}.{}", prefix, parent_name, name)
    };

    let params = extract_formal_params(node, source);
    let return_type = extract_return_type(node, source);
    let sig = build_method_signature(node, source);

    let mut sym = Symbol::new(name, qualified_name, "method", loc(node, path), vis);
    sym.signature = Some(sig);
    sym.parent = Some(parent_name.to_string());
    sym.attributes = attrs;
    sym.params = params;
    sym.return_type = return_type;
    Some(sym)
}

fn extract_constructor(
    node: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    parent_name: &str,
) -> Option<Symbol> {
    let name = get_name(node, source)
        .or_else(|| get_identifier(node, source))?;
    let (vis, attrs) = extract_modifiers(node, source);
    let qualified_name = if prefix.is_empty() {
        format!("{}.{}", parent_name, name)
    } else {
        format!("{}.{}.{}", prefix, parent_name, name)
    };

    let params = extract_formal_params(node, source);
    let sig = build_method_signature(node, source);

    let mut sym = Symbol::new(name, qualified_name, "method", loc(node, path), vis);
    sym.signature = Some(sig);
    sym.parent = Some(parent_name.to_string());
    sym.attributes = attrs;
    sym.params = params;
    Some(sym)
}

// ---------------------------------------------------------------------------
// Parameter & field extraction
// ---------------------------------------------------------------------------

fn extract_formal_params(node: &Node, source: &str) -> Vec<Param> {
    let mut params = Vec::new();
    if let Some(param_list) = find_child_by_kind(node, "formal_parameters") {
        let mut cursor = param_list.walk();
        for child in param_list.children(&mut cursor) {
            if child.kind() == "formal_parameter" {
                let type_name = extract_type_from_param(&child, source);
                let name = find_child_by_kind(&child, "identifier")
                    .map(|n| node_text(&n, source).to_string())
                    .unwrap_or_default();
                if !name.is_empty() {
                    params.push(Param { name, type_name });
                }
            }
        }
    }
    params
}

/// Extract the type text from a formal_parameter node.
/// The type can be many things: type_identifier, integral_type, generic_type, array_type, etc.
/// We take all children except the last identifier (which is the param name).
fn extract_type_from_param(node: &Node, source: &str) -> String {
    let child_count = node.child_count();
    if child_count < 2 {
        return String::new();
    }
    // The last child is the identifier (name). Everything before it except modifiers is the type.
    let mut parts = Vec::new();
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();
    for child in &children[..children.len() - 1] {
        if child.kind() != "modifiers" {
            parts.push(node_text(child, source));
        }
    }
    parts.join(" ")
}

fn extract_class_fields(node: &Node, source: &str) -> Vec<Field> {
    let mut fields = Vec::new();
    if let Some(body) = find_child_by_kind(node, "class_body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "field_declaration" {
                if let Some(f) = extract_field(&child, source) {
                    fields.push(f);
                }
            }
        }
    }
    fields
}

fn extract_field(node: &Node, source: &str) -> Option<Field> {
    let (vis, _attrs) = extract_modifiers(node, source);

    // Type is the second meaningful child (after modifiers)
    // variable_declarator contains the name
    let type_name = extract_field_type(node, source);
    let name = find_child_by_kind(node, "variable_declarator")
        .and_then(|vd| {
            // Can't use find_child_by_kind here due to lifetime — iterate manually
            for i in 0..vd.child_count() {
                if let Some(c) = vd.child(i) {
                    if c.kind() == "identifier" {
                        return Some(node_text(&c, source).to_string());
                    }
                }
            }
            None
        })
        .unwrap_or_default();

    if name.is_empty() {
        return None;
    }

    Some(Field {
        name,
        type_name,
        visibility: vis,
    })
}

/// Extract the type from a field_declaration node.
/// The type node comes after modifiers and before variable_declarator.
fn extract_field_type(node: &Node, source: &str) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "modifiers" | "variable_declarator" | ";" => continue,
            _ => {
                let text = node_text(&child, source);
                if !text.is_empty() {
                    return text.to_string();
                }
            }
        }
    }
    String::new()
}

/// Extract return type from a method_declaration.
fn extract_return_type(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            // Skip modifiers, the method name (identifier), formal_parameters, block, etc.
            "modifiers" | "identifier" | "formal_parameters" | "block" | ";" | "throws" => continue,
            // These are the return type nodes
            "void_type" => return Some("void".to_string()),
            "type_identifier" | "generic_type" | "array_type" | "integral_type"
            | "floating_point_type" | "boolean_type" | "scoped_type_identifier" => {
                return Some(node_text(&child, source).to_string());
            }
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Extends / Implements dependencies
// ---------------------------------------------------------------------------

fn extract_superclass_deps(
    node: &Node,
    source: &str,
    path: &Path,
    qualified_name: &str,
    ir: &mut Ir,
) {
    if let Some(superclass) = find_child_by_kind(node, "superclass") {
        // The type_identifier is the superclass name
        if let Some(type_id) = find_child_by_kind(&superclass, "type_identifier") {
            ir.dependencies.push(Dependency {
                from_qualified: qualified_name.to_string(),
                to_name: node_text(&type_id, source).to_string(),
                kind: DepKind::Implements,
                loc: loc(node, path),
            });
        }
    }
}

fn extract_super_interfaces_deps(
    node: &Node,
    source: &str,
    path: &Path,
    qualified_name: &str,
    ir: &mut Ir,
) {
    if let Some(ifaces) = find_child_by_kind(node, "super_interfaces") {
        // Look for type_list child, then iterate type_identifiers
        if let Some(type_list) = find_child_by_kind(&ifaces, "type_list") {
            let mut cursor = type_list.walk();
            for child in type_list.children(&mut cursor) {
                if child.kind() == "type_identifier" || child.kind() == "generic_type" {
                    ir.dependencies.push(Dependency {
                        from_qualified: qualified_name.to_string(),
                        to_name: node_text(&child, source).to_string(),
                        kind: DepKind::Implements,
                        loc: loc(node, path),
                    });
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Signature builder
// ---------------------------------------------------------------------------

fn build_method_signature(node: &Node, source: &str) -> String {
    let full = node_text(node, source);
    if let Some(pos) = full.find('{') {
        full[..pos].trim().to_string()
    } else {
        // Interface method — ends with ;
        full.trim_end_matches(';').trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_prefix_from_path() {
        assert_eq!(
            package_prefix_from_path(Path::new("src/main/java/com/example/Foo.java")),
            "com.example"
        );
        assert_eq!(
            package_prefix_from_path(Path::new("src/Foo.java")),
            ""
        );
        assert_eq!(
            package_prefix_from_path(Path::new("Foo.java")),
            ""
        );
    }

    #[test]
    fn test_parse_simple_class() {
        let source = "public class Foo { public int x; }";
        let ir = parse_file(Path::new("Foo.java"), source).unwrap();
        let classes: Vec<_> = ir.symbols.iter().filter(|s| s.kind == "class").collect();
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].name, "Foo");
        assert_eq!(classes[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_parse_interface() {
        let source = "public interface Processor { void process(); }";
        let ir = parse_file(Path::new("Processor.java"), source).unwrap();
        let traits: Vec<_> = ir.symbols.iter().filter(|s| s.kind == "interface").collect();
        assert_eq!(traits.len(), 1);
        assert_eq!(traits[0].name, "Processor");
    }

    #[test]
    fn test_parse_enum() {
        let source = "public enum Color { RED, GREEN, BLUE }";
        let ir = parse_file(Path::new("Color.java"), source).unwrap();
        let enums: Vec<_> = ir.symbols.iter().filter(|s| s.kind == "enum").collect();
        assert_eq!(enums.len(), 1);
        assert_eq!(enums[0].name, "Color");
    }

    #[test]
    fn test_parse_import() {
        let source = "import java.util.List;\npublic class Foo {}";
        let ir = parse_file(Path::new("Foo.java"), source).unwrap();
        let imports: Vec<_> = ir.dependencies.iter().filter(|d| d.kind == DepKind::Import).collect();
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].to_name, "java.util.List");
    }

    #[test]
    fn test_package_used_as_prefix() {
        let source = "package com.example;\npublic class Foo {}";
        let ir = parse_file(Path::new("Foo.java"), source).unwrap();
        let foo = ir.symbols.iter().find(|s| s.name == "Foo").unwrap();
        assert_eq!(foo.qualified_name, "com.example.Foo");
    }

    #[test]
    fn test_method_has_parent() {
        let source = r#"
public class Foo {
    public void bar(int x) {}
}
"#;
        let ir = parse_file(Path::new("Foo.java"), source).unwrap();
        let bar = ir.symbols.iter().find(|s| s.name == "bar").unwrap();
        assert_eq!(bar.kind, "method");
        assert_eq!(bar.parent.as_deref(), Some("Foo"));
        assert_eq!(bar.params.len(), 1);
        assert_eq!(bar.params[0].name, "x");
    }

    #[test]
    fn test_sealed_with_permits_records() {
        let source = r#"public sealed interface TripQuery permits TripQuery.Everything, TripQuery.ByStatus {
    record Everything() implements TripQuery {}
    record ByStatus(String status) implements TripQuery {}
    static TripQuery everything() { return new Everything(); }
}"#;
        let ir = parse_file(Path::new("TripQuery.java"), source).unwrap();
        let records: Vec<_> = ir.symbols.iter()
            .filter(|s| s.kind == "record")
            .collect();
        assert_eq!(records.len(), 2, "Should find both inner records");
        let names: Vec<&str> = records.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Everything"));
        assert!(names.contains(&"ByStatus"));
    }

    #[test]
    fn test_sealed_interface_with_annotated_params() {
        // Real-world pattern: sealed interface + extends + annotated record params + multi-line braces
        let source = "package com.example;\n\nimport org.springframework.lang.NonNull;\n\npublic sealed interface TripPersistenceAction extends TripProvider {\n\n    record CreateTripAction(@NonNull Trip trip) implements TripPersistenceAction {\n    }\n\n    record UpdateTripAction(@NonNull Trip trip) implements TripPersistenceAction {\n    }\n\n    record DeleteTripAction(@NonNull Trip trip) implements TripPersistenceAction {\n    }\n}\n";
        let ir = parse_file(Path::new("src/main/java/com/example/TripPersistenceAction.java"), source).unwrap();
        let records: Vec<_> = ir.symbols.iter()
            .filter(|s| s.kind == "record")
            .collect();
        assert_eq!(records.len(), 3, "Should find all 3 inner records");
        let names: Vec<&str> = records.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"CreateTripAction"));
        assert!(names.contains(&"UpdateTripAction"));
        assert!(names.contains(&"DeleteTripAction"));
    }

    #[test]
    fn test_record_has_fields_and_params() {
        let source = "public record Point(int x, int y) {}";
        let ir = parse_file(Path::new("Point.java"), source).unwrap();
        let point = ir.symbols.iter().find(|s| s.name == "Point").unwrap();
        assert_eq!(point.kind, "record");
        assert_eq!(point.fields.len(), 2);
        assert_eq!(point.params.len(), 2);
        assert_eq!(point.fields[0].name, "x");
        assert_eq!(point.params[0].name, "x");
    }
}
