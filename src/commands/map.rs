use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use crate::format::OutputFormat;
use crate::index::auto;
use crate::index::types::Index;
use crate::ir::types::{Symbol, SymbolKind, Visibility};

pub fn run(
    in_path: &Option<String>,
    all: bool,
    depth: Option<usize>,
    include_generated: bool,
    symbols_mode: bool,
    format_str: &str,
    project_root: &Option<PathBuf>,
) -> Result<()> {
    let root = super::resolve_root(project_root)?;
    let start = std::time::Instant::now();
    let index = auto::ensure_index(&root)?;

    // Collect all (abs_file, filtered_symbols) pairs
    let mut file_syms: Vec<(PathBuf, Vec<&Symbol>)> = index
        .file_lookup
        .keys()
        .map(|file| {
            let mut syms: Vec<&Symbol> = index
                .by_file(file)
                .into_iter()
                .filter(|s| {
                    if matches!(s.kind, SymbolKind::Impl | SymbolKind::Method) {
                        return false;
                    }
                    if !all {
                        matches!(s.visibility, Visibility::Public | Visibility::Crate)
                    } else {
                        true
                    }
                })
                .collect();
            syms.sort_by(|a, b| {
                symbol_sort_order(&a.kind)
                    .cmp(&symbol_sort_order(&b.kind))
                    .then(a.name.cmp(&b.name))
            });
            (file.clone(), syms)
        })
        .collect();

    // Filter by in_path
    if let Some(ref path) = in_path {
        file_syms.retain(|(f, _)| f.to_string_lossy().contains(path.as_str()));
    }

    // Exclude generated files unless opted in
    let mut excluded_generated = 0usize;
    if !include_generated {
        let before = file_syms.len();
        file_syms.retain(|(f, _)| !is_generated(f));
        excluded_generated = before - file_syms.len();
    }

    // Sort alphabetically so directory grouping is contiguous
    file_syms.sort_by(|(a, _), (b, _)| a.cmp(b));

    let total_files = file_syms.len();
    let total_symbols: usize = file_syms.iter().map(|(_, s)| s.len()).sum();

    // Build module-path → directory map once; used by dep signal for all groups
    let module_dir_map = build_module_dir_map(&index, &root);

    let output = match format_str.parse::<OutputFormat>().unwrap_or(OutputFormat::Text) {
        OutputFormat::Json => format_json(&file_syms, &index, &root, depth, &module_dir_map),
        OutputFormat::Text => format_text(
            &file_syms,
            &index,
            &root,
            depth,
            symbols_mode,
            total_files,
            total_symbols,
            excluded_generated,
            &module_dir_map,
        ),
    };

    super::log_direct(
        &root,
        "map",
        in_path.as_deref().unwrap_or(""),
        &output,
        start.elapsed().as_millis() as u64,
    );
    println!("{}", output);
    Ok(())
}

// --- Path helpers ---

fn is_generated(path: &Path) -> bool {
    let s = path.to_string_lossy();
    for marker in &["/generated/", "/gen/", "/vendor/", "/third_party/", "/thirdparty/"] {
        if s.contains(marker) {
            return true;
        }
    }
    if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
        let fl = fname.to_lowercase();
        if fl.contains("generated")
            || fl.ends_with(".pb.go")
            || fl == "bindings.rs"
            || fl.ends_with("_pb2.py")
            || fl.ends_with(".pb.swift")
        {
            return true;
        }
    }
    false
}

fn relative_path<'a>(file: &'a Path, root: &Path) -> &'a Path {
    file.strip_prefix(root).unwrap_or(file)
}

/// Return the directory of a relative file path, clamped to `depth` components.
fn dir_at_depth(rel_file: &Path, depth: Option<usize>) -> PathBuf {
    let parent = rel_file.parent().unwrap_or(Path::new(""));
    limit_dir_depth(parent, depth)
}

/// Clamp a relative directory path to `depth` components.
fn limit_dir_depth(dir: &Path, depth: Option<usize>) -> PathBuf {
    match depth {
        None => dir.to_path_buf(),
        Some(d) => dir.components().take(d).collect(),
    }
}

