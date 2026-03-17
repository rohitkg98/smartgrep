use std::path::Path;

use anyhow::Result;
use tree_sitter::{Node, Parser};

use crate::ir::types::*;
use crate::parser::common::{find_child_by_kind, loc, node_text};

/// Parse a TypeScript or TSX source file and return the IR.
pub fn parse_file(path: &Path, source: &str) -> Result<Ir> {
    let mut parser = Parser::new();

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("ts");
    let language = if ext == "tsx" {
        tree_sitter_typescript::LANGUAGE_TSX
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT
    };
    parser.set_language(&language.into())?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter failed to parse {}", path.display()))?;

    let mut ir = Ir::default();
    let prefix = module_prefix_from_path(path);

    extract_items(tree.root_node(), source, path, &prefix, false, None, &mut ir);

    Ok(ir)
}

/// Derive a module prefix from the file path.
/// `src/services/user.ts` → `services.user`
/// `src/index.ts` → ``
fn module_prefix_from_path(path: &Path) -> String {
    let path_str = path.to_string_lossy();

    // Strip common source roots
    let stripped = path_str
        .strip_prefix("src/")
        .unwrap_or(&path_str);

    // Remove extension
    let without_ext = stripped
        .strip_suffix(".tsx")
        .or_else(|| stripped.strip_suffix(".ts"))
        .unwrap_or(stripped);

    // Take directory part (drop the filename)
    if let Some(pos) = without_ext.rfind('/') {
        without_ext[..pos].replace('/', ".")
    } else {
        String::new()
    }
}

// ---------------------------------------------------------------------------
// Main dispatcher
// ---------------------------------------------------------------------------

fn extract_items(
    node: Node,
    source: &str,
    path: &Path,
    prefix: &str,
    exported: bool,
    parent: Option<&str>,
    ir: &mut Ir,
) {
    // Collect decorators that precede declarations (they appear as sibling nodes
    // at the program level in tree-sitter-typescript)
    let mut pending_decorators: Vec<String> = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "decorator" => {
                pending_decorators.push(node_text(&child, source).to_string());
            }
            "import_statement" => {
                pending_decorators.clear();
                if let Some(dep) = extract_import(&child, source, path, prefix) {
                    ir.dependencies.push(dep);
                }
            }
            "export_statement" => {
                // Collect any decorators that are children of the export_statement
                let mut export_decorators = std::mem::take(&mut pending_decorators);
                let mut inner_cursor = child.walk();
                for export_child in child.children(&mut inner_cursor) {
                    if export_child.kind() == "decorator" {
                        export_decorators.push(node_text(&export_child, source).to_string());
                    }
                }
                // Unwrap: export_statement wraps the inner declaration
                extract_items_with_decorators(child, source, path, prefix, true, parent, &export_decorators, ir);
            }
            "function_declaration" => {
                let decorators = std::mem::take(&mut pending_decorators);
                if let Some(mut sym) = extract_function(&child, source, path, prefix, exported, parent) {
                    sym.attributes.extend(decorators);
                    ir.symbols.push(sym);
                }
            }
            "lexical_declaration" => {
                pending_decorators.clear();
                extract_lexical_declaration(&child, source, path, prefix, exported, parent, ir);
            }
            "class_declaration" | "abstract_class_declaration" => {
                let decorators = std::mem::take(&mut pending_decorators);
                extract_class(&child, source, path, prefix, exported, &decorators, ir);
            }
            "interface_declaration" => {
                pending_decorators.clear();
                extract_interface(&child, source, path, prefix, exported, ir);
            }
            "enum_declaration" => {
                pending_decorators.clear();
                if let Some(sym) = extract_enum(&child, source, path, prefix, exported) {
                    ir.symbols.push(sym);
                }
            }
            "type_alias_declaration" => {
                pending_decorators.clear();
                if let Some(sym) = extract_type_alias(&child, source, path, prefix, exported) {
                    ir.symbols.push(sym);
                }
            }
            "internal_module" => {
                pending_decorators.clear();
                extract_namespace(&child, source, path, prefix, exported, ir);
            }
            _ => {}
        }
    }
}

