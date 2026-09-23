pub mod common;
pub mod go;
pub mod java;
pub mod python;
pub mod rust;
pub mod typescript;

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::ir::types::Ir;

/// Parse a source file by dispatching to the appropriate language parser based on extension.
/// The set of supported languages lives in `crate::lang::LANGUAGES`.
///
/// This is the single point where paths enter the IR: `path` is normalized to
/// `/` separators here, so parsers (qualified names) and `SourceLoc.file` never
/// see a native Windows separator. CRLF line endings are folded to LF so
/// multi-line signatures don't carry `\r` (line/col are unaffected).
pub fn parse_by_extension(path: &Path, source: &str) -> Result<Ir> {
    let path = crate::paths::normalize(path);
    if source.contains('\r') {
        parse_slash_path(&path, &source.replace("\r\n", "\n"))
    } else {
        parse_slash_path(&path, source)
    }
}

fn parse_slash_path(path: &Path, source: &str) -> Result<Ir> {
    match crate::lang::language_for_path(path) {
        Some(lang) => (lang.parse)(path, source),
        None => {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            Err(anyhow!(
                "Unsupported file type '.{}'. smartgrep supports {} files.",
                ext,
                crate::lang::supported_extensions_display()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::to_slash_with;

    /// Parse `unix_rel` and its Windows-separator twin; both must yield the
    /// same qualified names and file locations.
    fn assert_same_ir(unix_rel: &str, source: &str) -> Ir {
        let win_rel = unix_rel.replace('/', "\\");
        let from_win = parse_slash_path(Path::new(&to_slash_with(&win_rel, '\\')), source).unwrap();
        let from_unix = parse_by_extension(Path::new(unix_rel), source).unwrap();
        let key = |ir: &Ir| -> Vec<(String, String)> {
            ir.symbols
                .iter()
                .map(|s| (s.qualified_name.clone(), s.loc.file.to_string_lossy().into_owned()))
                .collect()
        };
        assert_eq!(key(&from_win), key(&from_unix));
        for s in &from_win.symbols {
            assert!(!s.loc.file.to_string_lossy().contains('\\'), "{:?}", s.loc.file);
            assert_eq!(s.loc.file.to_string_lossy(), unix_rel);
        }
        from_win
    }

    fn qnames(ir: &Ir) -> Vec<String> {
        ir.symbols.iter().map(|s| s.qualified_name.clone()).collect()
    }

    #[test]
    fn crlf_source_matches_lf() {
        let lf = "public class A {\n    @Override\n    public String name() { return \"a\"; }\n}\n";
        let crlf = lf.replace('\n', "\r\n");
        let a = parse_by_extension(Path::new("A.java"), lf).unwrap();
        let b = parse_by_extension(Path::new("A.java"), &crlf).unwrap();
        assert_eq!(
            serde_json::to_string(&a.symbols).unwrap(),
            serde_json::to_string(&b.symbols).unwrap()
        );
        assert!(!b.symbols.iter().any(|s| s.signature.as_deref().unwrap_or("").contains('\r')));
    }

    #[test]
    fn rust_windows_path_gives_module_path() {
        let ir = assert_same_ir("src/ir/types.rs", "pub struct Symbol {}\n");
        assert!(qnames(&ir).contains(&"crate::ir::types::Symbol".to_string()), "{:?}", qnames(&ir));
    }

    #[test]
    fn typescript_windows_path_gives_module_path() {
        let ir = assert_same_ir("src/models/user.ts", "export class User {}\n");
        assert!(qnames(&ir).iter().any(|q| q.starts_with("models.") && q.ends_with("User")), "{:?}", qnames(&ir));
    }

    #[test]
    fn java_windows_path_without_package_gives_package() {
        let ir = assert_same_ir("src/main/java/com/example/User.java", "public class User {}\n");
        assert!(qnames(&ir).contains(&"com.example.User".to_string()), "{:?}", qnames(&ir));
    }

    #[test]
    fn go_windows_path_is_normalized() {
        assert_same_ir("pkg/models/user.go", "package models\n\ntype User struct{}\n");
    }

    #[test]
    fn python_windows_path_gives_module_path() {
        let ir = assert_same_ir("app/models/user.py", "class User:\n    pass\n");
        assert!(qnames(&ir).contains(&"app.models.user.User".to_string()), "{:?}", qnames(&ir));
    }
}
