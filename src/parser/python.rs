use std::path::Path;

use anyhow::Result;
use tree_sitter::{Node, Parser};

use crate::ir::types::*;
use crate::parser::common::{loc, node_text};

/// Parse a Python source file (`.py` / `.pyi`) and return the IR.
///
/// Kinds produced (Python-native vocabulary):
/// - `def`    module-level function (sync or async)
/// - `class`  class definition (nested classes get `parent` = outer class)
/// - `method` function defined directly in a class body (`parent` = class name)
/// - `const`  module-level assignment to an UPPER_SNAKE_CASE name
/// - `type`   PEP 695 `type X = ...` or `X: TypeAlias = ...`
///
/// Functions nested inside functions are skipped.
pub fn parse_file(path: &Path, source: &str) -> Result<Ir> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_python::LANGUAGE.into())?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter failed to parse {}", path.display()))?;

    let module = module_path(path);
    let ctx = Ctx {
        source,
        path,
        package: package_path(path, &module),
        module,
    };

    let mut ir = Ir::default();
    for stmt in statements(tree.root_node()) {
        ctx.module_statement(stmt, &mut ir);
    }
    Ok(ir)
}

/// Dotted module path derived from a (project-relative) file path.
/// `src/app/models/user.py` → `app.models.user`; `pkg/__init__.py` → `pkg`.
pub fn module_path(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let s = s.strip_prefix("./").unwrap_or(&s);
    let s = s.strip_prefix("src/").unwrap_or(s);
    let s = s
        .strip_suffix(".pyi")
        .or_else(|| s.strip_suffix(".py"))
        .unwrap_or(s);
    let mut parts: Vec<&str> = s.split('/').filter(|p| !p.is_empty()).collect();
    if parts.last() == Some(&"__init__") {
        parts.pop();
    }
    parts.join(".")
}

/// The package a module lives in (used to resolve relative imports).
fn package_path(path: &Path, module: &str) -> Vec<String> {
    let parts: Vec<String> = if module.is_empty() {
        vec![]
    } else {
        module.split('.').map(String::from).collect()
    };
    let is_init = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map_or(false, |s| s == "__init__");
    if is_init {
        parts
    } else {
        let mut p = parts;
        p.pop();
        p
    }
}

fn is_dunder(name: &str) -> bool {
    name.len() > 4 && name.starts_with("__") && name.ends_with("__")
}

/// Visibility by naming convention: dunder → public, leading `_` → private.
fn visibility(name: &str) -> Visibility {
    if is_dunder(name) {
        Visibility::Public
    } else if name.starts_with('_') {
        Visibility::Private
    } else {
        Visibility::Public
    }
}

/// UPPER_SNAKE_CASE (optionally with leading underscores), at least one letter.
fn is_constant_name(name: &str) -> bool {
    let body = name.trim_start_matches('_');
    body.len() >= 2
        && body.chars().next().map_or(false, |c| c.is_ascii_uppercase())
        && body
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Collapse all runs of whitespace (including newlines) into single spaces.
fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate(s: String, max: usize) -> String {
    if s.chars().count() <= max {
        s
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{}...", cut.trim_end())
    }
}

/// Statements of a block-like node, flattening compound statements whose
/// bodies still belong to the same scope (`if`/`try`/`with` blocks, e.g.
/// `if TYPE_CHECKING:` imports or version-conditional definitions).
fn statements<'a>(node: Node<'a>) -> Vec<Node<'a>> {
    let mut out = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "if_statement" | "try_statement" | "with_statement" => {
                for block in scope_blocks(child) {
                    out.extend(statements(block));
                }
            }
            _ => out.push(child),
        }
    }
    out
}

/// Blocks directly owned by a compound statement or its clauses.
fn scope_blocks<'a>(node: Node<'a>) -> Vec<Node<'a>> {
    let mut out = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "block" {
            out.push(child);
        } else if child.kind().ends_with("_clause") {
            let mut c2 = child.walk();
            for inner in child.named_children(&mut c2) {
                if inner.kind() == "block" {
                    out.push(inner);
                }
            }
        }
    }
    out
}

