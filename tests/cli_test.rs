use std::process::Command;

#[test]
fn version_flag_prints_crate_version() {
    for flag in ["--version", "-V"] {
        let out = Command::new(env!("CARGO_BIN_EXE_smartgrep"))
            .arg(flag)
            .output()
            .expect("run smartgrep");
        assert!(out.status.success(), "{flag} failed: {out:?}");
        let stdout = String::from_utf8(out.stdout).unwrap();
        assert_eq!(stdout.trim(), format!("smartgrep {}", env!("CARGO_PKG_VERSION")));
    }
}
