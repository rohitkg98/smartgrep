use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use smartgrep::commands::init::{self, InitOptions, END_MARKER, START_MARKER};
use tempfile::TempDir;

const RUST_SRC: &str = r#"
pub struct Widget {
    pub size: u32,
}

pub fn make_widget() -> Widget {
    helper();
    Widget { size: 1 }
}

fn helper() {}

impl Widget {
    pub fn new() -> Self {
        Widget { size: 0 }
    }
}

pub fn use_widget(w: &Widget) -> u32 {
    let _ = make_widget();
    let _ = Widget::new();
    w.size
}
"#;

const PY_SRC: &str = r#"
class Gadget:
    def spin(self):
        return 1


def build_gadget():
    return Gadget()
"#;

fn write(root: &Path, rel: &str, content: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

/// A small Rust project that is a git repo (has `.git/`).
fn rust_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "Cargo.toml", "[package]\nname = \"demo\"\n");
    write(dir.path(), "src/lib.rs", RUST_SRC);
    fs::create_dir_all(dir.path().join(".git")).unwrap();
    dir
}

/// Every file under root (excluding `.git/`) with its content.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    for e in walkdir::WalkDir::new(root).into_iter().flatten() {
        let rel = e.path().strip_prefix(root).unwrap().to_path_buf();
        if rel.starts_with(".git") || !e.file_type().is_file() {
            continue;
        }
        out.insert(rel, fs::read(e.path()).unwrap());
    }
    out
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap()
}

fn opts() -> InitOptions {
    InitOptions::default()
}

fn block_of(text: &str) -> &str {
    let s = text.find(START_MARKER).unwrap();
    let e = text.find(END_MARKER).unwrap() + END_MARKER.len();
    &text[s..e]
}

#[test]
fn fresh_project_creates_claude_md_skill_and_gitignore() {
    let dir = rust_project();
    let root = dir.path();
    let out = init::init(root, &opts()).unwrap();

    assert!(out.contains("Detected: rust (1 file)"), "{out}");
    assert!(out.contains("Indexed "), "{out}");
    assert!(out.contains("Created CLAUDE.md (smartgrep block)"), "{out}");
    assert!(out.contains("Installed skill: .claude/skills/smartgrep/SKILL.md"), "{out}");
    assert!(out.contains("Added .smartgrep/ to .gitignore"), "{out}");

    let claude = read(root, "CLAUDE.md");
    assert!(claude.starts_with(START_MARKER));
    assert!(claude.ends_with(&format!("{END_MARKER}\n")));
    assert!(!root.join("AGENTS.md").exists());
    assert_eq!(
        read(root, ".claude/skills/smartgrep/SKILL.md"),
        smartgrep::commands::install_skill::SKILL_CONTENT
    );
    assert_eq!(read(root, ".gitignore"), ".smartgrep/\n");
    assert!(root.join(".smartgrep/index.json").exists());
}

#[test]
fn block_uses_real_symbols_and_is_short() {
    let dir = rust_project();
    let root = dir.path();
    init::init(root, &opts()).unwrap();
    let block = block_of(&read(root, "CLAUDE.md")).to_string();

    assert!(block.contains("smartgrep show Widget"), "{block}");
    assert!(block.contains("smartgrep refs Widget"), "{block}");
    assert!(block.contains("smartgrep deps make_widget"), "{block}");
    assert!(block.contains("smartgrep context src/lib.rs"), "{block}");
    assert!(block.contains("smartgrep ls structs --in src/"), "{block}");
    assert!(block.contains(".claude/skills/smartgrep/SKILL.md"), "{block}");
    assert!(!block.contains("Large project"), "{block}");
    assert!(block.lines().count() <= 30, "block too long:\n{block}");
}

#[test]
fn refs_example_falls_back_to_function_when_type_has_no_refs() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(root, "src/lib.rs", "pub struct Lonely;\n\npub fn callee() {}\n\npub fn caller() {\n    callee();\n}\n");
    let out = init::init(root, &InitOptions { dry_run: true, ..opts() }).unwrap();
    assert!(out.contains("smartgrep show Lonely"), "{out}");
    assert!(out.contains("smartgrep refs callee"), "{out}");
    assert!(out.contains("smartgrep deps caller"), "{out}");
}

#[test]
fn existing_claude_md_content_preserved_and_block_appended() {
    let dir = rust_project();
    let root = dir.path();
    let user = "# My project\n\nSome rules.\n";
    write(root, "CLAUDE.md", user);
    let out = init::init(root, &opts()).unwrap();
    assert!(out.contains("Updated CLAUDE.md (smartgrep block)"), "{out}");

    let claude = read(root, "CLAUDE.md");
    assert!(claude.starts_with("# My project\n\nSome rules.\n\n<!-- smartgrep:start -->"), "{claude}");
    assert!(claude.ends_with(&format!("{END_MARKER}\n")));
}

