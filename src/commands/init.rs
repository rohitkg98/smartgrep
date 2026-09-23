//! `smartgrep init`: make a project smartgrep-ready for coding agents.
//!
//! Detects languages, builds the index, writes a short smartgrep block into the
//! agent instruction files (CLAUDE.md / AGENTS.md), installs the repo-scoped
//! skill and ignores `.smartgrep/` in git. Non-interactive and idempotent.
//!
//! Everything is computed up front by [`plan`] (nothing written); [`apply`]
//! then performs the writes. A malformed instruction file therefore aborts
//! before anything touches disk.

use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};

use crate::index::types::Index;
use crate::index::{auto, builder, store};
use crate::ir::kinds;
use crate::ir::types::{Symbol, Visibility};
use crate::lang::{self, Language};

use super::install_skill;

pub const START_MARKER: &str = "<!-- smartgrep:start -->";
pub const END_MARKER: &str = "<!-- smartgrep:end -->";

/// Projects with at least this many source files get the scoping warning.
pub const LARGE_PROJECT_FILES: usize = 100;

const INSTRUCTION_FILES: [&str; 2] = ["CLAUDE.md", "AGENTS.md"];

#[derive(Debug, Clone, Copy, Default)]
pub struct InitOptions {
    /// Also create/update AGENTS.md.
    pub agents_md: bool,
    /// Don't install the repo-scoped skill.
    pub no_skill: bool,
    /// Print what would change; write nothing.
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Create,
    Update,
    Unchanged,
}

/// A planned write of a whole file, relative to the project root.
#[derive(Debug, Clone)]
pub struct FileChange {
    pub rel_path: PathBuf,
    pub action: Action,
    pub content: String,
}

/// Everything `init` would do, computed without writing.
pub struct Plan {
    pub root: PathBuf,
    /// (language name, source file count), in registry order.
    pub languages: Vec<(&'static str, usize)>,
    pub symbol_count: usize,
    pub dep_count: usize,
    pub block: String,
    pub instruction_files: Vec<FileChange>,
    pub skill: Option<FileChange>,
    pub gitignore: Option<FileChange>,
}

/// Count source files per registered language under `root`.
/// Returns (language, count) pairs for languages present, in registry order.
pub fn detect_languages(root: &Path) -> Vec<(&'static Language, usize)> {
    let sources = auto::collect_sources(root);
    lang::LANGUAGES
        .iter()
        .map(|l| {
            let n = sources
                .iter()
                .filter(|p| lang::language_for_path(p).map(|x| x.name) == Some(l.name))
                .count();
            (l, n)
        })
        .filter(|(_, n)| *n > 0)
        .collect()
}

// ---------------------------------------------------------------------------
// Block rendering
// ---------------------------------------------------------------------------

/// Real symbol names used to make the block's example commands work.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Examples {
    /// A public type (struct/class/interface/...) and its kind.
    pub type_name: Option<(String, String)>,
    /// A public function (or method) and its kind.
    pub function: Option<(String, String)>,
    /// A source file worth `context`-ing (the type's file).
    pub file: Option<String>,
    /// Directory (with trailing '/') of the type, for `--in` scoping.
    pub type_dir: Option<String>,
    /// Directory (with trailing '/') of the function, for `file contains`.
    pub function_dir: Option<String>,
    /// Name for the `refs` example (the type, or the function if only it has refs).
    pub refs_target: Option<String>,
    /// Name for the `deps` example (the function, or the type if only it has deps).
    pub deps_target: Option<String>,
}

/// Inputs to [`render_block`].
#[derive(Clone)]
pub struct BlockContext {
    /// Detected languages, registry order.
    pub languages: Vec<&'static Language>,
    pub total_files: usize,
    pub examples: Examples,
    /// Repo-relative path of the installed skill, if there is one.
    pub skill_path: Option<String>,
}

