use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use crate::ir::names::{dep_matches, dep_target_key, strip_generics};
use crate::ir::names::path_segments;
use crate::ir::types::{Dependency, Symbol};

/// Bump when the index schema or the set of indexed languages changes, so
/// existing on-disk indexes are rebuilt (e.g. v3: Python files are now indexed;
/// v4: call deps, per-leaf grouped imports, `reverse_deps` keyed by
/// [`dep_target_key`]; v5: TypeScript imports recorded per imported name;
/// v6: `TypeRef` / `FieldType` deps derived by the builder).
pub const INDEX_VERSION: u32 = 6;

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
    /// [`dep_target_key`] of `dep.to_name` → dep indices
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
            if let Some(lang) = crate::lang::language_for_path(file) {
                langs.insert(lang.name);
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

    /// Get incoming references to a name.
    ///
    /// Deps are matched on their normalized target (see [`dep_target_key`]):
    /// - a bare name (`Index`, `ensure_index`) matches every dep whose last
    ///   path segment is that name (`crate::index::types::Index`, `Index`,
    ///   `auto::ensure_index`, `Processor<String>`, ...);
    /// - a qualified name (`Symbol::new`, `fmt.Println`) additionally requires
    ///   the dep's path to end with the query's segments, treating `::`, `.`
    ///   and `/` as the same separator (see [`dep_matches`]).
    pub fn refs_to(&self, name: &str) -> Vec<&Dependency> {
        let key = dep_target_key(name);
        if key.is_empty() {
            return Vec::new();
        }
        let qualified = path_segments(&strip_generics(name.trim())).len() > 1;
        self.reverse_deps
            .get(&key)
            .map(|indices| {
                indices
                    .iter()
                    .map(|&i| &self.deps[i])
                    .filter(|d| !qualified || dep_matches(&d.to_name, name))
                    .collect()
            })
            .unwrap_or_default()
    }
}
