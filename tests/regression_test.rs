//! Regression assertions against the multi-file projects in tests/regression.
//!
//! Each project is parsed with the real parsers and built with the real index
//! builder, then checked for concrete call-graph / reference facts that an
//! agent relies on (`refs`, `deps`, `implementing`).

use std::path::{Path, PathBuf};

use smartgrep::index::auto::parse_all_sources;
use smartgrep::index::builder;
use smartgrep::index::types::Index;
use smartgrep::ir::types::{DepKind, Dependency};
use smartgrep::query::{engine, parser};

fn project(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/regression")
        .join(name)
}

fn build_index(name: &str) -> Index {
    let ir = parse_all_sources(&project(name)).expect("parse project");
    builder::build(&ir)
}

/// `(kind, from_qualified)` pairs of refs to `name`, sorted.
fn refs(index: &Index, name: &str) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = index
        .refs_to(name)
        .iter()
        .map(|d| (d.kind.to_string(), d.from_qualified.clone()))
        .collect();
    v.sort();
    v.dedup();
    v
}

fn has_ref(index: &Index, name: &str, kind: DepKind, from: &str) -> bool {
    index
        .refs_to(name)
        .iter()
        .any(|d| d.kind == kind && d.from_qualified == from)
}

/// Call targets (`to_name`) of a function, in source order.
fn calls_of<'a>(index: &'a Index, qn: &str) -> Vec<&'a str> {
    index
        .deps_of(qn)
        .into_iter()
        .filter(|d| d.kind == DepKind::Call)
        .map(|d| d.to_name.as_str())
        .collect()
}

fn query_names(index: &Index, q: &str) -> Vec<String> {
    let batch = parser::parse(q).unwrap();
    let rows = engine::execute_query(&batch.queries[0], index).unwrap();
    let mut n: Vec<String> = rows
        .iter()
        .map(|r| r.get("name").unwrap().to_string())
        .collect();
    n.sort();
    n
}

fn find_ref<'a>(index: &'a Index, name: &str, from: &str) -> &'a Dependency {
    index
        .refs_to(name)
        .into_iter()
        .find(|d| d.from_qualified == from)
        .unwrap_or_else(|| panic!("no ref to {} from {}", name, from))
}

// ---------------------------------------------------------------------------
// Rust
// ---------------------------------------------------------------------------

#[test]
fn rust_refs_to_type_finds_imports_and_callers() {
    let index = build_index("rust_project");

    // `use crate::models::{User, ...}` (grouped, split per leaf) in service.rs,
    // `pub use models::User` in lib.rs, and `User::new(...)` in create_user.
    assert!(has_ref(&index, "User", DepKind::Import, "crate::service"));
    assert!(has_ref(&index, "User", DepKind::Import, "crate"));
    assert!(has_ref(
        &index,
        "User",
        DepKind::Call,
        "crate::service::UserService::create_user"
    ));

    // `use crate::errors::AppError` in two modules + `AppError::X(..)` calls.
    assert!(has_ref(&index, "AppError", DepKind::Import, "crate::models"));
    assert!(has_ref(&index, "AppError", DepKind::Import, "crate::service"));
    assert!(has_ref(
        &index,
        "AppError",
        DepKind::Call,
        "crate::service::UserService::deactivate_user"
    ));
    assert!(has_ref(&index, "AppError", DepKind::Call, "crate::errors::wrap_error"));

    // Fully qualified query hits the import by path suffix.
    assert!(has_ref(
        &index,
        "crate::errors::AppError",
        DepKind::Import,
        "crate::service"
    ));
}

#[test]
fn rust_grouped_import_split_per_leaf() {
    let index = build_index("rust_project");
    for leaf in ["User", "Validatable", "Identifiable"] {
        let full = format!("crate::models::{}", leaf);
        assert!(
            index
                .deps
                .iter()
                .any(|d| d.kind == DepKind::Import
                    && d.from_qualified == "crate::service"
                    && d.to_name == full),
            "missing import {}",
            full
        );
    }
}

#[test]
fn rust_deps_of_function_include_calls() {
    let index = build_index("rust_project");
    assert_eq!(
        calls_of(&index, "crate::service::UserService::create_user"),
        vec!["User::new", "validate", "id", "insert"]
    );
    // Closure body (`ok_or_else(|| AppError::NotFound(..))`) belongs to delete.
    assert_eq!(
        calls_of(&index, "crate::service::UserService::delete"),
        vec!["remove", "map", "ok_or_else", "AppError::NotFound"]
    );
    // `write!` / `format!` are macros, not calls.
    assert!(calls_of(&index, "crate::models::User::display_id").is_empty());
}