#[test]
fn rerun_is_idempotent_and_replaces_block() {
    let dir = rust_project();
    let root = dir.path();
    write(root, "CLAUDE.md", "# Top\n");
    init::init(root, &opts()).unwrap();
    let first = snapshot(root);
    let out = init::init(root, &opts()).unwrap();
    assert!(out.contains("CLAUDE.md already up to date"), "{out}");
    assert!(!out.contains("gitignore"), "{out}");
    let second = snapshot(root);
    let strip_index = |m: BTreeMap<PathBuf, Vec<u8>>| {
        m.into_iter().filter(|(p, _)| !p.starts_with(".smartgrep")).collect::<BTreeMap<_, _>>()
    };
    assert_eq!(strip_index(first), strip_index(second));

    // A stale block between markers gets replaced; text around it is untouched.
    write(
        root,
        "CLAUDE.md",
        &format!("# Top\n\n{START_MARKER}\nold stuff\n{END_MARKER}\n\n## After\nkeep me\n"),
    );
    init::init(root, &opts()).unwrap();
    let claude = read(root, "CLAUDE.md");
    assert!(!claude.contains("old stuff"));
    assert!(claude.starts_with(&format!("# Top\n\n{START_MARKER}\n")));
    assert!(claude.ends_with(&format!("{END_MARKER}\n\n## After\nkeep me\n")), "{claude}");
    assert_eq!(claude.matches(START_MARKER).count(), 1);
}

#[test]
fn both_claude_and_agents_md_updated_when_both_exist() {
    let dir = rust_project();
    let root = dir.path();
    write(root, "CLAUDE.md", "claude\n");
    write(root, "AGENTS.md", "agents\n");
    init::init(root, &opts()).unwrap();
    let c = read(root, "CLAUDE.md");
    let a = read(root, "AGENTS.md");
    assert!(c.starts_with("claude\n\n") && c.contains(START_MARKER));
    assert!(a.starts_with("agents\n\n") && a.contains(START_MARKER));
    assert_eq!(block_of(&c), block_of(&a));
}

#[test]
fn only_agents_md_existing_is_updated_without_creating_claude_md() {
    let dir = rust_project();
    let root = dir.path();
    write(root, "AGENTS.md", "agents\n");
    init::init(root, &opts()).unwrap();
    assert!(read(root, "AGENTS.md").contains(START_MARKER));
    assert!(!root.join("CLAUDE.md").exists());
}

#[test]
fn agents_md_flag_creates_agents_md() {
    let dir = rust_project();
    let root = dir.path();
    let out = init::init(root, &InitOptions { agents_md: true, ..opts() }).unwrap();
    assert!(out.contains("Created CLAUDE.md"), "{out}");
    assert!(out.contains("Created AGENTS.md"), "{out}");
    assert_eq!(read(root, "CLAUDE.md"), read(root, "AGENTS.md"));
}

#[test]
fn no_skill_skips_skill_install() {
    let dir = rust_project();
    let root = dir.path();
    let out = init::init(root, &InitOptions { no_skill: true, ..opts() }).unwrap();
    assert!(!out.contains("skill"), "{out}");
    assert!(!root.join(".claude").exists());
    let claude = read(root, "CLAUDE.md");
    assert!(claude.contains("smartgrep install-skill"), "{claude}");
    assert!(!claude.contains(".claude/skills/smartgrep/SKILL.md"), "{claude}");
}

#[test]
fn dry_run_writes_nothing() {
    let dir = rust_project();
    let root = dir.path();
    write(root, "CLAUDE.md", "keep\n");
    let before = snapshot(root);
    let out = init::init(root, &InitOptions { dry_run: true, agents_md: true, ..opts() }).unwrap();
    assert_eq!(before, snapshot(root));
    assert!(!root.join(".smartgrep").exists());
    assert!(out.contains("Would update CLAUDE.md"), "{out}");
    assert!(out.contains("Would create AGENTS.md"), "{out}");
    assert!(out.contains("Would install skill"), "{out}");
    assert!(out.contains("Would add .smartgrep/ to .gitignore"), "{out}");
    assert!(out.contains(START_MARKER) && out.contains("smartgrep show Widget"), "{out}");
}

#[test]
fn no_sources_errors_without_writing() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(root, "README.md", "hi\n");
    fs::create_dir_all(root.join(".git")).unwrap();
    let before = snapshot(root);
    let err = init::init(root, &opts()).unwrap_err().to_string();
    assert!(err.contains("No supported source files"), "{err}");
    assert!(err.contains(".rs") && err.contains(".py"), "{err}");
    assert_eq!(before, snapshot(root));
}