struct Ctx<'s> {
    source: &'s str,
    path: &'s Path,
    module: String,
    package: Vec<String>,
}

impl<'s> Ctx<'s> {
    fn text(&self, node: &Node) -> &'s str {
        node_text(node, self.source)
    }

    fn qualify(&self, name: &str) -> String {
        if self.module.is_empty() {
            name.to_string()
        } else {
            format!("{}.{}", self.module, name)
        }
    }

    fn import_from(&self) -> String {
        if self.module.is_empty() {
            "(file)".to_string()
        } else {
            self.module.clone()
        }
    }

    // -----------------------------------------------------------------------
    // Module scope
    // -----------------------------------------------------------------------

    fn module_statement(&self, stmt: Node, ir: &mut Ir) {
        match stmt.kind() {
            "import_statement" => self.import_statement(stmt, ir),
            "import_from_statement" => self.import_from_statement(stmt, ir),
            "function_definition" => {
                let sym = self.function(stmt, &[], None);
                ir.symbols.push(sym);
            }
            "class_definition" => self.class(stmt, &[], None, ir),
            "decorated_definition" => {
                let decorators = self.decorators(stmt);
                if let Some(def) = stmt.child_by_field_name("definition") {
                    match def.kind() {
                        "function_definition" => {
                            let sym = self.function(def, &decorators, None);
                            ir.symbols.push(sym);
                        }
                        "class_definition" => self.class(def, &decorators, None, ir),
                        _ => {}
                    }
                }
            }
            "type_alias_statement" => {
                if let Some(sym) = self.type_alias_statement(stmt) {
                    ir.symbols.push(sym);
                }
            }
            "expression_statement" => {
                if let Some(assign) = first_named_child_of_kind(stmt, "assignment") {
                    if let Some(sym) = self.module_assignment(assign) {
                        ir.symbols.push(sym);
                    }
                }
            }
            _ => {}
        }
    }

    fn module_assignment(&self, assign: Node) -> Option<Symbol> {
        let left = assign.child_by_field_name("left")?;
        if left.kind() != "identifier" {
            return None;
        }
        let name = self.text(&left).to_string();
        let annotation = assign.child_by_field_name("type").map(|t| self.text(&t));
        let is_alias = annotation.map_or(false, |t| {
            let t = t.trim();
            t == "TypeAlias" || t.ends_with(".TypeAlias")
        });
        // `T = TypeVar("T")` and friends are type parameters, not constants;
        // `UserId = NewType("UserId", int)` defines a type.
        let called = assign
            .child_by_field_name("right")
            .filter(|r| r.kind() == "call")
            .and_then(|r| r.child_by_field_name("function"))
            .map(|f| {
                let t = self.text(&f);
                t.rsplit('.').next().unwrap_or(t).to_string()
            });
        let kind = if matches!(called.as_deref(), Some("TypeVar" | "ParamSpec" | "TypeVarTuple")) {
            return None;
        } else if is_alias || called.as_deref() == Some("NewType") || self.is_implicit_alias(&name, assign) {
            "type"
        } else if is_constant_name(&name) {
            "const"
        } else {
            return None;
        };
        let mut sym = Symbol::new(
            name.clone(),
            self.qualify(&name),
            kind,
            loc(&assign, self.path),
            visibility(&name),
        );
        sym.signature = Some(truncate(squash(self.text(&assign)), 120));
        Some(sym)
    }

    /// Pre-PEP 613 implicit alias: `ResponseValue = t.Union[str, bytes]`.
    /// Heuristic: PascalCase name (has a lowercase letter) and a subscripted
    /// right-hand side, with no annotation.
    fn is_implicit_alias(&self, name: &str, assign: Node) -> bool {
        if assign.child_by_field_name("type").is_some() {
            return false;
        }
        let pascal = name.chars().next().map_or(false, |c| c.is_ascii_uppercase())
            && name.chars().any(|c| c.is_ascii_lowercase())
            && !name.contains('_');
        pascal
            && assign
                .child_by_field_name("right")
                .map_or(false, |r| r.kind() == "subscript")
    }

    fn type_alias_statement(&self, stmt: Node) -> Option<Symbol> {
        let left = stmt.child_by_field_name("left")?;
        let name_node = first_identifier(left)?;
        let name = self.text(&name_node).to_string();
        let mut sym = Symbol::new(
            name.clone(),
            self.qualify(&name),
            "type",
            loc(&stmt, self.path),
            visibility(&name),
        );
        sym.signature = Some(truncate(squash(self.text(&stmt)), 120));
        Some(sym)
    }

    // -----------------------------------------------------------------------
    // Imports
    // -----------------------------------------------------------------------

    fn push_import(&self, to_name: String, node: &Node, ir: &mut Ir) {
        ir.dependencies.push(Dependency {
            from_qualified: self.import_from(),
            to_name,
            kind: DepKind::Import,
            loc: loc(node, self.path),
        });
    }

    /// `import a.b`, `import a.b as c` → one Import dep per module.
    fn import_statement(&self, stmt: Node, ir: &mut Ir) {
        let mut cursor = stmt.walk();
        for name in stmt.children_by_field_name("name", &mut cursor) {
            let module = match name.kind() {
                "aliased_import" => name.child_by_field_name("name").map(|n| self.text(&n)),
                _ => Some(self.text(&name)),
            };
            if let Some(m) = module {
                self.push_import(m.to_string(), &stmt, ir);
            }
        }
    }

    /// `from a.b import c, d as e` → `a.b.c`, `a.b.d`; `from a import *` → `a.*`.
    /// Relative imports are resolved against this file's package when possible.
    fn import_from_statement(&self, stmt: Node, ir: &mut Ir) {
        let module_node = match stmt.child_by_field_name("module_name") {
            Some(m) => m,
            None => return,
        };
        let module = if module_node.kind() == "relative_import" {
            self.resolve_relative(module_node)
        } else {
            self.text(&module_node).to_string()
        };

        let join = |name: &str| {
            if module.is_empty() {
                name.to_string()
            } else if module.ends_with('.') {
                format!("{}{}", module, name)
            } else {
                format!("{}.{}", module, name)
            }
        };

        let mut any = false;
        let mut cursor = stmt.walk();
        for name in stmt.children_by_field_name("name", &mut cursor) {
            let imported = match name.kind() {
                "aliased_import" => name.child_by_field_name("name").map(|n| self.text(&n)),
                _ => Some(self.text(&name)),
            };
            if let Some(n) = imported {
                self.push_import(join(n), &stmt, ir);
                any = true;
            }
        }
        if !any && first_named_child_of_kind(stmt, "wildcard_import").is_some() {
            self.push_import(join("*"), &stmt, ir);
        }
    }

    /// Resolve `.`, `..pkg` etc. to an absolute dotted path using the file's package.
    /// Falls back to the raw text (e.g. `..pkg`) if it climbs above the project root.
    fn resolve_relative(&self, node: Node) -> String {
        let raw = self.text(&node).to_string();
        let dots = raw.chars().take_while(|c| *c == '.').count();
        let rest = raw[dots..].trim();
        let up = dots - 1;
        if up > self.package.len() {
            return raw;
        }
        let mut parts: Vec<&str> = self.package[..self.package.len() - up]
            .iter()
            .map(|s| s.as_str())
            .collect();
        if !rest.is_empty() {
            parts.push(rest);
        }
        if parts.is_empty() {
            // `from . import x` in a top-level module: nothing to anchor to.
            return raw;
        }
        parts.join(".")
    }

    // -----------------------------------------------------------------------
    // Decorators
    // -----------------------------------------------------------------------

    fn decorators(&self, decorated: Node) -> Vec<String> {
        let mut out = Vec::new();
        let mut cursor = decorated.walk();
        for child in decorated.named_children(&mut cursor) {
            if child.kind() == "decorator" {
                out.push(squash(self.text(&child)));
            }
        }
        out
    }

    // -----------------------------------------------------------------------
    // Functions / methods
    // -----------------------------------------------------------------------

    /// Build a `def` (module level) or `method` (when `class` is given) symbol.
    fn function(
        &self,
        node: Node,
        decorators: &[String],
        class: Option<(&str, &str)>, // (class name, class qualified name)
    ) -> Symbol {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.text(&n).to_string())
            .unwrap_or_default();
        let is_async = node.child(0).map_or(false, |c| c.kind() == "async");
        let is_static = decorators
            .iter()
            .any(|d| d == "@staticmethod" || d.ends_with(".staticmethod"));

        let params_node = node.child_by_field_name("parameters");
        let mut params = params_node.map(|p| self.params(p)).unwrap_or_default();
        if class.is_some() && !is_static {
            if let Some(first) = params.first() {
                if matches!(first.name.as_str(), "self" | "cls" | "mcs" | "mcls" | "metacls") {
                    params.remove(0);
                }
            }
        }

        let return_type = node
            .child_by_field_name("return_type")
            .map(|r| squash(self.text(&r)));

        let type_params = node
            .child_by_field_name("type_parameters")
            .map(|t| squash(self.text(&t)))
            .unwrap_or_default();
        let sig_params = params_node
            .map(|p| self.signature_params(p))
            .unwrap_or_default();
        let signature = format!(
            "{}def {}{}({}){}",
            if is_async { "async " } else { "" },
            name,
            type_params,
            sig_params,
            return_type
                .as_ref()
                .map(|r| format!(" -> {}", r))
                .unwrap_or_default()
        );

        let (kind, qn, parent) = match class {
            Some((cname, cqn)) => ("method", format!("{}.{}", cqn, name), Some(cname.to_string())),
            None => ("def", self.qualify(&name), None),
        };

        let mut sym = Symbol::new(name.clone(), qn, kind, loc(&node, self.path), visibility(&name));
        sym.parent = parent;
        sym.params = params;
        sym.return_type = return_type;
        sym.signature = Some(signature);
        sym.attributes = decorators.to_vec();
        if is_async {
            sym.attributes.push("async".to_string());
        }
        sym
    }

    fn params(&self, params_node: Node) -> Vec<Param> {
        let mut out = Vec::new();
        let mut cursor = params_node.walk();
        for p in params_node.named_children(&mut cursor) {
            let (name, type_name) = match p.kind() {
                "identifier" => (self.text(&p).to_string(), String::new()),
                "list_splat_pattern" | "dictionary_splat_pattern" => {
                    (self.text(&p).to_string(), String::new())
                }
                "typed_parameter" => {
                    let name = p
                        .named_child(0)
                        .map(|n| self.text(&n).to_string())
                        .unwrap_or_default();
                    let ty = p
                        .child_by_field_name("type")
                        .map(|t| squash(self.text(&t)))
                        .unwrap_or_default();
                    (name, ty)
                }
                "default_parameter" | "typed_default_parameter" => {
                    let name = p
                        .child_by_field_name("name")
                        .map(|n| self.text(&n).to_string())
                        .unwrap_or_default();
                    let ty = p
                        .child_by_field_name("type")
                        .map(|t| squash(self.text(&t)))
                        .unwrap_or_default();
                    (name, ty)
                }
                "tuple_pattern" => (squash(self.text(&p)), String::new()),
                // keyword_separator (`*`), positional_separator (`/`), comments
                _ => continue,
            };
            if !name.is_empty() {
                out.push(Param { name, type_name });
            }
        }
        out
    }

    /// Parameter list text for signatures: every parameter as written
    /// (defaults and separators included), whitespace-normalized, comments dropped.
    fn signature_params(&self, params_node: Node) -> String {
        let mut parts = Vec::new();
        let mut cursor = params_node.walk();
        for p in params_node.named_children(&mut cursor) {
            if p.kind() == "comment" {
                continue;
            }
            parts.push(squash(self.text(&p)));
        }
        parts.join(", ")
    }

    // -----------------------------------------------------------------------
    // Classes
    // -----------------------------------------------------------------------

    fn class(&self, node: Node, decorators: &[String], outer: Option<(&str, &str)>, ir: &mut Ir) {
        let name = match node.child_by_field_name("name") {
            Some(n) => self.text(&n).to_string(),
            None => return,
        };
        let qn = match outer {
            Some((_, oqn)) => format!("{}.{}", oqn, name),
            None => self.qualify(&name),
        };

        // Base classes → Implements deps
        let mut bases_text = Vec::new();
        if let Some(supers) = node.child_by_field_name("superclasses") {
            let mut cursor = supers.walk();
            for arg in supers.named_children(&mut cursor) {
                if arg.kind() == "comment" {
                    continue;
                }
                bases_text.push(squash(self.text(&arg)));
                if let Some(base) = base_name(arg, self.source) {
                    ir.dependencies.push(Dependency {
                        from_qualified: qn.clone(),
                        to_name: base,
                        kind: DepKind::Implements,
                        loc: loc(&arg, self.path),
                    });
                }
            }
        }

        let type_params = node
            .child_by_field_name("type_parameters")
            .map(|t| squash(self.text(&t)))
            .unwrap_or_default();
        let signature = if bases_text.is_empty() {
            format!("class {}{}", name, type_params)
        } else {
            format!("class {}{}({})", name, type_params, bases_text.join(", "))
        };

        // Push the class before its members so listings read top-down.
        let mut sym = Symbol::new(name.clone(), qn.clone(), "class", loc(&node, self.path), visibility(&name));
        sym.parent = outer.map(|(n, _)| n.to_string());
        sym.signature = Some(signature);
        sym.attributes = decorators.to_vec();
        let idx = ir.symbols.len();
        ir.symbols.push(sym);

        let mut fields: Vec<Field> = Vec::new();
        if let Some(body) = node.child_by_field_name("body") {
            for stmt in statements(body) {
                self.class_statement(stmt, &name, &qn, &mut fields, ir);
            }
        }
        ir.symbols[idx].fields = fields;
    }

    fn class_statement(
        &self,
        stmt: Node,
        class_name: &str,
        class_qn: &str,
        fields: &mut Vec<Field>,
        ir: &mut Ir,
    ) {
        let (def, decorators) = match stmt.kind() {
            "decorated_definition" => match stmt.child_by_field_name("definition") {
                Some(d) => (d, self.decorators(stmt)),
                None => return,
            },
            "function_definition" | "class_definition" => (stmt, vec![]),
            "expression_statement" => {
                if let Some(assign) = first_named_child_of_kind(stmt, "assignment") {
                    self.class_field(assign, fields);
                }
                return;
            }
            _ => return,
        };

        match def.kind() {
            "function_definition" => {
                let sym = self.function(def, &decorators, Some((class_name, class_qn)));
                if sym.name == "__init__" {
                    if let Some(body) = def.child_by_field_name("body") {
                        self.init_fields(body, fields);
                    }
                }
                ir.symbols.push(sym);
            }
            "class_definition" => self.class(def, &decorators, Some((class_name, class_qn)), ir),
            _ => {}
        }
    }

    /// Class-body `x: int = 0` / `x = 0` → field `x`.
    fn class_field(&self, assign: Node, fields: &mut Vec<Field>) {
        let left = match assign.child_by_field_name("left") {
            Some(l) if l.kind() == "identifier" => l,
            _ => return,
        };
        let name = self.text(&left).to_string();
        let ty = assign
            .child_by_field_name("type")
            .map(|t| squash(self.text(&t)))
            .unwrap_or_default();
        push_field(fields, name, ty);
    }

    /// `self.x = ...` / `self.x: T = ...` anywhere in `__init__` (not in nested defs).
    fn init_fields(&self, block: Node, fields: &mut Vec<Field>) {
        let mut cursor = block.walk();
        for child in block.named_children(&mut cursor) {
            match child.kind() {
                "function_definition" | "class_definition" | "decorated_definition" => continue,
                "assignment" => self.self_assignment(child, fields),
                _ => self.init_fields(child, fields),
            }
        }
    }

    /// Record `self.x` targets of an assignment, following chains (`self.a = self.b = v`).
    fn self_assignment(&self, assign: Node, fields: &mut Vec<Field>) {
        if let Some(left) = assign.child_by_field_name("left") {
            if left.kind() == "attribute" {
                let obj = left.child_by_field_name("object").map(|o| self.text(&o));
                let attr = left.child_by_field_name("attribute").map(|a| self.text(&a));
                if let (Some("self"), Some(attr)) = (obj, attr) {
                    let ty = assign
                        .child_by_field_name("type")
                        .map(|t| squash(self.text(&t)))
                        .unwrap_or_default();
                    push_field(fields, attr.to_string(), ty);
                }
            }
        }
        if let Some(right) = assign.child_by_field_name("right") {
            if right.kind() == "assignment" {
                self.self_assignment(right, fields);
            }
        }
    }
}

