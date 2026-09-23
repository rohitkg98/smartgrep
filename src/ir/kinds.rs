//! Classification helpers for language-native kind strings.
//!
//! Symbols carry the language's own vocabulary (`fn`, `func`, `function`,
//! `class`, `struct`, ...). Formatters and commands that need to treat kinds
//! by category (e.g. "is this callable?") should use these helpers instead of
//! matching on kind strings locally.

/// Free-standing function kinds across languages
/// (Rust `fn`, Go `func`, TS `function`).
pub const FUNCTION_KINDS: &[&str] = &["fn", "func", "function"];

/// Type-defining kinds across languages.
pub const TYPE_KINDS: &[&str] = &[
    "struct",
    "class",
    "record",
    "enum",
    "trait",
    "interface",
    "annotation",
    "type",
];

/// A free-standing function (not a method).
pub fn is_function_kind(kind: &str) -> bool {
    FUNCTION_KINDS.contains(&kind)
}

/// Anything with params/return type: free functions and methods.
pub fn is_callable_kind(kind: &str) -> bool {
    is_function_kind(kind) || kind == "method"
}

/// A type definition (struct, class, interface, enum, type alias, ...).
pub fn is_type_kind(kind: &str) -> bool {
    TYPE_KINDS.contains(&kind)
}

/// Kinds whose `fields` represent data members worth listing inline.
pub fn has_data_fields(kind: &str) -> bool {
    matches!(kind, "struct" | "class" | "record")
}

/// Stable display ordering for kinds: types first, then consts/modules,
/// then functions, then everything else. Ties are broken by the caller
/// (typically by kind string, then name).
pub fn kind_rank(kind: &str) -> u8 {
    if let Some(i) = TYPE_KINDS.iter().position(|k| *k == kind) {
        return i as u8;
    }
    match kind {
        "const" => 20,
        "mod" | "namespace" => 21,
        k if is_function_kind(k) => 30,
        "method" => 31,
        "impl" => 32,
        _ => 40,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification() {
        for k in ["fn", "func", "function"] {
            assert!(is_function_kind(k));
            assert!(is_callable_kind(k));
            assert!(!is_type_kind(k));
        }
        assert!(is_callable_kind("method"));
        assert!(!is_function_kind("method"));
        assert!(is_type_kind("class"));
        assert!(is_type_kind("interface"));
        assert!(!is_type_kind("const"));
    }

    #[test]
    fn types_rank_before_functions() {
        assert!(kind_rank("class") < kind_rank("const"));
        assert!(kind_rank("const") < kind_rank("fn"));
        assert!(kind_rank("struct") < kind_rank("interface"));
        assert!(kind_rank("function") < kind_rank("whatever"));
    }
}