fn is_test_or_example_path(path: &Path) -> bool {
    // Test dirs can be nested (src/test/java, pkg/tests); docs/examples/benches
    // only count at the top level, since `com/example/...` is a common package path.
    const TEST_DIRS: &[&str] = &[
        "test", "tests", "testing", "__tests__", "spec", "specs", "fixtures", "testdata",
    ];
    const TOP_DIRS: &[&str] = &["example", "examples", "docs", "bench", "benches", "benchmarks"];
    let dirs: Vec<&str> = path
        .parent()
        .map(|p| p.components().filter_map(|c| c.as_os_str().to_str()).collect())
        .unwrap_or_default();
    let in_dir = dirs.iter().any(|d| TEST_DIRS.contains(d))
        || dirs.first().map_or(false, |d| TOP_DIRS.contains(d));
    let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let stem = fname.split('.').next().unwrap_or("");
    in_dir
        || stem.starts_with("test_")
        || stem.ends_with("_test")
        || stem.ends_with("Test")
        || stem.ends_with("Tests")
        || fname.contains(".test.")
        || fname.contains(".spec.")
        || stem == "conftest"
}

fn dir_of(path: &Path) -> Option<String> {
    let parent = path.parent()?.to_string_lossy().replace('\\', "/");
    if parent.is_empty() {
        None
    } else {
        Some(format!("{}/", parent))
    }
}

/// Pick the best example among `candidates`: prefer names that resolve to a
/// single symbol (so `show`/`deps` give one answer), then those with outgoing
/// deps, then the most referenced. Ties break by name for determinism.
fn best<'a>(index: &Index, candidates: Vec<&'a Symbol>) -> Option<&'a Symbol> {
    candidates.into_iter().min_by_key(|s| {
        let unique = index.by_name(&s.name).len() == 1;
        let has_deps = !index.deps_of(&s.qualified_name).is_empty();
        let refs = index.refs_to(&s.name).len();
        (!unique, refs == 0, !has_deps, Reverse(refs), s.name.clone(), s.loc.file.clone())
    })
}