/// Add a field unless one with the same name exists; fill in a missing type.
/// Dunder attributes (`__slots__`, `__hash__ = None`, ...) are protocol hooks,
/// not data fields, and are skipped.
fn push_field(fields: &mut Vec<Field>, name: String, type_name: String) {
    if is_dunder(&name) {
        return;
    }
    if let Some(existing) = fields.iter_mut().find(|f| f.name == name) {
        if existing.type_name.is_empty() && !type_name.is_empty() {
            existing.type_name = type_name;
        }
        return;
    }
    let vis = visibility(&name);
    fields.push(Field { name, type_name, visibility: vis });
}

/// Name of a base class expression: `Bar` → Bar, `mod.Baz` → Baz,
/// `Generic[T]` → Generic. Keyword args (`metaclass=M`) and splats → None.
fn base_name(node: Node, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" => Some(node_text(&node, source).to_string()),
        "attribute" => node
            .child_by_field_name("attribute")
            .map(|a| node_text(&a, source).to_string()),
        "subscript" => node.child_by_field_name("value").and_then(|v| base_name(v, source)),
        "call" => node.child_by_field_name("function").and_then(|f| base_name(f, source)),
        _ => None,
    }
}

fn first_named_child_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let found = node.named_children(&mut cursor).find(|c| c.kind() == kind);
    found
}

