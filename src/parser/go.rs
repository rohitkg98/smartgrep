use std::path::Path;

use anyhow::Result;
use tree_sitter::{Node, Parser};

use crate::ir::types::*;

/// Parse a Go source file and return the IR.
pub fn parse_file(path: &Path, source: &str) -> Result<Ir> {
    let mut parser = Parser::new();
    let language = tree_sitter_go::LANGUAGE;
    parser.set_language(&language.into())?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter failed to parse {}", path.display()))?;

    let mut ir = Ir::default();

    // First pass: extract the package name for qualified name prefix
    let prefix = extract_package(tree.root_node(), source).unwrap_or_default();

    extract_items(tree.root_node(), source, path, &prefix, &mut ir);

    Ok(ir)
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

fn loc(node: &Node, path: &Path) -> SourceLoc {
    let start = node.start_position();
    SourceLoc {
        file: path.to_path_buf(),
        line: start.row + 1,
        col: start.column + 1,
    }
}

fn node_text<'a>(node: &Node, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

fn find_child_by_kind<'a>(node: &Node<'a>, kind: &str) -> Option<Node<'a>> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == kind {
                return Some(child);
            }
        }
    }
    None
}

/// Go visibility: uppercase first letter = Public, lowercase = Private.
fn go_visibility(name: &str) -> Visibility {
    if name.starts_with(|c: char| c.is_uppercase()) {
        Visibility::Public
    } else {
        Visibility::Private
    }
}

fn qualified(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{}.{}", prefix, name)
    }
}

// ---------------------------------------------------------------------------
// Package extraction
// ---------------------------------------------------------------------------