/// Same as extract_items but with pre-collected decorators to attach to the first declaration found.
fn extract_items_with_decorators(
    node: Node,
    source: &str,
    path: &Path,
    prefix: &str,
    exported: bool,
    parent: Option<&str>,
    decorators: &[String],
    ir: &mut Ir,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_declaration" => {
                if let Some(mut sym) = extract_function(&child, source, path, prefix, exported, parent) {
                    sym.attributes.extend(decorators.iter().cloned());
                    ir.symbols.push(sym);
                }
            }
            "lexical_declaration" => {
                extract_lexical_declaration(&child, source, path, prefix, exported, parent, ir);
            }
            "class_declaration" | "abstract_class_declaration" => {
                extract_class(&child, source, path, prefix, exported, decorators, ir);
            }
            "interface_declaration" => {
                extract_interface(&child, source, path, prefix, exported, ir);
            }
            "enum_declaration" => {
                if let Some(sym) = extract_enum(&child, source, path, prefix, exported) {
                    ir.symbols.push(sym);
                }
            }
            "type_alias_declaration" => {
                if let Some(sym) = extract_type_alias(&child, source, path, prefix, exported) {
                    ir.symbols.push(sym);
                }
            }
            "internal_module" => {
                extract_namespace(&child, source, path, prefix, exported, ir);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

fn extract_import(node: &Node, source: &str, path: &Path, prefix: &str) -> Option<Dependency> {
    // import_statement: import { X } from 'module'
    // The source is a string_fragment inside a string child
    let source_node = find_child_by_kind(node, "string")?;
    let raw = node_text(&source_node, source);
    let module_name = raw.trim_matches('\'').trim_matches('"');

    let from_qn = if prefix.is_empty() {
        "(file)".to_string()
    } else {
        prefix.to_string()
    };

    Some(Dependency {
        from_qualified: from_qn,
        to_name: module_name.to_string(),
        kind: DepKind::Import,
        loc: loc(node, path),
    })
}

// ---------------------------------------------------------------------------
// Function declaration
// ---------------------------------------------------------------------------

fn extract_function(
    node: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    exported: bool,
    parent: Option<&str>,
) -> Option<Symbol> {
    let name = get_name(node, source)?;

    let qn = if let Some(p) = parent {
        if prefix.is_empty() {
            format!("{}.{}", p, name)
        } else {
            format!("{}.{}.{}", prefix, p, name)
        }
    } else if prefix.is_empty() {
        name.clone()
    } else {
        format!("{}.{}", prefix, name)
    };

    let vis = if exported { Visibility::Public } else { Visibility::Private };
    let params = extract_formal_params(node, source);
    let return_type = extract_return_type(node, source);
    let signature = build_function_signature(node, source);

    Some(Symbol {
        name,
        qualified_name: qn,
        kind: "function".to_string(),
        loc: loc(node, path),
        visibility: vis,
        parent: parent.map(|s| s.to_string()),
        fields: vec![],
        params,
        return_type,
        signature: Some(signature),
        attributes: vec![],
    })
}

// ---------------------------------------------------------------------------
// Lexical declaration (const/let with arrow functions or regular consts)
// ---------------------------------------------------------------------------

fn extract_lexical_declaration(
    node: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    exported: bool,
    parent: Option<&str>,
    ir: &mut Ir,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            let name = child.child_by_field_name("name")
                .map(|n| node_text(&n, source).to_string());
            let name = match name {
                Some(n) => n,
                None => continue,
            };

            let value = child.child_by_field_name("value");

            // Check if the value is an arrow function
            let is_arrow = value.as_ref().map(|v| v.kind() == "arrow_function").unwrap_or(false);

            if is_arrow {
                let val_node = value.unwrap();
                let qn = if let Some(p) = parent {
                    if prefix.is_empty() {
                        format!("{}.{}", p, name)
                    } else {
                        format!("{}.{}.{}", prefix, p, name)
                    }
                } else if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}.{}", prefix, name)
                };

                let vis = if exported { Visibility::Public } else { Visibility::Private };
                let params = extract_formal_params(&val_node, source);
                let return_type = extract_return_type(&val_node, source);
                let signature = format!("const {} = {}", name, build_arrow_signature(&val_node, source));

                ir.symbols.push(Symbol {
                    name,
                    qualified_name: qn,
                    kind: "function".to_string(),
                    loc: loc(&child, path),
                    visibility: vis,
                    parent: parent.map(|s| s.to_string()),
                    fields: vec![],
                    params,
                    return_type,
                    signature: Some(signature),
                    attributes: vec![],
                });
            } else {
                // Regular const
                let qn = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}.{}", prefix, name)
                };

                let vis = if exported { Visibility::Public } else { Visibility::Private };

                ir.symbols.push(Symbol {
                    name,
                    qualified_name: qn,
                    kind: "const".to_string(),
                    loc: loc(&child, path),
                    visibility: vis,
                    parent: parent.map(|s| s.to_string()),
                    fields: vec![],
                    params: vec![],
                    return_type: None,
                    signature: None,
                    attributes: vec![],
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Class
// ---------------------------------------------------------------------------

fn extract_class(
    node: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    exported: bool,
    outer_decorators: &[String],
    ir: &mut Ir,
) {
    let name = match get_name(node, source) {
        Some(n) => n,
        None => return,
    };

    let is_abstract = node.kind() == "abstract_class_declaration";
    let qn = if prefix.is_empty() {
        name.clone()
    } else {
        format!("{}.{}", prefix, name)
    };

    let vis = if exported { Visibility::Public } else { Visibility::Private };
    let mut attributes: Vec<String> = outer_decorators.to_vec();
    attributes.extend(extract_decorators(node, source));
    if is_abstract {
        attributes.push("abstract".to_string());
    }

    // Extract extends/implements deps
    extract_heritage_deps(node, source, path, &qn, ir);

    // Extract body
    let mut fields = Vec::new();
    let body = find_child_by_kind(node, "class_body");
    if let Some(ref body_node) = body {
        extract_class_body(body_node, source, path, &name, &qn, &mut fields, ir);
    }

    ir.symbols.push(Symbol {
        name,
        qualified_name: qn,
        kind: "class".to_string(),
        loc: loc(node, path),
        visibility: vis,
        parent: None,
        fields,
        params: vec![],
        return_type: None,
        signature: None,
        attributes,
    });
}

fn extract_class_body(
    body: &Node,
    source: &str,
    path: &Path,
    parent_name: &str,
    parent_qn: &str,
    fields: &mut Vec<Field>,
    ir: &mut Ir,
) {
    let mut pending_decorators: Vec<String> = Vec::new();
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        match child.kind() {
            "decorator" => {
                pending_decorators.push(node_text(&child, source).to_string());
            }
            "method_definition" | "abstract_method_signature" => {
                let decorators = std::mem::take(&mut pending_decorators);
                if let Some(mut sym) = extract_method(&child, source, path, parent_name, parent_qn) {
                    if child.kind() == "abstract_method_signature" {
                        sym.attributes.push("abstract".to_string());
                    }
                    sym.attributes.extend(decorators);
                    ir.symbols.push(sym);
                }
            }
            "public_field_definition" => {
                pending_decorators.clear();
                if let Some(field) = extract_class_field(&child, source) {
                    fields.push(field);
                }
            }
            _ => {}
        }
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
    exported: bool,
    ir: &mut Ir,
) {
    let name = match get_name(node, source) {
        Some(n) => n,
        None => return,
    };

    let qn = if prefix.is_empty() {
        name.clone()
    } else {
        format!("{}.{}", prefix, name)
    };

    let vis = if exported { Visibility::Public } else { Visibility::Private };

    // Extract extends deps
    extract_heritage_deps(node, source, path, &qn, ir);

    // Extract body
    let mut fields = Vec::new();
    let body = find_child_by_kind(node, "interface_body")
        .or_else(|| find_child_by_kind(node, "object_type"));
    if let Some(ref body_node) = body {
        extract_interface_body(body_node, source, path, &name, &qn, &mut fields, ir);
    }

    ir.symbols.push(Symbol {
        name,
        qualified_name: qn,
        kind: "interface".to_string(),
        loc: loc(node, path),
        visibility: vis,
        parent: None,
        fields,
        params: vec![],
        return_type: None,
        signature: None,
        attributes: vec![],
    });
}

fn extract_interface_body(
    body: &Node,
    source: &str,
    path: &Path,
    parent_name: &str,
    parent_qn: &str,
    fields: &mut Vec<Field>,
    ir: &mut Ir,
) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        match child.kind() {
            "method_signature" => {
                if let Some(sym) = extract_method_signature(&child, source, path, parent_name, parent_qn) {
                    ir.symbols.push(sym);
                }
            }
            "property_signature" => {
                if let Some(field) = extract_property_signature(&child, source) {
                    fields.push(field);
                }
            }
            _ => {}
        }
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
    exported: bool,
) -> Option<Symbol> {
    let name = get_name(node, source)?;
    let qn = if prefix.is_empty() {
        name.clone()
    } else {
        format!("{}.{}", prefix, name)
    };

    let vis = if exported { Visibility::Public } else { Visibility::Private };

    // Check for const enum
    let text = node_text(node, source);
    let mut attributes = Vec::new();
    if text.starts_with("const ") {
        attributes.push("const".to_string());
    }

    // Extract enum members as fields
    let mut fields = Vec::new();
    if let Some(body) = find_child_by_kind(node, "enum_body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "property_identifier" || child.kind() == "enum_assignment" {
                let member_name = if child.kind() == "enum_assignment" {
                    child.child(0).map(|n| node_text(&n, source).to_string())
                } else {
                    Some(node_text(&child, source).to_string())
                };
                if let Some(n) = member_name {
                    fields.push(Field {
                        name: n,
                        type_name: String::new(),
                        visibility: Visibility::Public,
                    });
                }
            }
        }
    }

    Some(Symbol {
        name,
        qualified_name: qn,
        kind: "enum".to_string(),
        loc: loc(node, path),
        visibility: vis,
        parent: None,
        fields,
        params: vec![],
        return_type: None,
        signature: None,
        attributes,
    })
}

