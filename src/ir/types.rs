use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Visibility of a symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Crate,
    Private,
}

/// A source location: file path plus line and column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLoc {
    pub file: PathBuf,
    pub line: usize,
    pub col: usize,
}

/// A struct/enum field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub type_name: String,
    pub visibility: Visibility,
}

/// A function/method parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub type_name: String,
}

/// A symbol extracted from source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub qualified_name: String,
    pub kind: String,
    pub loc: SourceLoc,
    pub visibility: Visibility,
    pub signature: Option<String>,
    pub parent: Option<String>,
    pub attributes: Vec<String>,
    pub fields: Vec<Field>,
    pub params: Vec<Param>,
    pub return_type: Option<String>,
}

impl Symbol {
    /// Create a new Symbol with the required fields, defaulting optional fields.
    pub fn new(
        name: String,
        qualified_name: String,
        kind: impl Into<String>,
        loc: SourceLoc,
        visibility: Visibility,
    ) -> Self {
        Symbol {
            name,
            qualified_name,
            kind: kind.into(),
            loc,
            visibility,
            signature: None,
            parent: None,
            attributes: vec![],
            fields: vec![],
            params: vec![],
            return_type: None,
        }
    }
}

/// The kind of dependency between symbols.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DepKind {
    Import,
    Call,
    TypeRef,
    Implements,
    FieldType,
}

impl std::fmt::Display for DepKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            DepKind::Import => "import",
            DepKind::Call => "call",
            DepKind::TypeRef => "type_ref",
            DepKind::Implements => "implements",
            DepKind::FieldType => "field_type",
        };
        write!(f, "{}", s)
    }
}

/// A dependency from one symbol to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub from_qualified: String,
    pub to_name: String,
    pub kind: DepKind,
    pub loc: SourceLoc,
}

/// The intermediate representation: a collection of symbols and dependencies
/// extracted from source files.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Ir {
    pub symbols: Vec<Symbol>,
    pub dependencies: Vec<Dependency>,
}
