use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::ir::types::{Dependency, Symbol};

pub const INDEX_VERSION: u32 = 2;

/// The queryable index: symbols + dependencies + lookup tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
    #[serde(default = "default_version")]
    pub version: u32,
    pub symbols: Vec<Symbol>,
    pub deps: Vec<Dependency>,
    /// name → symbol indices (multiple symbols can share a name)
    pub name_lookup: HashMap<String, Vec<usize>>,
    /// file → symbol indices
    pub file_lookup: HashMap<PathBuf, Vec<usize>>,
    /// qualified_name → symbol index
    pub qualified_lookup: HashMap<String, usize>,
    /// qualified_name → dep indices where dep.to_name matches
    pub reverse_deps: HashMap<String, Vec<usize>>,
}

fn default_version() -> u32 {
    0
}

impl Index {
    /// Look up symbols by short name.
    pub fn by_name(&self, name: &str) -> Vec<&Symbol> {
        self.name_lookup
            .get(name)
            .map(|indices| indices.iter().map(|&i| &self.symbols[i]).collect())
            .unwrap_or_default()
    }

    /// Look up symbols defined in a given file.
    pub fn by_file(&self, file: &PathBuf) -> Vec<&Symbol> {
        self.file_lookup
            .get(file)
            .map(|indices| indices.iter().map(|&i| &self.symbols[i]).collect())
            .unwrap_or_default()
    }

    /// Look up a symbol by its fully qualified name.
    pub fn by_qualified(&self, qn: &str) -> Option<&Symbol> {
        self.qualified_lookup.get(qn).map(|&i| &self.symbols[i])
    }

    /// Look up all symbols of a given kind.
    pub fn by_kind(&self, kind: &str) -> Vec<&Symbol> {
        self.symbols.iter().filter(|s| s.kind == kind).collect()
    }

    /// Look up symbols matching any of the given kinds.
    pub fn by_kinds(&self, kinds: &[&str]) -> Vec<&Symbol> {
        self.symbols.iter().filter(|s| kinds.contains(&s.kind.as_str())).collect()
    }

    /// Infer which languages are present from file extensions.
    pub fn languages(&self) -> Vec<&'static str> {
        let mut langs = std::collections::HashSet::new();
        for file in self.file_lookup.keys() {
            match file.extension().and_then(|e| e.to_str()) {
                Some("rs") => { langs.insert("rust"); }
                Some("java") => { langs.insert("java"); }
                Some("go") => { langs.insert("go"); }
                _ => {}
            }
        }
        langs.into_iter().collect()
    }

    /// Get outgoing dependencies from a symbol (by qualified name).
    pub fn deps_of(&self, qn: &str) -> Vec<&Dependency> {
        self.deps
            .iter()
            .filter(|d| d.from_qualified == qn)
            .collect()
    }

    /// Get incoming references to a name (dependencies where to_name matches).
    pub fn refs_to(&self, name: &str) -> Vec<&Dependency> {
        self.reverse_deps
            .get(name)
            .map(|indices| indices.iter().map(|&i| &self.deps[i]).collect())
            .unwrap_or_default()
    }
}