// ---------------------------------------------------------------------------
// Type alias
// ---------------------------------------------------------------------------

fn extract_type_alias(
    node: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    exported: bool,
) -> Option<Symbol> {
    let name = get_name(node, source)?;
    let qn = if prefix.is_empty() {
        name.clone()
    } else {
        format!("{}.{}", prefix, name)
    };

    let vis = if exported { Visibility::Public } else { Visibility::Private };

    // Capture the full signature
    let signature = node_text(node, source).lines().next().unwrap_or("").to_string();

    Some(Symbol {
        name,
        qualified_name: qn,
        kind: "type".to_string(),
        loc: loc(node, path),
        visibility: vis,
        parent: None,
        fields: vec![],
        params: vec![],
        return_type: None,
        signature: Some(signature),
        attributes: vec![],
    })
}

// ---------------------------------------------------------------------------
// Namespace (internal_module)
// ---------------------------------------------------------------------------

fn extract_namespace(
    node: &Node,
    source: &str,
    path: &Path,
    prefix: &str,
    exported: bool,
    ir: &mut Ir,
) {
    let name = get_name(node, source)
        .or_else(|| {
            // namespace name may be an identifier child, not a "name" field
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "identifier" {
                        return Some(node_text(&child, source).to_string());
                    }
                }
            }
            None
        });
    let name = match name {
        Some(n) => n,
        None => return,
    };

    let qn = if prefix.is_empty() {
        name.clone()
    } else {
        format!("{}.{}", prefix, name)
    };

    let vis = if exported { Visibility::Public } else { Visibility::Private };

    ir.symbols.push(Symbol {
        name: name.clone(),
        qualified_name: qn.clone(),
        kind: "namespace".to_string(),
        loc: loc(node, path),
        visibility: vis,
        parent: None,
        fields: vec![],
        params: vec![],
        return_type: None,
        signature: None,
        attributes: vec![],
    });

    // Extract inner declarations — find the statement_block body
    // Pass the namespace name as parent so inner symbols show it in display_name,
    // but use qn as prefix so qualified names are correct (no duplication).
    if let Some(body) = find_child_by_kind(node, "statement_block") {
        extract_items(body, source, path, &qn, exported, None, ir);
        // Set parent on inner symbols (top-level in the namespace body only)
        // to enable display like Validation::isValid
        for sym in ir.symbols.iter_mut() {
            if sym.parent.is_none() && sym.qualified_name.starts_with(&format!("{}.", qn)) && sym.qualified_name != qn {
                // Only set parent for direct children (one level deep)
                let rest = &sym.qualified_name[qn.len() + 1..];
                if !rest.contains('.') {
                    sym.parent = Some(name.clone());
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Method (class method_definition)
// ---------------------------------------------------------------------------

fn extract_method(
    node: &Node,
    source: &str,
    path: &Path,
    parent_name: &str,
    parent_qn: &str,
) -> Option<Symbol> {
    let name = get_name(node, source)
        .or_else(|| find_child_by_kind(node, "property_identifier")
            .map(|n| node_text(&n, source).to_string()))?;

    let qn = format!("{}.{}", parent_qn, name);
    let vis = get_member_visibility(node, source);
    let params = extract_formal_params(node, source);
    let return_type = extract_return_type(node, source);
    let signature = build_method_signature(node, source);
    let mut attributes = extract_decorators(node, source);

    // Check for static/abstract/readonly modifiers
    if has_modifier(node, source, "static") {
        attributes.push("static".to_string());
    }
    if has_modifier(node, source, "abstract") {
        attributes.push("abstract".to_string());
    }

    Some(Symbol {
        name,
        qualified_name: qn,
        kind: "method".to_string(),
        loc: loc(node, path),
        visibility: vis,
        parent: Some(parent_name.to_string()),
        fields: vec![],
        params,
        return_type,
        signature: Some(signature),
        attributes,
    })
}

// ---------------------------------------------------------------------------
// Method signature (interface)
// ---------------------------------------------------------------------------

fn extract_method_signature(
    node: &Node,
    source: &str,
    path: &Path,
    parent_name: &str,
    parent_qn: &str,
) -> Option<Symbol> {
    let name = get_name(node, source)
        .or_else(|| {
            // method_signature name might be a property_identifier child
            find_child_by_kind(node, "property_identifier")
                .map(|n| node_text(&n, source).to_string())
        })?;

    let qn = format!("{}.{}", parent_qn, name);
    let params = extract_formal_params(node, source);
    let return_type = extract_return_type(node, source);
    let signature = node_text(node, source).trim().trim_end_matches(';').to_string();

    Some(Symbol {
        name,
        qualified_name: qn,
        kind: "method".to_string(),
        loc: loc(node, path),
        visibility: Visibility::Public,
        parent: Some(parent_name.to_string()),
        fields: vec![],
        params,
        return_type,
        signature: Some(signature),
        attributes: vec![],
    })
}

// ---------------------------------------------------------------------------
// Class field
// ---------------------------------------------------------------------------

fn extract_class_field(node: &Node, source: &str) -> Option<Field> {
    let name = node.child_by_field_name("name")
        .or_else(|| find_child_by_kind(node, "property_identifier"))
        .map(|n| node_text(&n, source).to_string())?;

    let type_name = extract_type_annotation(node, source).unwrap_or_default();
    let vis = get_member_visibility(node, source);

    Some(Field {
        name,
        type_name,
        visibility: vis,
    })
}

// ---------------------------------------------------------------------------
// Property signature (interface)
// ---------------------------------------------------------------------------

fn extract_property_signature(node: &Node, source: &str) -> Option<Field> {
    let name = node.child_by_field_name("name")
        .or_else(|| find_child_by_kind(node, "property_identifier"))
        .map(|n| node_text(&n, source).to_string())?;

    let type_name = extract_type_annotation(node, source).unwrap_or_default();

    Some(Field {
        name,
        type_name,
        visibility: Visibility::Public,
    })
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

fn get_name(node: &Node, source: &str) -> Option<String> {
    node.child_by_field_name("name")
        .map(|n| node_text(&n, source).to_string())
}

fn get_member_visibility(node: &Node, source: &str) -> Visibility {
    // Check for accessibility_modifier child
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "accessibility_modifier" {
                let modifier = node_text(&child, source);
                return match modifier {
                    "private" => Visibility::Private,
                    "protected" => Visibility::Crate,
                    "public" => Visibility::Public,
                    _ => Visibility::Public,
                };
            }
        }
    }
    // Default for class members is public in TypeScript
    Visibility::Public
}

fn has_modifier(node: &Node, source: &str, modifier: &str) -> bool {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == modifier || node_text(&child, source) == modifier {
                return true;
            }
        }
    }
    false
}

fn extract_decorators(node: &Node, source: &str) -> Vec<String> {
    let mut decorators = Vec::new();
    // Decorators are sibling nodes before the declaration, but in tree-sitter-typescript
    // they appear as children of the declaration node
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "decorator" {
                let text = node_text(&child, source).to_string();
                decorators.push(text);
            }
        }
    }
    decorators
}

