use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;

/// The kind of a symbol extracted from source code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Impl,
    Const,
    TypeAlias,
    Module,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SymbolKind::Function => "fn",
            SymbolKind::Method => "method",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Trait => "trait",
            SymbolKind::Impl => "impl",
            SymbolKind::Const => "const",
            SymbolKind::TypeAlias => "type",
            SymbolKind::Module => "mod",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for SymbolKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "functions" | "function" | "fn" => Ok(SymbolKind::Function),
            "methods" | "method" => Ok(SymbolKind::Method),
            "structs" | "struct" => Ok(SymbolKind::Struct),
            "enums" | "enum" => Ok(SymbolKind::Enum),
            "traits" | "trait" | "interfaces" | "interface" => Ok(SymbolKind::Trait),
            "impls" | "impl" => Ok(SymbolKind::Impl),
            "consts" | "const" => Ok(SymbolKind::Const),
            "types" | "type" => Ok(SymbolKind::TypeAlias),
            "modules" | "module" | "mod" => Ok(SymbolKind::Module),
            _ => Err(format!("unknown symbol kind '{}'", s)),
        }
    }
}

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
    pub kind: SymbolKind,
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
        kind: SymbolKind,
        loc: SourceLoc,
        visibility: Visibility,
    ) -> Self {
        Symbol {
            name,
            qualified_name,
            kind,
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
    FunctionCall,
    TypeReference,
    TraitImpl,
    FieldType,
}

impl std::fmt::Display for DepKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            DepKind::Import => "import",
            DepKind::FunctionCall => "call",
            DepKind::TypeReference => "type_ref",
            DepKind::TraitImpl => "trait_impl",
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