fn first_identifier(node: Node) -> Option<Node> {
    if node.kind() == "identifier" {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(id) = first_identifier(child) {
            return Some(id);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_paths() {
        assert_eq!(module_path(Path::new("src/app/models/user.py")), "app.models.user");
        assert_eq!(module_path(Path::new("pkg/__init__.py")), "pkg");
        assert_eq!(module_path(Path::new("__init__.py")), "");
        assert_eq!(module_path(Path::new("stubs/foo.pyi")), "stubs.foo");
        assert_eq!(module_path(Path::new("main.py")), "main");
    }

    #[test]
    fn constant_names() {
        assert!(is_constant_name("MAX"));
        assert!(is_constant_name("MAX_RETRIES_2"));
        assert!(is_constant_name("_PRIVATE"));
        assert!(!is_constant_name("T"));
        assert!(!is_constant_name("logger"));
        assert!(!is_constant_name("__all__"));
        assert!(!is_constant_name("MaxValue"));
        assert!(!is_constant_name("_"));
    }

    #[test]
    fn visibility_rules() {
        assert_eq!(visibility("__init__"), Visibility::Public);
        assert_eq!(visibility("_x"), Visibility::Private);
        assert_eq!(visibility("__x"), Visibility::Private);
        assert_eq!(visibility("x"), Visibility::Public);
    }

    #[test]
    fn relative_import_in_init_resolves_to_package() {
        let ir = parse_file(Path::new("pkg/sub/__init__.py"), "from . import a\nfrom .. import b\n").unwrap();
        let names: Vec<&str> = ir.dependencies.iter().map(|d| d.to_name.as_str()).collect();
        assert_eq!(names, vec!["pkg.sub.a", "pkg.b"]);
    }

    #[test]
    fn relative_import_above_root_keeps_raw() {
        let ir = parse_file(Path::new("mod.py"), "from ..x import y\n").unwrap();
        assert_eq!(ir.dependencies[0].to_name, "..x.y");
    }
}
