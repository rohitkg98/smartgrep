//! Platform-specific CLI behavior, run against the real binary.

use std::path::Path;
use std::process::{Command, Output};

fn smartgrep(args: &[&str]) -> Output {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/regression/rust_project");
    Command::new(env!("CARGO_BIN_EXE_smartgrep"))
        .arg("--project-root")
        .arg(&root)
        .args(args)
        .output()
        .expect("run smartgrep")
}

/// Without Unix sockets, `--daemon` prints one notice and runs the command directly.
#[cfg(not(unix))]
#[test]
fn daemon_flag_falls_back_to_direct_execution() {
    let direct = smartgrep(&["ls", "structs"]);
    let daemon = smartgrep(&["--daemon", "ls", "structs"]);
    assert!(daemon.status.success());
    assert_eq!(daemon.stdout, direct.stdout);
    let stderr = String::from_utf8_lossy(&daemon.stderr);
    assert_eq!(
        stderr.matches("daemon not supported on this platform; running directly").count(),
        1,
        "{stderr}"
    );
}

/// Windows users can type native separators in path filters.
#[cfg(windows)]
#[test]
fn backslash_path_filters_match() {
    let slash = smartgrep(&["ls", "fns", "--in", "src/"]);
    let backslash = smartgrep(&["ls", "fns", "--in", "src\\"]);
    assert!(slash.status.success() && !slash.stdout.is_empty());
    assert_eq!(slash.stdout, backslash.stdout);

    let q = |p: &str| smartgrep(&["query", &format!("fns where file contains '{p}'")]).stdout;
    assert_eq!(q("src/models"), q("src\\models"));
}

/// Stored paths never contain a native Windows separator.
#[test]
fn output_paths_use_forward_slashes() {
    let out = smartgrep(&["--format", "json", "ls", "structs"]);
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("src/models.rs"), "{s}");
    assert!(!s.contains("\\\\"), "{s}");
}