fn extract_formal_params(node: &Node, source: &str) -> Vec<Param> {
    let params_node = node.child_by_field_name("parameters")
        .or_else(|| find_child_by_kind(node, "formal_parameters"));
    let params_node = match params_node {
        Some(n) => n,
        None => return vec![],
    };

    let mut params = Vec::new();
    let mut cursor = params_node.walk();
    for child in params_node.children(&mut cursor) {
        match child.kind() {
            "required_parameter" | "optional_parameter" => {
                let name = child.child_by_field_name("pattern")
                    .or_else(|| find_child_by_kind(&child, "identifier"))
                    .map(|n| node_text(&n, source).to_string())
                    .unwrap_or_default();
                let type_name = extract_type_annotation(&child, source).unwrap_or_default();
                if !name.is_empty() {
                    params.push(Param { name, type_name });
                }
            }
            _ => {}
        }
    }
    params
}

fn extract_return_type(node: &Node, source: &str) -> Option<String> {
    node.child_by_field_name("return_type")
        .map(|n| {
            let text = node_text(&n, source).to_string();
            // Strip leading ": " if present
            text.strip_prefix(": ").unwrap_or(&text).to_string()
        })
        .or_else(|| {
            // Look for type_annotation after formal_parameters
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "type_annotation" {
                        let text = node_text(&child, source);
                        return Some(text.strip_prefix(": ").unwrap_or(text).to_string());
                    }
                }
            }
            None
        })
}

