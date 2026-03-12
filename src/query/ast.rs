/// AST for the smartgrep query DSL.
///
/// Grammar (informal):
///   batch       = query (";" query)*
///   query       = source ("|" stage)*
///   source      = source_kind [argument] [where_clause]
///   source_kind = "symbols" | "structs" | "functions" | "methods" | "traits"
///                | "enums" | "impls" | "consts" | "types" | "modules"
///                | "symbol" <name> | "deps" [<name>] | "refs" [<name>]
///   where_clause = "where" and_group ("or" and_group)*
///   and_group   = condition ("and" condition)*
///   condition   = field op value
///   field       = "name" | "file" | "visibility" | "kind" | "parent"
///                | "from" | "to" | "dep_kind"
///                | "field_count" | "param_count"
///   op          = "=" | "!=" | "contains" | ">" | "<" | ">=" | "<="
///   value       = quoted_string | bare_word | number
///   stage       = with_stage | show_stage | where_stage | sort_stage | limit_stage
///   with_stage  = "with" enrichment ("," enrichment)*
///   enrichment  = "fields" | "params" | "deps" | "refs" | "signature"
///   show_stage  = "show" column ("," column)*
///   where_stage = "where" and_group ("or" and_group)*
///   sort_stage  = "sort" field ["asc"|"desc"]
///   limit_stage = "limit" number

/// A batch of one or more queries separated by ";".
#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    pub queries: Vec<Query>,
}

/// A single query: a source clause piped through zero or more stages.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub source: Source,
    pub stages: Vec<Stage>,
}

/// The source clause determines what data to start with.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// All symbols, optionally filtered by kind.
    /// "symbols", "structs", "functions", etc.
    Symbols {
        kind_filter: Option<KindFilter>,
        in_file: Option<String>,
        where_clause: Vec<Vec<Condition>>,
    },
    /// A specific symbol by name: "symbol Foo"
    Symbol {
        name: String,
        where_clause: Vec<Vec<Condition>>,
    },
    /// Dependencies of a symbol: "deps Foo" or all deps: "deps"
    Deps {
        name: Option<String>,
        where_clause: Vec<Vec<Condition>>,
    },
    /// References to a symbol: "refs Foo" or all refs: "refs"
    Refs {
        name: Option<String>,
        where_clause: Vec<Vec<Condition>>,
    },
}

/// Symbol kind filter for the source clause.
#[derive(Debug, Clone, PartialEq)]
pub enum KindFilter {
    Functions,
    Methods,
    Structs,
    Enums,
    Traits,
    Impls,
    Consts,
    Types,
    Modules,
}

/// A pipeline stage that transforms query results.
#[derive(Debug, Clone, PartialEq)]
pub enum Stage {
    /// Enrich results with additional data: "with fields, deps"
    With { enrichments: Vec<Enrichment> },
    /// Select specific columns to show: "show name, file, kind"
    Show { columns: Vec<String> },
    /// Post-filter results: "where field_count > 5"
    Where { conditions: Vec<Vec<Condition>> },
    /// Sort results: "sort name asc"
    Sort { field: String, descending: bool },
    /// Limit results: "limit 10"
    Limit { count: usize },
}

/// An enrichment adds extra data to each result row.
#[derive(Debug, Clone, PartialEq)]
pub enum Enrichment {
    Fields,
    Params,
    Deps,
    Refs,
    Signature,
}

/// A filter condition: field op value.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub field: String,
    pub op: Op,
    pub value: Value,
}

/// Comparison operators.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Eq,
    NotEq,
    Contains,
    Gt,
    Lt,
    Gte,
    Lte,
    StartsWith,
    EndsWith,
}

/// A value in a condition.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Number(i64),
}

impl Value {
    pub fn as_str(&self) -> &str {
        match self {
            Value::String(s) => s.as_str(),
            Value::Number(_) => {
                // For numeric comparisons use as_number()
                ""
            }
        }
    }

    pub fn as_number(&self) -> Option<i64> {
        match self {
            Value::Number(n) => Some(*n),
            Value::String(s) => s.parse::<i64>().ok(),
        }
    }
}