/// Group file_syms by depth-limited directory, preserving sorted order.
/// Returns (dir, indices_into_file_syms).
fn group_by_dir(
    file_syms: &[(PathBuf, Vec<&Symbol>)],
    root: &Path,
    depth: Option<usize>,
) -> Vec<(PathBuf, Vec<usize>)> {
    // Preserve first-seen order while allowing non-contiguous merging at collapsed depth
    let mut order: Vec<PathBuf> = Vec::new();
    let mut map: HashMap<PathBuf, Vec<usize>> = HashMap::new();

    for (i, (file, _)) in file_syms.iter().enumerate() {
        let rel = relative_path(file, root);
        let dir = dir_at_depth(rel, depth);
        if !map.contains_key(&dir) {
            order.push(dir.clone());
        }
        map.entry(dir).or_default().push(i);
    }

    order
        .into_iter()
        .map(|dir| {
            let indices = map.remove(&dir).unwrap();
            (dir, indices)
        })
        .collect()
}

// --- Symbol helpers ---

fn symbol_sort_order(kind: &SymbolKind) -> u8 {
    match kind {
        SymbolKind::Struct
        | SymbolKind::Enum
        | SymbolKind::Trait
        | SymbolKind::TypeAlias
        | SymbolKind::Const => 0,
        _ => 1,
    }
}

fn inline_name(s: &Symbol) -> String {
    match s.kind {
        SymbolKind::Function => s.name.clone(),
        SymbolKind::Struct => format!("struct {}", s.name),
        SymbolKind::Enum => format!("enum {}", s.name),
        SymbolKind::Trait => format!("trait {}", s.name),
        SymbolKind::TypeAlias => format!("type {}", s.name),
        SymbolKind::Const => format!("const {}", s.name),
        SymbolKind::Module => format!("mod {}", s.name),
        _ => s.name.clone(),
    }
}

fn kind_label(kind: &SymbolKind) -> Option<(&'static str, u8)> {
    match kind {
        SymbolKind::Struct => Some(("struct", 0)),
        SymbolKind::Enum => Some(("enum", 1)),
        SymbolKind::Trait => Some(("trait", 2)),
        SymbolKind::TypeAlias => Some(("type", 3)),
        SymbolKind::Const => Some(("const", 4)),
        SymbolKind::Module => Some(("mod", 5)),
        SymbolKind::Function => Some(("fn", 6)),
        _ => None,
    }
}