#[test]
fn rust_refs_to_function_finds_callers() {
    let index = build_index("rust_project");
    assert_eq!(
        refs(&index, "User::new"),
        vec![(
            "call".to_string(),
            "crate::service::UserService::create_user".to_string()
        )]
    );
    assert_eq!(
        refs(&index, "validate"),
        vec![
            ("call".to_string(), "crate::service::UserService::create_user".to_string()),
            ("call".to_string(), "crate::service::UserService::save".to_string()),
        ]
    );
    let d = find_ref(&index, "deactivate", "crate::service::UserService::deactivate_user");
    assert_eq!(d.loc.file, PathBuf::from("src/service.rs"));
    assert_eq!(d.loc.line, 35);
}

#[test]
fn rust_implementing_matches_qualified_and_generic_traits() {
    let index = build_index("rust_project");
    // `impl fmt::Display for User` / `for AppError`
    assert_eq!(query_names(&index, "structs implementing Display"), vec!["User"]);
    assert_eq!(query_names(&index, "enums implementing Display"), vec!["AppError"]);
    assert_eq!(query_names(&index, "enums implementing fmt::Display"), vec!["AppError"]);
    // `impl std::error::Error for AppError`
    assert_eq!(query_names(&index, "enums implementing Error"), vec!["AppError"]);
    // `impl Repository<User> for UserService`
    assert_eq!(query_names(&index, "structs implementing Repository"), vec!["UserService"]);
    assert_eq!(query_names(&index, "structs implementing Validatable"), vec!["User"]);
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

#[test]
fn go_deps_of_function_include_calls() {
    let index = build_index("go_project");
    assert_eq!(
        calls_of(&index, "main.UserService.CreateUser"),
        vec!["NewUser", "Validate", "fmt.Errorf", "GetID"]
    );
    assert_eq!(
        calls_of(&index, "main.UserService.DeactivateUser"),
        vec!["fmt.Errorf", "Deactivate"]
    );
    // `make` is a builtin
    assert!(calls_of(&index, "main.NewUserService").is_empty());
}

#[test]
fn go_refs_to_function_finds_callers() {
    let index = build_index("go_project");
    let d = find_ref(&index, "NewUser", "main.UserService.CreateUser");
    assert_eq!(d.kind, DepKind::Call);
    assert_eq!(d.loc.file, PathBuf::from("service.go"));
    assert_eq!(d.loc.line, 26);

    let errorf_callers: Vec<String> = refs(&index, "fmt.Errorf")
        .into_iter()
        .map(|(_, from)| from)
        .collect();
    assert_eq!(
        errorf_callers,
        vec![
            "main.User.Validate",
            "main.UserService.CreateUser",
            "main.UserService.DeactivateUser",
            "main.UserService.FindByID",
            "main.WrapError",
        ]
    );
    // bare name and `::` spelling match the same deps
    assert_eq!(refs(&index, "Errorf").len(), 5);
    assert_eq!(refs(&index, "fmt::Errorf").len(), 5);
}

#[test]
fn go_refs_to_package_finds_imports_and_calls() {
    let index = build_index("go_project");
    let fmt_refs = index.refs_to("fmt");
    let import_files: std::collections::BTreeSet<_> = fmt_refs
        .iter()
        .filter(|d| d.kind == DepKind::Import)
        .map(|d| d.loc.file.clone())
        .collect();
    assert_eq!(import_files.len(), 3);
    assert!(fmt_refs
        .iter()
        .any(|d| d.kind == DepKind::Call && d.from_qualified == "main.AppError.Error"));
}

// ---------------------------------------------------------------------------
// TODO(java/ts/python): fill in after the Java / TypeScript / Python parser
// call-graph work is merged. Suggested facts:
//   - java_project: refs_to(<type>) finds imports + `new <Type>()` callers;
//     deps_of(<method>) includes its calls; `classes implementing <Iface>`
//     matches `implements <Iface><T>` / fully qualified interfaces.
//   - ts_project: refs_to(<class>) finds `import { X }` sites + `new X()`;
//     deps_of(<function>) includes calls; `classes implementing <Iface>`.
//   - python_project: refs_to(<class>) finds `from m import X` + `X()` calls;
//     deps_of(<def>) includes calls; `classes implementing <Base>` via
//     `Base[T]` / `module.Base`.
// ---------------------------------------------------------------------------