fn extract_type_annotation(node: &Node, source: &str) -> Option<String> {
    // Look for type_annotation child
    node.child_by_field_name("type")
        .map(|n| {
            let text = node_text(&n, source).to_string();
            text.strip_prefix(": ").unwrap_or(&text).to_string()
        })
        .or_else(|| {
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    if child.kind() == "type_annotation" {
                        let text = node_text(&child, source);
                        return Some(text.strip_prefix(": ").unwrap_or(text).to_string());
                    }
                }
            }
            None
        })
}

fn extract_heritage_deps(
    node: &Node,
    source: &str,
    path: &Path,
    qualified_name: &str,
    ir: &mut Ir,
) {
    // Heritage clauses can be direct children or inside a class_heritage wrapper
    let mut heritage_nodes = Vec::new();

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            match child.kind() {
                "class_heritage" => {
                    // Recurse into class_heritage to find extends_clause/implements_clause
                    let mut inner = child.walk();
                    for hchild in child.children(&mut inner) {
                        heritage_nodes.push(hchild);
                    }
                }
                "extends_clause" | "extends_type_clause" | "implements_clause" => {
                    heritage_nodes.push(child);
                }
                _ => {}
            }
        }
    }

    for clause in heritage_nodes {
        let dep_kind = DepKind::Implements;
        match clause.kind() {
            "extends_clause" | "extends_type_clause" => {
                extract_type_refs_from_clause(&clause, source, path, qualified_name, dep_kind, ir);
            }
            "implements_clause" => {
                extract_type_refs_from_clause(&clause, source, path, qualified_name, dep_kind, ir);
            }
            _ => {}
        }
    }
}

