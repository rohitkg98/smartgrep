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

fn assert_calls_include(index: &Index, qn: &str, expected: &[&str]) {
    let calls = calls_of(index, qn);
    for e in expected {
        assert!(calls.contains(e), "{} calls {:?}, missing {}", qn, calls, e);
    }
}

// ---------------------------------------------------------------------------
// Java
// ---------------------------------------------------------------------------

#[test]
fn java_deps_of_method_include_calls() {
    let index = build_index("java_project");
    // `new User(..)` is recorded as `User`; `this.users.put` reduces to `put`.
    assert_calls_include(&index, "com.example.UserService.createUser", &["User", "validate", "put", "getId"]);
    let calls = calls_of(&index, "com.example.UserService.createUser");
    assert!(calls.iter().all(|c| !c.starts_with("this.") && !c.starts_with("users.")), "{:?}", calls);
    // Static call on a class keeps its qualifier.
    assert_calls_include(&index, "com.example.User.toString", &["String.format"]);
    assert_calls_include(&index, "com.example.User.validate", &["ValidationException", "isEmpty"]);
}

#[test]
fn java_refs_and_implementing() {
    let index = build_index("java_project");
    assert!(has_ref(&index, "validate", DepKind::Call, "com.example.UserService.createUser"));
    assert!(has_ref(&index, "validate", DepKind::Call, "com.example.UserService.save"));
    assert!(has_ref(&index, "ValidationException", DepKind::Call, "com.example.User.validate"));
    // `implements Repository<User>` matches the bare interface name.
    assert_eq!(query_names(&index, "classes implementing Repository"), vec!["UserService"]);
}

// ---------------------------------------------------------------------------
// TypeScript
// ---------------------------------------------------------------------------

#[test]
fn ts_deps_of_method_include_calls() {
    let index = build_index("ts_project");
    assert_calls_include(&index, "services.UserService.createUser", &["User", "validate", "set", "getId"]);
    // `Array.from` is a global namespace call and keeps its qualifier.
    assert_calls_include(&index, "services.UserService.listAll", &["Array.from", "values"]);
    assert_calls_include(&index, "services.UserService.deactivateUser", &["Error", "get", "deactivate"]);
}

#[test]
fn ts_refs_to_function_finds_callers() {
    let index = build_index("ts_project");
    assert!(has_ref(&index, "validate", DepKind::Call, "validateAll"));
    assert!(has_ref(&index, "validate", DepKind::Call, "services.UserService.createUser"));
    assert!(has_ref(&index, "User", DepKind::Call, "services.UserService.createUser"));
}

#[test]
fn ts_refs_to_type_finds_named_imports() {
    let index = build_index("ts_project");
    // `import { User, Validatable, Repository, ValidationError } from '../models'`
    assert!(has_ref(&index, "User", DepKind::Import, "services"));
    assert!(has_ref(&index, "ValidationError", DepKind::Import, "services"));
    assert!(has_ref(&index, "models/User", DepKind::Import, "services"));
    // `import { Validatable } from './models'` in the root-level utils.ts.
    assert!(has_ref(&index, "Validatable", DepKind::Import, "(file)"));
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

#[test]
fn python_deps_of_method_include_calls() {
    let index = build_index("python_project");
    // `logger.info` (instance receiver) reduces to `info`.
    assert_calls_include(
        &index,
        "shop.services.user_service.UserService.register",
        &["User", "len", "ensure_valid", "add", "info"],
    );
    assert_calls_include(&index, "shop.models.user.User.ensure_valid", &["validate", "ValidationError"]);
    // `super().__init__(..)` records `__init__`, never `super`.
    let calls = calls_of(&index, "shop.models.base.ValidationError.__init__");
    assert!(calls.contains(&"__init__") && !calls.contains(&"super"), "{:?}", calls);
}

#[test]
fn python_refs_find_imports_and_callers() {
    let index = build_index("python_project");
    assert!(has_ref(&index, "ensure_valid", DepKind::Call, "shop.services.user_service.UserService.register"));
    // `from ..models.user import User` resolves to `shop.models.user.User`.
    assert!(has_ref(&index, "User", DepKind::Import, "shop.services.user_service"));
    assert!(has_ref(&index, "User", DepKind::Call, "shop.services.user_service.UserService.register"));
    // Module-level `logging.getLogger(..)` has no enclosing function.
    assert!(index.refs_to("logging.getLogger").is_empty());
    // Base classes: `class User(Entity, Validatable)`, `class Repository(Generic[E])`.
    assert_eq!(query_names(&index, "classes implementing Entity"), vec!["User"]);
    assert_eq!(query_names(&index, "classes implementing Generic"), vec!["Repository"]);
}

// ---------------------------------------------------------------------------
// Type references (TypeRef / FieldType, derived by the index builder)
// ---------------------------------------------------------------------------

/// `(kind, to_name)` of the type deps of `qn`.
fn type_deps_of(index: &Index, qn: &str) -> Vec<(String, String)> {
    index
        .deps_of(qn)
        .into_iter()
        .filter(|d| matches!(d.kind, DepKind::TypeRef | DepKind::FieldType))
        .map(|d| (d.kind.to_string(), d.to_name.clone()))
        .collect()
}

fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
}

