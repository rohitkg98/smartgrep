use std::collections::HashMap;
use std::path::PathBuf;

use crate::ir::names::{dep_qualifier_key, dep_target_key};
use crate::ir::types::{DepKind, Ir};

use super::types::Index;

/// Build an Index from an Ir by constructing all lookup tables.
pub fn build(ir: &Ir) -> Index {
    let symbols = ir.symbols.clone();
    let deps = ir.dependencies.clone();

    let mut name_lookup: HashMap<String, Vec<usize>> = HashMap::new();
    let mut file_lookup: HashMap<PathBuf, Vec<usize>> = HashMap::new();
    let mut qualified_lookup: HashMap<String, usize> = HashMap::new();

    for (i, sym) in symbols.iter().enumerate() {
        name_lookup
            .entry(sym.name.clone())
            .or_default()
            .push(i);

        file_lookup
            .entry(sym.loc.file.clone())
            .or_default()
            .push(i);

        // For qualified names, last-writer-wins if duplicates exist (e.g. multiple impl blocks)
        qualified_lookup.insert(sym.qualified_name.clone(), i);
    }

    let mut reverse_deps: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, dep) in deps.iter().enumerate() {
        let key = dep_target_key(&dep.to_name);
        if key.is_empty() {
            continue;
        }
        // Calls through a path (`User::new`, `fmt.Println`) are also refs to
        // their owner, so `refs User` lists constructor/associated-fn calls.
        if dep.kind == DepKind::Call {
            if let Some(owner) = dep_qualifier_key(&dep.to_name) {
                if owner != key {
                    reverse_deps.entry(owner).or_default().push(i);
                }
            }
        }
        reverse_deps.entry(key).or_default().push(i);
    }

    Index {
        version: super::types::INDEX_VERSION,
        symbols,
        deps,
        name_lookup,
        file_lookup,
        qualified_lookup,
        reverse_deps,
    }
}
