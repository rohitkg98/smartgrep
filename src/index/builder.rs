use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::ir::kinds::{is_callable_kind, is_type_kind};
use crate::ir::names::{dep_qualifier_key, dep_target_key, path_segments};
use crate::ir::types::{DepKind, Dependency, Ir, Symbol};

use super::types::Index;

/// Build an Index from an Ir by constructing all lookup tables.
pub fn build(ir: &Ir) -> Index {
    let symbols = ir.symbols.clone();
    let deps = with_type_deps(&ir.symbols, &ir.dependencies, type_deps(&ir.symbols));

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

/// Split a type string into identifier paths: `&mut crate::x::Index` →
/// `mut`, `crate::x::Index`; `map[int64]*models.User` → `map`, `int64`,
/// `models.User`; `Promise<User[]>` → `Promise`, `User`. Paths keep `::` and
/// `.` separators; anything else (sigils, brackets, generics, `|`, `,`,
/// whitespace, a lone `:`) ends a token.
pub fn type_name_tokens(ty: &str) -> Vec<&str> {
    let bytes = ty.as_bytes();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !is_ident(bytes[i]) {
            i += 1;
            continue;
        }
        let start = i;
        loop {
            while i < bytes.len() && is_ident(bytes[i]) {
                i += 1;
            }
            // Continue through `::` or `.` when an identifier follows.
            let sep = if ty[i..].starts_with("::") {
                2
            } else if ty[i..].starts_with('.') {
                1
            } else {
                0
            };
            if sep > 0 && i + sep < bytes.len() && is_ident(bytes[i + sep]) {
                i += sep;
            } else {
                break;
            }
        }
        let tok = &ty[start..i];
        if !tok.as_bytes()[0].is_ascii_digit() {
            out.push(tok);
        }
    }
    out
}

/// Type-reference deps derived from what parsers already extract:
/// - `TypeRef` from every callable (fn/func/function/def/method) to each
///   project type named in its parameter types or return type;
/// - `FieldType` from every type-kind symbol to each project type named in
///   its field types.
///
/// A token counts only if its last path segment is the name of a type-kind
/// symbol defined in the project, which drops primitives, std/library
/// wrappers (`Option`, `Vec`, `List`, `Promise`, `Iterable`) and external
/// types. `to_name` is the token as written (`crate::index::types::Index`,
/// `models.User`), deduped per `(from, to_name)`. Recursive types
/// (`next: Option<Box<Node>>` in `Node`) keep their self-reference: it is a
/// real use of the type. Params carry no location, so `loc` is the symbol's.
pub fn type_deps(symbols: &[Symbol]) -> Vec<Dependency> {
    let type_names: HashSet<&str> = symbols
        .iter()
        .filter(|s| is_type_kind(&s.kind))
        .map(|s| s.name.as_str())
        .collect();
    if type_names.is_empty() {
        return Vec::new();
    }

    let mut seen: HashSet<(&str, &str)> = HashSet::new();
    let mut out = Vec::new();
    for sym in symbols {
        let (kind, types): (DepKind, Vec<&str>) = if is_callable_kind(&sym.kind) {
            let mut t: Vec<&str> = sym.params.iter().map(|p| p.type_name.as_str()).collect();
            t.extend(sym.return_type.as_deref());
            (DepKind::TypeRef, t)
        } else if is_type_kind(&sym.kind) && !sym.fields.is_empty() {
            (DepKind::FieldType, sym.fields.iter().map(|f| f.type_name.as_str()).collect())
        } else {
            continue;
        };
        for ty in types {
            for tok in type_name_tokens(ty) {
                let is_project_type = path_segments(tok)
                    .last()
                    .map_or(false, |last| type_names.contains(last));
                if is_project_type && seen.insert((sym.qualified_name.as_str(), tok)) {
                    out.push(Dependency {
                        from_qualified: sym.qualified_name.clone(),
                        to_name: tok.to_string(),
                        kind: kind.clone(),
                        loc: sym.loc.clone(),
                    });
                }
            }
        }
    }
    out
}

/// Merge derived deps into the parser deps, keeping deps grouped by file in
/// the order files first appear (symbols, then deps — both follow the parse
/// order): each file's derived deps follow its parser deps, so per-file
/// output (`refs`, `deps`) stays grouped.
fn with_type_deps(symbols: &[Symbol], parsed: &[Dependency], derived: Vec<Dependency>) -> Vec<Dependency> {
    if derived.is_empty() {
        return parsed.to_vec();
    }
    let mut file_rank: HashMap<&PathBuf, usize> = HashMap::new();
    let files = symbols
        .iter()
        .map(|s| &s.loc.file)
        .chain(parsed.iter().chain(derived.iter()).map(|d| &d.loc.file));
    for f in files {
        let next = file_rank.len();
        file_rank.entry(f).or_insert(next);
    }
    let mut all: Vec<(usize, &Dependency)> = parsed
        .iter()
        .chain(derived.iter())
        .map(|d| (file_rank[&d.loc.file], d))
        .collect();
    // Stable: parser order within a file is kept, derived deps come after.
    all.sort_by_key(|(rank, _)| *rank);
    all.into_iter().map(|(_, d)| d.clone()).collect()
}