#[test]
fn cli_no_sources_exits_nonzero() {
    let dir = TempDir::new().unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_smartgrep"))
        .args(["--project-root", dir.path().to_str().unwrap(), "init"])
        .output()
        .unwrap();
    assert!(!status.status.success());
    assert!(String::from_utf8_lossy(&status.stderr).contains("No supported source files"));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn cli_respects_project_root() {
    let dir = rust_project();
    let out = Command::new(env!("CARGO_BIN_EXE_smartgrep"))
        .args(["--project-root", dir.path().to_str().unwrap(), "init"])
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(dir.path().join("CLAUDE.md").exists());
    assert!(dir.path().join(".claude/skills/smartgrep/SKILL.md").exists());
}

#[test]
fn vocabulary_only_for_detected_languages() {
    let dir = rust_project();
    let root = dir.path();
    init::init(root, &opts()).unwrap();
    let block = block_of(&read(root, "CLAUDE.md")).to_string();
    assert!(block.contains("- Rust: fn, method, struct"), "{block}");
    for other in ["- Python:", "- Go:", "- Java:", "- TypeScript:", "`functions` spans"] {
        assert!(!block.contains(other), "{other} in {block}");
    }

    write(root, "tools/gadget.py", PY_SRC);
    init::init(root, &opts()).unwrap();
    let block = block_of(&read(root, "CLAUDE.md")).to_string();
    assert!(block.contains("- Rust:") && block.contains("- Python: def, class"), "{block}");
    assert!(block.contains("`functions` spans"), "{block}");
    assert!(!block.contains("- Go:"), "{block}");
}

#[test]
fn large_project_gets_scoping_warning() {
    let dir = rust_project();
    let root = dir.path();
    for i in 0..100 {
        write(root, &format!("src/m{i}.rs"), &format!("pub fn f{i}() {{}}\n"));
    }
    let out = init::init(root, &InitOptions { dry_run: true, ..opts() }).unwrap();
    assert!(out.contains("Large project (101 files)"), "{out}");
    assert!(out.contains("smartgrep map --depth 1"), "{out}");
}

#[test]
fn malformed_markers_error_without_writing() {
    let dir = rust_project();
    let root = dir.path();
    write(root, "CLAUDE.md", &format!("# x\n{START_MARKER}\nno end here\n"));
    let before = snapshot(root);
    let err = init::init(root, &opts()).unwrap_err().to_string();
    assert!(err.contains("CLAUDE.md") && err.contains("malformed"), "{err}");
    assert_eq!(before, snapshot(root));
    assert!(!root.join(".smartgrep").exists());
}

#[test]
fn apply_block_marker_cases() {
    let b = format!("{START_MARKER}\nnew\n{END_MARKER}");
    assert_eq!(init::apply_block(None, &b).unwrap(), format!("{b}\n"));
    assert_eq!(init::apply_block(Some(""), &b).unwrap(), format!("{b}\n"));
    assert_eq!(init::apply_block(Some("a\n\n\n"), &b).unwrap(), format!("a\n\n{b}\n"));
    assert_eq!(init::apply_block(Some("a"), &b).unwrap(), format!("a\n\n{b}\n"));
    // end before start, end only, duplicate blocks
    for bad in [
        format!("{END_MARKER}\n{START_MARKER}\n"),
        format!("x\n{END_MARKER}\n"),
        format!("{START_MARKER}\n{END_MARKER}\n{START_MARKER}\n{END_MARKER}\n"),
    ] {
        assert!(init::apply_block(Some(&bad), &b).is_err(), "{bad}");
    }
}

#[test]
fn gitignore_without_trailing_newline() {
    let dir = rust_project();
    let root = dir.path();
    write(root, ".gitignore", ".vscode");
    init::init(root, &opts()).unwrap();
    assert_eq!(read(root, ".gitignore"), ".vscode\n.smartgrep/\n");
}

#[test]
fn gitignore_existing_entry_left_alone_and_non_git_untouched() {
    assert_eq!(init::gitignore_with_smartgrep(Some("target\n/.smartgrep\n")), None);
    assert_eq!(init::gitignore_with_smartgrep(Some(".smartgrep")), None);
    assert_eq!(init::gitignore_with_smartgrep(Some(".smartgrep-old\n")).unwrap(), ".smartgrep-old\n.smartgrep/\n");

    let dir = TempDir::new().unwrap();
    write(dir.path(), "main.go", "package main\n\nfunc main() {}\n");
    let out = init::init(dir.path(), &opts()).unwrap();
    assert!(!out.contains("gitignore"), "{out}");
    assert!(!dir.path().join(".gitignore").exists());
}