/// Count symbols per kind for a set of index entries, sorted types-first.
fn count_by_kind(file_syms: &[(PathBuf, Vec<&Symbol>)], indices: &[usize]) -> Vec<(&'static str, usize)> {
    let mut counts: std::collections::BTreeMap<u8, (&'static str, usize)> =
        std::collections::BTreeMap::new();
    for &i in indices {
        for sym in &file_syms[i].1 {
            if let Some((label, order)) = kind_label(&sym.kind) {
                let e = counts.entry(order).or_insert((label, 0));
                e.1 += 1;
            }
        }
    }
    counts.into_values().collect()
}

fn format_counts(counts: &[(&'static str, usize)]) -> String {
    counts
        .iter()
        .map(|(label, n)| format!("{}×{}", label, n))
        .collect::<Vec<_>>()
        .join("  ")
}

// --- Dependency signal ---

/// Build a map from every qualified-name prefix → relative directory.
/// This lets import dep paths like `crate::ir::types` resolve to `src/ir/`.
fn build_module_dir_map(index: &Index, root: &Path) -> HashMap<String, PathBuf> {
    let mut map: HashMap<String, PathBuf> = HashMap::new();
    for sym in &index.symbols {
        let rel = sym.loc.file.strip_prefix(root).unwrap_or(&sym.loc.file);
        let dir = rel.parent().unwrap_or(Path::new("")).to_path_buf();
        let parts: Vec<&str> = sym.qualified_name.split("::").collect();
        // Register every prefix so both `crate::ir` and `crate::ir::types` resolve
        for len in 1..=parts.len() {
            let prefix = parts[..len].join("::");
            map.entry(prefix).or_insert_with(|| dir.clone());
        }
    }
    map
}

/// Resolve an import `to_name` (e.g. `crate::ir::types::*`) to a project directory.
fn resolve_import_dir(to_name: &str, module_dir_map: &HashMap<String, PathBuf>) -> Option<PathBuf> {
    // Normalize: strip wildcard / braces so `crate::ir::types::*` → `crate::ir::types`
    // and `crate::ir::{Symbol, Dep}` → `crate::ir`
    let base = to_name
        .split('{')
        .next()
        .unwrap_or(to_name)
        .trim_end_matches('*')
        .trim_end_matches("::")
        .trim();

    // Try longest prefix first, then progressively shorter
    let parts: Vec<&str> = base.split("::").collect();
    for len in (1..=parts.len()).rev() {
        let prefix = parts[..len].join("::");
        if let Some(dir) = module_dir_map.get(&prefix) {
            return Some(dir.clone());
        }
    }
    None
}

/// For a set of absolute file paths, find which other directories (at depth) they import from.
fn outgoing_dirs(
    abs_files: &[&PathBuf],
    index: &Index,
    root: &Path,
    this_dir: &Path,
    depth: Option<usize>,
    module_dir_map: &HashMap<String, PathBuf>,
) -> Vec<String> {
    let mut dirs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    // Scan all deps whose source file is in this directory group.
    // dep.loc.file is stored relative to project root; abs_files are absolute.
    // Build a set of relative file paths for fast lookup.
    let rel_files: std::collections::HashSet<PathBuf> = abs_files
        .iter()
        .map(|f| f.strip_prefix(root).unwrap_or(f).to_path_buf())
        .collect();

    for dep in &index.deps {
        let dep_rel = dep.loc.file.strip_prefix(root).unwrap_or(&dep.loc.file);
        if !rel_files.contains(dep_rel) {
            continue;
        }
        // Try module-path resolution first (works for import deps like `crate::ir::types`)
        let target_dir: PathBuf = if let Some(d) = resolve_import_dir(&dep.to_name, module_dir_map) {
            // d is already a relative directory — just apply depth
            limit_dir_depth(&d, depth)
        } else {
            // Fallback: by_name lookup for trait_impl / call deps
            let mut found = None;
            for target in index.by_name(&dep.to_name) {
                let rel = target.loc.file.strip_prefix(root).unwrap_or(&target.loc.file);
                found = Some(dir_at_depth(rel, depth));
                break;
            }
            match found {
                Some(d) => d,
                None => continue,
            }
        };

        if target_dir != this_dir {
            let s = target_dir.display().to_string();
            if !s.is_empty() {
                dirs.insert(s);
            }
        }
    }
    dirs.into_iter().collect()
}

// --- Text formatting ---

fn format_text(
    file_syms: &[(PathBuf, Vec<&Symbol>)],
    index: &Index,
    root: &Path,
    depth: Option<usize>,
    symbols_mode: bool,
    total_files: usize,
    total_symbols: usize,
    excluded_generated: usize,
    module_dir_map: &HashMap<String, PathBuf>,
) -> String {
    let mut lines: Vec<String> = Vec::new();

    // Header
    let mut header = format!("{} files · {} symbols", total_files, total_symbols);
    if excluded_generated > 0 {
        header.push_str(&format!(
            "  ({} generated excluded — use --include-generated to show)",
            excluded_generated
        ));
    }
    lines.push(header);

    if file_syms.is_empty() {
        return lines.join("\n");
    }

    lines.push(String::new());

    if symbols_mode {
        format_symbols(file_syms, root, depth, &mut lines);
    } else {
        format_summary(file_syms, index, root, depth, &mut lines, module_dir_map);
    }

    lines.join("\n")
}

/// Default: one line per directory with symbol counts and outgoing dep arrows.
fn format_summary(
    file_syms: &[(PathBuf, Vec<&Symbol>)],
    index: &Index,
    root: &Path,
    depth: Option<usize>,
    lines: &mut Vec<String>,
    module_dir_map: &HashMap<String, PathBuf>,
) {
    let groups = group_by_dir(file_syms, root, depth);

    // Pre-compute all values for column alignment
    struct Row {
        dir_str: String,
        file_str: String,
        counts_str: String,
        dep_str: String,
    }

    let rows: Vec<Row> = groups
        .iter()
        .map(|(dir, indices)| {
            let dir_str = if dir.as_os_str().is_empty() {
                "./".to_string()
            } else {
                format!("{}/", dir.display())
            };

            let n = indices.len();
            let file_str = if n == 1 {
                "1 file".to_string()
            } else {
                format!("{} files", n)
            };

            let counts = count_by_kind(file_syms, indices);
            let counts_str = format_counts(&counts);

            let abs_files: Vec<&PathBuf> = indices.iter().map(|&i| &file_syms[i].0).collect();
            let deps = outgoing_dirs(&abs_files, index, root, dir, depth, module_dir_map);
            let dep_str = if deps.is_empty() {
                String::new()
            } else {
                format!("→ {}", deps.join(", "))
            };

            Row { dir_str, file_str, counts_str, dep_str }
        })
        .collect();

    let max_dir = rows.iter().map(|r| r.dir_str.len()).max().unwrap_or(0);
    let max_files = rows.iter().map(|r| r.file_str.len()).max().unwrap_or(0);
    let max_counts = rows.iter().map(|r| r.counts_str.len()).max().unwrap_or(0);

    for row in &rows {
        if row.dep_str.is_empty() {
            lines.push(format!(
                "{:<dw$}  {:<fw$}  {}",
                row.dir_str,
                row.file_str,
                row.counts_str,
                dw = max_dir,
                fw = max_files,
            ));
        } else {
            lines.push(format!(
                "{:<dw$}  {:<fw$}  {:<cw$}  {}",
                row.dir_str,
                row.file_str,
                row.counts_str,
                row.dep_str,
                dw = max_dir,
                fw = max_files,
                cw = max_counts,
            ));
        }
    }
}

/// `--symbols` mode: files grouped by directory with inline symbol lists.
fn format_symbols(
    file_syms: &[(PathBuf, Vec<&Symbol>)],
    root: &Path,
    depth: Option<usize>,
    lines: &mut Vec<String>,
) {
    let groups = group_by_dir(file_syms, root, depth);

    let mut first_group = true;
    for (dir, indices) in &groups {
        if !first_group {
            lines.push(String::new());
        }
        first_group = false;

        let dir_str = if dir.as_os_str().is_empty() {
            "./".to_string()
        } else {
            format!("{}/", dir.display())
        };
        lines.push(dir_str);

        // Display name = path relative to the group dir (filename at full depth, subpath when collapsed)
        let display_names: Vec<String> = indices
            .iter()
            .map(|&i| {
                let rel = relative_path(&file_syms[i].0, root);
                rel.strip_prefix(dir).unwrap_or(rel).display().to_string()
            })
            .collect();

        let max_fname = display_names.iter().map(|n| n.len()).max().unwrap_or(0);

        for (&i, dname) in indices.iter().zip(display_names.iter()) {
            let syms = &file_syms[i].1;
            let sym_list: Vec<String> = syms.iter().map(|s| inline_name(s)).collect();
            if sym_list.is_empty() {
                lines.push(format!("  {}", dname));
            } else {
                lines.push(format!(
                    "  {:<width$}  {}",
                    dname,
                    sym_list.join(", "),
                    width = max_fname
                ));
            }
        }
    }
}

// --- JSON formatting ---

#[derive(Serialize)]
struct JsonDir {
    dir: String,
    files: usize,
    symbols: Vec<JsonKindCount>,
    outgoing: Vec<String>,
    file_list: Vec<JsonFile>,
}

#[derive(Serialize)]
struct JsonKindCount {
    kind: &'static str,
    count: usize,
}

#[derive(Serialize)]
struct JsonFile {
    file: String,
    symbols: Vec<JsonSymbol>,
}

#[derive(Serialize)]
struct JsonSymbol {
    name: String,
    kind: String,
}

fn format_json(
    file_syms: &[(PathBuf, Vec<&Symbol>)],
    index: &Index,
    root: &Path,
    depth: Option<usize>,
    module_dir_map: &HashMap<String, PathBuf>,
) -> String {
    let groups = group_by_dir(file_syms, root, depth);

    let dirs: Vec<JsonDir> = groups
        .iter()
        .map(|(dir, indices)| {
            let dir_str = dir.display().to_string();
            let counts = count_by_kind(file_syms, indices);
            let abs_files: Vec<&PathBuf> = indices.iter().map(|&i| &file_syms[i].0).collect();
            let outgoing = outgoing_dirs(&abs_files, index, root, dir, depth, module_dir_map);

            let file_list = indices
                .iter()
                .map(|&i| {
                    let (file, syms) = &file_syms[i];
                    let rel = relative_path(file, root);
                    JsonFile {
                        file: rel.display().to_string(),
                        symbols: syms
                            .iter()
                            .map(|s| JsonSymbol {
                                name: s.name.clone(),
                                kind: format!("{}", s.kind),
                            })
                            .collect(),
                    }
                })
                .collect();

            JsonDir {
                dir: dir_str,
                files: indices.len(),
                symbols: counts
                    .iter()
                    .map(|(label, count)| JsonKindCount { kind: label, count: *count })
                    .collect(),
                outgoing,
                file_list,
            }
        })
        .collect();

    serde_json::to_string_pretty(&dirs).unwrap_or_else(|_| "[]".to_string())
}