#[test]
fn rust_type_refs_from_params_returns_and_fields() {
    let index = build_index("rust_project");
    // `fn find_by_id(&self, id: u64) -> Option<&User>`
    assert!(has_ref(&index, "User", DepKind::TypeRef, "crate::service::UserService::find_by_id"));
    // `fn save(&mut self, user: User) -> Result<u64, AppError>`; the project
    // defines its own `type Result<T>` alias, so `Result` counts too.
    assert_eq!(
        type_deps_of(&index, "crate::service::UserService::save"),
        pairs(&[("type_ref", "User"), ("type_ref", "Result"), ("type_ref", "AppError")])
    );
    // `users: HashMap<u64, User>`
    assert!(has_ref(&index, "User", DepKind::FieldType, "crate::service::UserService"));
    // `User` has only primitive / std fields (`u64`, `String`, `bool`).
    assert!(type_deps_of(&index, "crate::models::User").is_empty());
    // Primitive-only signatures produce nothing.
    assert!(type_deps_of(&index, "crate::service::UserService::generate_id").is_empty());
}

#[test]
fn go_type_refs_from_pointers_and_maps() {
    let index = build_index("go_project");
    // `func NewUser(...) *User`, `FindByID(id int64) (*User, error)`
    assert!(has_ref(&index, "User", DepKind::TypeRef, "main.NewUser"));
    assert!(has_ref(&index, "User", DepKind::TypeRef, "main.UserService.FindByID"));
    // `users map[int64]*User`
    assert!(has_ref(&index, "User", DepKind::FieldType, "main.UserService"));
    // `Permissions []Permission` (Permission is a `type` alias)
    assert!(has_ref(&index, "Permission", DepKind::FieldType, "main.Role"));
    assert!(has_ref(&index, "AppError", DepKind::TypeRef, "main.NewNotFoundError"));
}

#[test]
fn java_type_refs_from_generics() {
    let index = build_index("java_project");
    // `List<User> listAll()`, `User findById(long)`, `long save(User)`
    for m in ["listAll", "findById", "save"] {
        let from = format!("com.example.UserService.{}", m);
        assert!(has_ref(&index, "User", DepKind::TypeRef, &from), "{}", from);
    }
    // `Map<Long, User> users`
    assert!(has_ref(&index, "User", DepKind::FieldType, "com.example.UserService"));
    // `List<T>` in the generic interface: `T` is not a project type.
    assert!(type_deps_of(&index, "com.example.Repository.listAll").is_empty());
}

#[test]
fn ts_type_refs_from_arrays_and_unions() {
    let index = build_index("ts_project");
    // `listAll(): User[]`, `findById(id): User | null`, `save(user: User)`
    for m in ["listAll", "findById", "save"] {
        let from = format!("services.UserService.{}", m);
        assert!(has_ref(&index, "User", DepKind::TypeRef, &from), "{}", from);
    }
    // `private users: Map<number, User>`
    assert!(has_ref(&index, "User", DepKind::FieldType, "services.UserService"));
    // `validateAll(items: Validatable[])`
    assert!(has_ref(&index, "Validatable", DepKind::TypeRef, "validateAll"));
    // `const createUserService = (): UserService => ...`
    assert!(has_ref(&index, "UserService", DepKind::TypeRef, "services.createUserService"));
}

#[test]
fn python_type_refs_from_annotations() {
    let index = build_index("python_project");
    // `def paginate(items: Iterable[User], ...) -> list[User]` — deduped.
    assert_eq!(
        type_deps_of(&index, "shop.services.user_service.paginate"),
        pairs(&[("type_ref", "User")])
    );
    // `def __init__(self, repo: Repository[User])`
    assert_eq!(
        type_deps_of(&index, "shop.services.user_service.UserService.__init__"),
        pairs(&[("type_ref", "Repository"), ("type_ref", "User")])
    );
    assert!(has_ref(&index, "User", DepKind::TypeRef, "shop.services.user_service.UserService.register"));
    // `role: Role` class attribute
    assert!(has_ref(&index, "Role", DepKind::FieldType, "shop.models.user.User"));
    // `EntityId` type alias used in a field and a param
    assert!(has_ref(&index, "EntityId", DepKind::FieldType, "shop.models.base.Entity"));
    assert!(has_ref(&index, "EntityId", DepKind::TypeRef, "shop.models.base.Entity.__init__"));
}