/// Choose real symbols from the index for the block's examples.
pub fn pick_examples(index: &Index) -> Examples {
    let eligible: Vec<&Symbol> = index
        .symbols
        .iter()
        .filter(|s| matches!(s.visibility, Visibility::Public))
        .filter(|s| s.name.len() >= 3 && !s.name.starts_with('_'))
        .filter(|s| s.name.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .filter(|s| !is_test_or_example_path(&s.loc.file) && !super::map::is_generated(&s.loc.file))
        .collect();

    let pick = |pred: &dyn Fn(&Symbol) -> bool| -> Option<&Symbol> {
        best(index, eligible.iter().copied().filter(|s| pred(s)).collect())
    };

    let ty = pick(&|s| matches!(s.kind.as_str(), "struct" | "class" | "interface" | "trait" | "record"))
        .or_else(|| pick(&|s| kinds::is_type_kind(&s.kind)));
    let func = pick(&|s| kinds::is_function_kind(&s.kind))
        .or_else(|| pick(&|s| s.kind == "method"));

    let has_refs = |s: &&Symbol| !index.refs_to(&s.name).is_empty();
    let has_deps = |s: &&Symbol| !index.deps_of(&s.qualified_name).is_empty();
    let refs_target = ty.filter(has_refs).or(func.filter(has_refs)).or(ty).or(func);
    let deps_target = func
        .filter(has_deps)
        .or_else(|| pick(&|s| kinds::is_callable_kind(&s.kind) && has_deps(&s)))
        .or(ty.filter(has_deps))
        .or(func)
        .or(ty);

    Examples {
        refs_target: refs_target.map(|s| s.name.clone()),
        deps_target: deps_target.map(|s| s.name.clone()),
        type_name: ty.map(|s| (s.name.clone(), s.kind.clone())),
        function: func.map(|s| (s.name.clone(), s.kind.clone())),
        file: ty.or(func).map(|s| s.loc.file.to_string_lossy().replace('\\', "/")),
        type_dir: ty.and_then(|s| dir_of(&s.loc.file)),
        function_dir: func.and_then(|s| dir_of(&s.loc.file)),
    }
}

/// Plural `ls`/`query` term for a native kind (`class` → `classes`).
/// TypeScript's `function` has no plural: `functions` is the cross-language umbrella.
fn kind_term(kind: &str) -> String {
    match kind {
        "function" => "function".to_string(),
        "class" => "classes".to_string(),
        k => format!("{}s", k),
    }
}

/// Render the instruction block (markers included, no trailing newline).
pub fn render_block(ctx: &BlockContext) -> String {
    let ex = &ctx.examples;
    let large = ctx.total_files >= LARGE_PROJECT_FILES;
    let multi_lang = ctx.languages.len() > 1;

    let ty = ex.type_name.as_ref().map(|(n, _)| n.as_str()).unwrap_or("<Type>");
    let ty_term = ex.type_name.as_ref().map(|(_, k)| kind_term(k));
    let func = ex.function.as_ref().map(|(n, _)| n.as_str()).unwrap_or("<function>");
    let fn_term = match &ex.function {
        _ if multi_lang => "functions".to_string(),
        Some((_, k)) => kind_term(k),
        None => "functions".to_string(),
    };
    let file = ex.file.as_deref().unwrap_or("<file>");

    let mut cmds: Vec<(String, String)> = Vec::new();
    if large {
        cmds.push(("smartgrep map --depth 1".into(), "project layout, top-level dirs".into()));
    } else {
        cmds.push(("smartgrep map".into(), "project layout: dirs, files, public symbols".into()));
    }
    cmds.push((format!("smartgrep context {}", file), "symbols in one file".into()));
    let ls_term = ty_term.unwrap_or_else(|| "classes".to_string());
    let ls = match &ex.type_dir {
        Some(d) => format!("smartgrep ls {} --in {}", ls_term, d),
        None if ex.type_name.is_some() => format!("smartgrep ls {}", ls_term),
        None => format!("smartgrep ls {} --in <dir>/", ls_term),
    };
    cmds.push((ls, "list symbols of a kind under a path".into()));
    cmds.push((format!("smartgrep show {}", ty), "signature, fields, location".into()));
    let refs = ex.refs_target.as_deref().unwrap_or(ty);
    let deps = ex.deps_target.as_deref().unwrap_or(func);
    cmds.push((format!("smartgrep refs {}", refs), "who references / calls it".into()));
    cmds.push((format!("smartgrep deps {}", deps), "what it calls and uses".into()));

    let width = cmds.iter().map(|(c, _)| c.len()).max().unwrap_or(0);
    let mut lines: Vec<String> = vec![
        START_MARKER.to_string(),
        "## Code navigation: smartgrep".to_string(),
        String::new(),
        "This project is indexed by `smartgrep` (tree-sitter symbol index; it re-indexes automatically). \
         For structural questions (where is X defined, what's in this file, who calls X, what implements Y) \
         prefer it over grep and reading whole files: one call, far fewer tokens. \
         Read files when you need a full implementation body."
            .to_string(),
        String::new(),
        "```bash".to_string(),
    ];
    for (c, comment) in &cmds {
        lines.push(format!("{:<width$}  # {}", c, comment, width = width));
    }
    let scope_dir = ex.function_dir.as_ref().or(ex.type_dir.as_ref());
    let query = match scope_dir {
        Some(d) => format!(
            "smartgrep query \"{} where file contains '{}' | show name, file, signature\"",
            fn_term, d
        ),
        None => format!("smartgrep query \"symbol {} | with deps, refs\"", ty),
    };
    lines.push(query);
    lines.push("```".to_string());
    lines.push(String::new());

    lines.push("Kinds use each language's own vocabulary (singular or plural):".to_string());
    for l in &ctx.languages {
        lines.push(format!("- {}: {}", l.display_name, l.kinds.join(", ")));
    }
    if multi_lang {
        lines.push("- `functions` spans free functions in every language".to_string());
    }

    if large {
        lines.push(String::new());
        lines.push(format!(
            "Large project ({} files): always scope with `--in <path>` or `where file contains '<path>'`; \
             don't run bare `ls` or `map`.",
            ctx.total_files
        ));
    }

    lines.push(String::new());
    match &ctx.skill_path {
        Some(p) => lines.push(format!(
            "Full reference (query DSL, more patterns): the smartgrep skill, `{}`.",
            p
        )),
        None => lines.push(
            "Full reference (query DSL, more patterns): the smartgrep skill (`smartgrep install-skill`)."
                .to_string(),
        ),
    }
    lines.push(END_MARKER.to_string());
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Pure file transforms
// ---------------------------------------------------------------------------

/// Insert or replace the smartgrep block in an instruction file's content.
/// `existing = None` means the file doesn't exist. Content outside the
/// markers is never modified (apart from trailing newlines when appending).
pub fn apply_block(existing: Option<&str>, block: &str) -> Result<String> {
    let Some(text) = existing else {
        return Ok(format!("{}\n", block));
    };
    let starts: Vec<usize> = text.match_indices(START_MARKER).map(|(i, _)| i).collect();
    let ends: Vec<usize> = text.match_indices(END_MARKER).map(|(i, _)| i).collect();
    match (starts.as_slice(), ends.as_slice()) {
        ([], []) => {
            let body = text.trim_end_matches(['\n', '\r']);
            if body.trim().is_empty() {
                Ok(format!("{}\n", block))
            } else {
                Ok(format!("{}\n\n{}\n", body, block))
            }
        }
        ([s], [e]) if s < e => {
            let end = e + END_MARKER.len();
            Ok(format!("{}{}{}", &text[..*s], block, &text[end..]))
        }
        _ => Err(anyhow!(
            "malformed smartgrep markers (expected exactly one `{}` followed by one `{}`; found {} start, {} end)",
            START_MARKER,
            END_MARKER,
            starts.len(),
            ends.len()
        )),
    }
}

/// True if a .gitignore line already ignores the `.smartgrep` directory.
fn ignores_smartgrep(line: &str) -> bool {
    matches!(
        line.trim(),
        ".smartgrep" | ".smartgrep/" | "/.smartgrep" | "/.smartgrep/" | ".smartgrep/*" | "/.smartgrep/*" | ".smartgrep/**"
    )
}

/// New .gitignore content with `.smartgrep/` added, or None if already ignored.
pub fn gitignore_with_smartgrep(existing: Option<&str>) -> Option<String> {
    let text = existing.unwrap_or("");
    if text.lines().any(ignores_smartgrep) {
        return None;
    }
    let mut out = text.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(".smartgrep/\n");
    Some(out)
}

// ---------------------------------------------------------------------------
// Plan / apply
// ---------------------------------------------------------------------------

fn read_optional(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow!("cannot read {}: {}", path.display(), e)),
    }
}