fn extract_package(root: Node, source: &str) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "package_clause" {
            // package_clause has a child "package_identifier"
            if let Some(pkg) = find_child_by_kind(&child, "package_identifier") {
                return Some(node_text(&pkg, source).to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Top-level item extraction
// ---------------------------------------------------------------------------

fn extract_items(node: Node, source: &str, path: &Path, prefix: &str, ir: &mut Ir) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "package_clause" => {
                // Already handled
            }
            "import_declaration" => {
                extract_imports(&child, source, path, prefix, ir);
            }
            "function_declaration" => {
                if let Some(sym) = extract_function(&child, source, path, prefix) {
                    ir.symbols.push(sym);
                }
            }
            "method_declaration" => {
                if let Some(sym) = extract_method(&child, source, path, prefix) {
                    ir.symbols.push(sym);
                }
            }
            "type_declaration" => {
                extract_type_declaration(&child, source, path, prefix, ir);
            }
            "const_declaration" => {
                extract_const_declaration(&child, source, path, prefix, ir);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Imports
// ---------------------------------------------------------------------------

fn extract_imports(node: &Node, source: &str, path: &Path, prefix: &str, ir: &mut Ir) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "import_spec" => {
                if let Some(dep) = extract_import_spec(&child, source, path, prefix) {
                    ir.dependencies.push(dep);
                }
            }
            "import_spec_list" => {
                let mut inner = child.walk();
                for spec in child.children(&mut inner) {
                    if spec.kind() == "import_spec" {
                        if let Some(dep) = extract_import_spec(&spec, source, path, prefix) {
                            ir.dependencies.push(dep);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn extract_import_spec(node: &Node, source: &str, path: &Path, prefix: &str) -> Option<Dependency> {
    // import_spec has a "path" child (interpreted_string_literal)
    if let Some(path_node) = find_child_by_kind(node, "interpreted_string_literal") {
        let import_path = node_text(&path_node, source)
            .trim_matches('"')
            .to_string();
        return Some(Dependency {
            from_qualified: prefix.to_string(),
            to_name: import_path,
            kind: DepKind::Import,
            loc: loc(node, path),
        });
    }
    None
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

fn extract_function(node: &Node, source: &str, path: &Path, prefix: &str) -> Option<Symbol> {
    let name = node
        .child_by_field_name("name")
        .map(|n| node_text(&n, source).to_string())?;

    let params = extract_params(node, source);
    let return_type = extract_return_type(node, source);
    let sig = build_signature(node, source);

    Some(Symbol {
        name: name.clone(),
        qualified_name: qualified(prefix, &name),
        kind: "func".to_string(),
        loc: loc(node, path),
        visibility: go_visibility(&name),
        signature: Some(sig),
        parent: None,
        attributes: vec![],
        fields: vec![],
        params,
        return_type,
    })
}

// ---------------------------------------------------------------------------
// Methods (with receiver)
// ---------------------------------------------------------------------------

fn extract_method(node: &Node, source: &str, path: &Path, prefix: &str) -> Option<Symbol> {
    let name = node
        .child_by_field_name("name")
        .map(|n| node_text(&n, source).to_string())?;

    let receiver_type = extract_receiver_type(node, source);
    let params = extract_params(node, source);
    let return_type = extract_return_type(node, source);
    let sig = build_signature(node, source);

    let parent_name = receiver_type.as_deref().unwrap_or("");
    let qualified_name = if parent_name.is_empty() {
        qualified(prefix, &name)
    } else {
        let base = qualified(prefix, parent_name);
        format!("{}.{}", base, name)
    };

    Some(Symbol {
        name,
        qualified_name,
        kind: "method".to_string(),
        loc: loc(node, path),
        visibility: go_visibility(&node.child_by_field_name("name").map(|n| node_text(&n, source)).unwrap_or("")),
        signature: Some(sig),
        parent: receiver_type,
        attributes: vec![],
        fields: vec![],
        params,
        return_type,
    })
}

/// Extract the receiver type name from a method_declaration.
/// e.g., `func (c *Config) GetName()` → "Config"
fn extract_receiver_type(node: &Node, source: &str) -> Option<String> {
    let receiver = node.child_by_field_name("receiver")?;
    // parameter_list → parameter_declaration → type
    let mut cursor = receiver.walk();
    for child in receiver.children(&mut cursor) {
        if child.kind() == "parameter_declaration" {
            // The type could be a pointer_type (*Config) or a type_identifier (Config)
            if let Some(ptr) = find_child_by_kind(&child, "pointer_type") {
                if let Some(type_id) = find_child_by_kind(&ptr, "type_identifier") {
                    return Some(node_text(&type_id, source).to_string());
                }
            }
            if let Some(type_id) = find_child_by_kind(&child, "type_identifier") {
                return Some(node_text(&type_id, source).to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Type declarations (struct, interface, type alias)
// ---------------------------------------------------------------------------

fn extract_type_declaration(node: &Node, source: &str, path: &Path, prefix: &str, ir: &mut Ir) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_spec" => {
                extract_type_spec(&child, source, path, prefix, ir);
            }
            "type_alias" => {
                extract_type_alias(&child, source, path, prefix, ir);
            }
            _ => {}
        }
    }
}

fn extract_type_spec(node: &Node, source: &str, path: &Path, prefix: &str, ir: &mut Ir) {
    let name = node
        .child_by_field_name("name")
        .map(|n| node_text(&n, source).to_string());
    let name = match name {
        Some(n) => n,
        None => return,
    };

    let type_node = node.child_by_field_name("type");

    match type_node.as_ref().map(|n| n.kind()) {
        Some("struct_type") => {
            let fields = extract_struct_fields(type_node.as_ref().unwrap(), source);
            ir.symbols.push(Symbol {
                name: name.clone(),
                qualified_name: qualified(prefix, &name),
                kind: "struct".to_string(),
                loc: loc(node, path),
                visibility: go_visibility(&name),
                signature: None,
                parent: None,
                attributes: vec![],
                fields,
                params: vec![],
                return_type: None,
            });
        }
        Some("interface_type") => {
            extract_interface_methods(type_node.as_ref().unwrap(), source, path, prefix, &name, ir);
            ir.symbols.push(Symbol {
                name: name.clone(),
                qualified_name: qualified(prefix, &name),
                kind: "interface".to_string(),
                loc: loc(node, path),
                visibility: go_visibility(&name),
                signature: None,
                parent: None,
                attributes: vec![],
                fields: vec![],
                params: vec![],
                return_type: None,
            });
        }
        _ => {
            // Type definition: `type X Y` (not an alias with =)
            let type_text = type_node
                .map(|n| node_text(&n, source).to_string())
                .unwrap_or_default();
            ir.symbols.push(Symbol {
                name: name.clone(),
                qualified_name: qualified(prefix, &name),
                kind: "type".to_string(),
                loc: loc(node, path),
                visibility: go_visibility(&name),
                signature: Some(format!("type {} {}", name, type_text)),
                parent: None,
                attributes: vec![],
                fields: vec![],
                params: vec![],
                return_type: Some(type_text),
            });
        }
    }
}

/// Extract a type alias: `type X = Y`
fn extract_type_alias(node: &Node, source: &str, path: &Path, prefix: &str, ir: &mut Ir) {
    let name = node
        .child_by_field_name("name")
        .map(|n| node_text(&n, source).to_string());
    let name = match name {
        Some(n) => n,
        None => return,
    };

    let type_text = node
        .child_by_field_name("type")
        .map(|n| node_text(&n, source).to_string())
        .unwrap_or_default();

    ir.symbols.push(Symbol {
        name: name.clone(),
        qualified_name: qualified(prefix, &name),
        kind: "type".to_string(),
        loc: loc(node, path),
        visibility: go_visibility(&name),
        signature: Some(format!("type {} = {}", name, type_text)),
        parent: None,
        attributes: vec![],
        fields: vec![],
        params: vec![],
        return_type: Some(type_text),
    });
}

// ---------------------------------------------------------------------------
// Struct fields
// ---------------------------------------------------------------------------

fn extract_struct_fields(struct_node: &Node, source: &str) -> Vec<Field> {
    let mut fields = Vec::new();
    if let Some(field_list) = find_child_by_kind(struct_node, "field_declaration_list") {
        let mut cursor = field_list.walk();
        for child in field_list.children(&mut cursor) {
            if child.kind() == "field_declaration" {
                extract_field_declaration(&child, source, &mut fields);
            }
        }
    }
    fields
}

fn extract_field_declaration(node: &Node, source: &str, fields: &mut Vec<Field>) {
    // A field_declaration can have:
    // 1. Named fields: `Name string` or `Name, Age int`
    // 2. Embedded types: `io.Reader` (no explicit name)

    // Collect all identifier children (field names)
    let mut names = Vec::new();
    let mut type_text = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "field_identifier" => {
                names.push(node_text(&child, source).to_string());
            }
            "type_identifier" | "pointer_type" | "array_type" | "slice_type"
            | "map_type" | "channel_type" | "function_type" | "interface_type"
            | "struct_type" | "qualified_type" | "generic_type" => {
                type_text = node_text(&child, source).to_string();
            }
            _ => {}
        }
    }

    if names.is_empty() {
        // Embedded field — use the type name as the field name
        let embed_name = type_text
            .trim_start_matches('*')
            .rsplit('.')
            .next()
            .unwrap_or(&type_text)
            .to_string();
        if !embed_name.is_empty() {
            fields.push(Field {
                name: embed_name.clone(),
                type_name: type_text,
                visibility: go_visibility(&embed_name),
            });
        }
    } else {
        for name in &names {
            fields.push(Field {
                name: name.clone(),
                type_name: type_text.clone(),
                visibility: go_visibility(name),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Interface methods
// ---------------------------------------------------------------------------

fn extract_interface_methods(
    iface_node: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    parent_name: &str,
    ir: &mut Ir,
) {
    let mut cursor = iface_node.walk();
    for child in iface_node.children(&mut cursor) {
        if child.kind() == "method_elem" {
            if let Some(sym) = extract_method_elem(&child, source, path, prefix, parent_name) {
                ir.symbols.push(sym);
            }
        }
    }
}

/// Extract a method from a `method_elem` inside an interface.
/// Structure: method_elem → field_identifier, parameter_list, [return type]
fn extract_method_elem(
    node: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    parent_name: &str,
) -> Option<Symbol> {
    let name = find_child_by_kind(node, "field_identifier")
        .map(|n| node_text(&n, source).to_string())?;

    let mut params = Vec::new();
    if let Some(param_list) = find_child_by_kind(node, "parameter_list") {
        let mut cursor = param_list.walk();
        for child in param_list.children(&mut cursor) {
            if child.kind() == "parameter_declaration" {
                extract_param_decl(&child, source, &mut params);
            }
        }
    }

    // Return type: look for type nodes after the parameter_list
    let return_type = extract_method_elem_return_type(node, source);

    let sig = node_text(node, source).trim().to_string();
    let base = qualified(prefix, parent_name);
    let qualified_name = format!("{}.{}", base, name);

    Some(Symbol {
        name,
        qualified_name,
        kind: "method".to_string(),
        loc: loc(node, path),
        visibility: Visibility::Public,
        signature: Some(sig),
        parent: Some(parent_name.to_string()),
        attributes: vec![],
        fields: vec![],
        params,
        return_type,
    })
}

fn extract_method_elem_return_type(node: &Node, source: &str) -> Option<String> {
    // After parameter_list, any type node is the return type
    let mut found_params = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "parameter_list" {
            if found_params {
                // Second parameter_list = return values
                return Some(node_text(&child, source).to_string());
            }
            found_params = true;
        } else if found_params && is_type_node(child.kind()) {
            return Some(node_text(&child, source).to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Const declarations
// ---------------------------------------------------------------------------

fn extract_const_declaration(node: &Node, source: &str, path: &Path, prefix: &str, ir: &mut Ir) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "const_spec" {
            extract_const_spec(&child, source, path, prefix, ir);
        }
    }
}

fn extract_const_spec(node: &Node, source: &str, path: &Path, prefix: &str, ir: &mut Ir) {
    // const_spec has name children (identifiers) and optional type + value
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            let name = node_text(&child, source).to_string();
            let sig = node_text(node, source).trim().to_string();

            ir.symbols.push(Symbol {
                name: name.clone(),
                qualified_name: qualified(prefix, &name),
                kind: "const".to_string(),
                loc: loc(&child, path),
                visibility: go_visibility(&name),
                signature: Some(sig),
                parent: None,
                attributes: vec![],
                fields: vec![],
                params: vec![],
                return_type: None,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Parameters and return types
// ---------------------------------------------------------------------------

fn extract_params(node: &Node, source: &str) -> Vec<Param> {
    extract_param_list(node, source, "parameters")
}

fn extract_param_list(node: &Node, source: &str, field_name: &str) -> Vec<Param> {
    let mut params = Vec::new();
    let param_list = node.child_by_field_name(field_name);
    let param_list = match param_list {
        Some(p) => p,
        None => return params,
    };

    let mut cursor = param_list.walk();
    for child in param_list.children(&mut cursor) {
        if child.kind() == "parameter_declaration" {
            extract_param_decl(&child, source, &mut params);
        }
    }
    params
}

fn extract_param_decl(node: &Node, source: &str, params: &mut Vec<Param>) {
    let mut names = Vec::new();
    let mut type_text = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                names.push(node_text(&child, source).to_string());
            }
            _ if is_type_node(child.kind()) => {
                type_text = node_text(&child, source).to_string();
            }
            _ => {}
        }
    }

    if names.is_empty() {
        // Unnamed parameter — just a type
        if !type_text.is_empty() {
            params.push(Param {
                name: String::new(),
                type_name: type_text,
            });
        }
    } else {
        for name in &names {
            params.push(Param {
                name: name.clone(),
                type_name: type_text.clone(),
            });
        }
    }
}

fn is_type_node(kind: &str) -> bool {
    matches!(
        kind,
        "type_identifier"
            | "pointer_type"
            | "array_type"
            | "slice_type"
            | "map_type"
            | "channel_type"
            | "function_type"
            | "interface_type"
            | "struct_type"
            | "qualified_type"
            | "generic_type"
            | "negated_type"
    )
}

fn extract_return_type(node: &Node, source: &str) -> Option<String> {
    extract_result_type(node, source)
}

fn extract_result_type(node: &Node, source: &str) -> Option<String> {
    let result = node.child_by_field_name("result")?;
    let text = node_text(&result, source).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

// ---------------------------------------------------------------------------
// Signature builder
// ---------------------------------------------------------------------------

fn build_signature(node: &Node, source: &str) -> String {
    let full = node_text(node, source);
    // Take everything up to the opening brace
    if let Some(pos) = full.find('{') {
        full[..pos].trim().to_string()
    } else {
        full.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_struct() {
        let source = "package main\n\ntype Foo struct { X int }";
        let ir = parse_file(Path::new("foo.go"), source).unwrap();
        let structs: Vec<_> = ir.symbols.iter().filter(|s| s.kind == "struct").collect();
        assert_eq!(structs.len(), 1);
        assert_eq!(structs[0].name, "Foo");
        assert_eq!(structs[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_parse_simple_function() {
        let source = "package main\n\nfunc Hello(name string) string { return name }";
        let ir = parse_file(Path::new("foo.go"), source).unwrap();
        let funcs: Vec<_> = ir.symbols.iter().filter(|s| s.kind == "func").collect();
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "Hello");
        assert_eq!(funcs[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_parse_interface() {
        let source = "package main\n\ntype Reader interface { Read(p []byte) (int, error) }";
        let ir = parse_file(Path::new("foo.go"), source).unwrap();
        let traits: Vec<_> = ir.symbols.iter().filter(|s| s.kind == "interface").collect();
        assert_eq!(traits.len(), 1);
        assert_eq!(traits[0].name, "Reader");
    }

    #[test]
    fn test_private_visibility() {
        let source = "package main\n\nfunc helper() {}";
        let ir = parse_file(Path::new("foo.go"), source).unwrap();
        let f = ir.symbols.iter().find(|s| s.name == "helper").unwrap();
        assert_eq!(f.visibility, Visibility::Private);
    }

    #[test]
    fn test_method_has_receiver() {
        let source = "package main\n\ntype Foo struct{}\nfunc (f *Foo) Bar() {}";
        let ir = parse_file(Path::new("foo.go"), source).unwrap();
        let bar = ir.symbols.iter().find(|s| s.name == "Bar").unwrap();
        assert_eq!(bar.kind, "method");
        assert_eq!(bar.parent.as_deref(), Some("Foo"));
    }

    #[test]
    fn test_import_extraction() {
        let source = "package main\n\nimport \"fmt\"\n";
        let ir = parse_file(Path::new("foo.go"), source).unwrap();
        let imports: Vec<_> = ir.dependencies.iter().filter(|d| d.kind == DepKind::Import).collect();
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].to_name, "fmt");
    }

    #[test]
    fn test_const_extraction() {
        let source = "package main\n\nconst MaxSize = 1024";
        let ir = parse_file(Path::new("foo.go"), source).unwrap();
        let consts: Vec<_> = ir.symbols.iter().filter(|s| s.kind == "const").collect();
        assert_eq!(consts.len(), 1);
        assert_eq!(consts[0].name, "MaxSize");
        assert_eq!(consts[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_type_alias() {
        let source = "package main\n\ntype MyInt = int";
        let ir = parse_file(Path::new("foo.go"), source).unwrap();
        let aliases: Vec<_> = ir.symbols.iter().filter(|s| s.kind == "type").collect();
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].name, "MyInt");
    }

    #[test]
    fn test_qualified_name_uses_package() {
        let source = "package myapp\n\nfunc DoStuff() {}";
        let ir = parse_file(Path::new("foo.go"), source).unwrap();
        let f = ir.symbols.iter().find(|s| s.name == "DoStuff").unwrap();
        assert_eq!(f.qualified_name, "myapp.DoStuff");
    }
}