fn extract_type_refs_from_clause(
    clause: &Node,
    source: &str,
    path: &Path,
    qualified_name: &str,
    dep_kind: DepKind,
    ir: &mut Ir,
) {
    let mut cursor = clause.walk();
    for child in clause.children(&mut cursor) {
        match child.kind() {
            "identifier" | "type_identifier" => {
                let name = node_text(&child, source).to_string();
                // Skip keywords
                if name != "extends" && name != "implements" {
                    ir.dependencies.push(Dependency {
                        from_qualified: qualified_name.to_string(),
                        to_name: name,
                        kind: dep_kind.clone(),
                        loc: loc(&child, path),
                    });
                }
            }
            "generic_type" => {
                if let Some(type_id) = child.child(0) {
                    let name = node_text(&type_id, source).to_string();
                    ir.dependencies.push(Dependency {
                        from_qualified: qualified_name.to_string(),
                        to_name: name,
                        kind: dep_kind.clone(),
                        loc: loc(&child, path),
                    });
                }
            }
            _ => {}
        }
    }
}

fn build_function_signature(node: &Node, source: &str) -> String {
    let name = get_name(node, source).unwrap_or_default();
    let params = node.child_by_field_name("parameters")
        .or_else(|| find_child_by_kind(node, "formal_parameters"))
        .map(|n| node_text(&n, source))
        .unwrap_or("()");
    let ret = extract_return_type(node, source)
        .map(|r| format!(": {}", r))
        .unwrap_or_default();
    format!("function {}{}{}", name, params, ret)
}

fn build_arrow_signature(node: &Node, source: &str) -> String {
    let params = node.child_by_field_name("parameters")
        .or_else(|| find_child_by_kind(node, "formal_parameters"))
        .map(|n| node_text(&n, source))
        .unwrap_or("()");
    let ret = extract_return_type(node, source)
        .map(|r| format!(": {}", r))
        .unwrap_or_default();
    format!("{}{} => ...", params, ret)
}

fn build_method_signature(node: &Node, source: &str) -> String {
    let name = get_name(node, source).unwrap_or_default();
    let params = node.child_by_field_name("parameters")
        .or_else(|| find_child_by_kind(node, "formal_parameters"))
        .map(|n| node_text(&n, source))
        .unwrap_or("()");
    let ret = extract_return_type(node, source)
        .map(|r| format!(": {}", r))
        .unwrap_or_default();
    format!("{}{}{}", name, params, ret)
}