fn change(rel_path: PathBuf, existing: Option<&str>, content: String) -> FileChange {
    let action = match existing {
        None => Action::Create,
        Some(old) if old == content => Action::Unchanged,
        Some(_) => Action::Update,
    };
    FileChange { rel_path, action, content }
}

fn rel_display(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Compute everything `init` would do (parses sources into an in-memory
/// index, but writes nothing). Returns the plan and the built index.
pub fn plan(root: &Path, opts: &InitOptions) -> Result<(Plan, Index)> {
    let detected = detect_languages(root);
    if detected.is_empty() {
        bail!(
            "No supported source files found under {}.\nsmartgrep supports: {}",
            root.display(),
            lang::supported_extensions_display()
        );
    }
    let total_files: usize = detected.iter().map(|(_, n)| n).sum();

    let ir = auto::parse_all_sources(root)?;
    let index = builder::build(&ir);

    let skill_file = install_skill::skill_path(root);
    let skill_rel = install_skill::skill_path(Path::new(""));
    let skill = if opts.no_skill {
        None
    } else {
        let existing = read_optional(&skill_file)?;
        Some(change(skill_rel.clone(), existing.as_deref(), install_skill::SKILL_CONTENT.to_string()))
    };
    let skill_available = skill.is_some() || skill_file.exists();

    let ctx = BlockContext {
        languages: detected.iter().map(|(l, _)| *l).collect(),
        total_files,
        examples: pick_examples(&index),
        skill_path: skill_available.then(|| rel_display(&skill_rel)),
    };
    let block = render_block(&ctx);

    // Targets: existing CLAUDE.md / AGENTS.md; CLAUDE.md if neither; AGENTS.md with --agents-md.
    let mut targets: Vec<&str> = INSTRUCTION_FILES
        .iter()
        .copied()
        .filter(|f| root.join(f).exists())
        .collect();
    if targets.is_empty() {
        targets.push("CLAUDE.md");
    }
    if opts.agents_md && !targets.contains(&"AGENTS.md") {
        targets.push("AGENTS.md");
    }
    let mut instruction_files = Vec::new();
    for f in targets {
        let existing = read_optional(&root.join(f))?;
        let content = apply_block(existing.as_deref(), &block).map_err(|e| anyhow!("{}: {}", f, e))?;
        instruction_files.push(change(PathBuf::from(f), existing.as_deref(), content));
    }

    let gitignore = if root.join(".git").exists() {
        let existing = read_optional(&root.join(".gitignore"))?;
        gitignore_with_smartgrep(existing.as_deref())
            .map(|content| change(PathBuf::from(".gitignore"), existing.as_deref(), content))
    } else {
        None
    };

    let plan = Plan {
        root: root.to_path_buf(),
        languages: detected.iter().map(|(l, n)| (l.name, *n)).collect(),
        symbol_count: index.symbols.len(),
        dep_count: index.deps.len(),
        block,
        instruction_files,
        skill,
        gitignore,
    };
    Ok((plan, index))
}

fn write_change(root: &Path, c: &FileChange) -> Result<()> {
    if c.action == Action::Unchanged {
        return Ok(());
    }
    let path = root.join(&c.rel_path);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&path, &c.content).map_err(|e| anyhow!("cannot write {}: {}", path.display(), e))
}

/// Perform the plan's writes: index, instruction files, skill, .gitignore.
pub fn apply(plan: &Plan, index: &Index) -> Result<()> {
    store::save(index, &auto::index_path(&plan.root))?;
    for c in &plan.instruction_files {
        write_change(&plan.root, c)?;
    }
    if let Some(c) = &plan.skill {
        write_change(&plan.root, c)?;
    }
    if let Some(c) = &plan.gitignore {
        write_change(&plan.root, c)?;
    }
    Ok(())
}

/// Human-readable summary of a plan (after applying it, or for --dry-run).
pub fn summary(plan: &Plan, dry_run: bool) -> String {
    let mut out = Vec::new();
    let langs: Vec<String> = plan
        .languages
        .iter()
        .map(|(l, n)| format!("{} ({} file{})", l, n, if *n == 1 { "" } else { "s" }))
        .collect();
    out.push(format!("Detected: {}", langs.join(", ")));
    out.push(format!(
        "{} {} symbols, {} deps",
        if dry_run { "Would index" } else { "Indexed" },
        plan.symbol_count,
        plan.dep_count
    ));
    for c in &plan.instruction_files {
        let p = rel_display(&c.rel_path);
        out.push(match (c.action, dry_run) {
            (Action::Create, false) => format!("Created {} (smartgrep block)", p),
            (Action::Create, true) => format!("Would create {} (smartgrep block)", p),
            (Action::Update, false) => format!("Updated {} (smartgrep block)", p),
            (Action::Update, true) => format!("Would update {} (smartgrep block)", p),
            (Action::Unchanged, _) => format!("{} already up to date", p),
        });
    }
    if let Some(c) = &plan.skill {
        let p = rel_display(&c.rel_path);
        out.push(match (c.action, dry_run) {
            (Action::Unchanged, _) => format!("Skill already up to date: {}", p),
            (_, false) => format!("Installed skill: {}", p),
            (_, true) => format!("Would install skill: {}", p),
        });
    }
    if plan.gitignore.is_some() {
        out.push(if dry_run {
            "Would add .smartgrep/ to .gitignore".to_string()
        } else {
            "Added .smartgrep/ to .gitignore".to_string()
        });
    }
    if dry_run {
        out.push(String::new());
        out.push("Block:".to_string());
        out.push(plan.block.clone());
        out.push(String::new());
        out.push("Dry run: nothing written.".to_string());
    }
    out.join("\n")
}

/// Run init on `root` and return the summary text. Used by the CLI and tests.
pub fn init(root: &Path, opts: &InitOptions) -> Result<String> {
    let (plan, index) = plan(root, opts)?;
    if !opts.dry_run {
        apply(&plan, &index)?;
    }
    Ok(summary(&plan, opts.dry_run))
}

/// CLI entry point. Uses `--project-root` if given, else the detected
/// project root, else the current directory.
pub fn run(project_root: &Option<PathBuf>, opts: &InitOptions) -> Result<()> {
    let root = match project_root {
        Some(r) => r.clone(),
        None => {
            let cwd = std::env::current_dir()?;
            auto::detect_project_root(&cwd).unwrap_or(cwd)
        }
    };
    println!("{}", init(&root, opts)?);
    Ok(())
}
