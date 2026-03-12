use std::collections::HashMap;
use std::path::PathBuf;

use crate::ir::types::Ir;

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
        reverse_deps
            .entry(dep.to_name.clone())
            .or_default()
            .push(i);
    }

    Index {
        symbols,
        deps,
        name_lookup,
        file_lookup,
        qualified_lookup,
        reverse_deps,
    }
}
