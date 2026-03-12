use anyhow::Result;

use crate::index::auto;
use crate::index::types::Index;
use crate::ir::types::Dependency;
use crate::format::OutputFormat;

/// Run the `deps` command: show what a symbol depends on.
pub fn run(name: &str, format_str: &str, project_root: &Option<std::path::PathBuf>) -> Result<()> {
    let root = resolve_root(project_root)?;
    let index = auto::ensure_index(&root)?;

    let results = collect_deps(&index, name);

    if results.is_empty() {
        eprintln!("No symbol found matching '{}'", name);
        return Ok(());
    }

    let output = match OutputFormat::from_str(format_str) {
        OutputFormat::Json => format_json(&results),
        OutputFormat::Text => format_text(&results),
    };

    println!("{}", output);
    Ok(())
}

/// Grouped deps result: qualified name -> list of dependencies.
pub struct DepsGroup<'a> {
    pub qualified_name: String,
    pub deps: Vec<&'a Dependency>,
}

/// Collect dependencies for all symbols matching the given name.
pub fn collect_deps<'a>(index: &'a Index, name: &str) -> Vec<DepsGroup<'a>> {
    let symbols = index.by_name(name);
    let mut results = Vec::new();

    for sym in &symbols {
        let deps = index.deps_of(&sym.qualified_name);
        results.push(DepsGroup {
            qualified_name: sym.qualified_name.clone(),
            deps,
        });
    }

    results
}

fn resolve_root(project_root: &Option<std::path::PathBuf>) -> Result<std::path::PathBuf> {
    if let Some(root) = project_root {
        return Ok(root.clone());
    }
    let cwd = std::env::current_dir()?;
    auto::detect_project_root(&cwd)
        .ok_or_else(|| anyhow::anyhow!("Could not find Cargo.toml in any parent directory"))
}

fn format_text(groups: &[DepsGroup]) -> String {
    if groups.iter().all(|g| g.deps.is_empty()) {
        return "No dependencies found.".to_string();
    }

    let mut lines = Vec::new();
    let multiple_groups = groups.len() > 1;

    // Compute column widths across all deps
    let kind_width = groups
        .iter()
        .flat_map(|g| g.deps.iter())
        .map(|d| format!("{}", d.kind).len())
        .max()
        .unwrap_or(0);
    let name_width = groups
        .iter()
        .flat_map(|g| g.deps.iter())
        .map(|d| d.to_name.len())
        .max()
        .unwrap_or(0);

    for group in groups {
        if multiple_groups {
            lines.push(format!("# {}", group.qualified_name));
        }
        if group.deps.is_empty() {
            if multiple_groups {
                lines.push("  (no dependencies)".to_string());
            }
            continue;
        }
        for dep in &group.deps {
            let kind_str = format!("{}", dep.kind);
            let loc = format!("{}:{}", dep.loc.file.display(), dep.loc.line);
            lines.push(format!(
                "{:<kw$}  {:<nw$}  {}",
                kind_str, dep.to_name, loc,
                kw = kind_width, nw = name_width,
            ));
        }
    }

    lines.join("\n")
}

fn format_json(groups: &[DepsGroup]) -> String {
    let entries: Vec<serde_json::Value> = groups
        .iter()
        .map(|g| {
            let deps: Vec<serde_json::Value> = g
                .deps
                .iter()
                .map(|d| {
                    serde_json::json!({
                        "to_name": d.to_name,
                        "kind": format!("{}", d.kind),
                        "file": d.loc.file.display().to_string(),
                        "line": d.loc.line,
                    })
                })
                .collect();
            serde_json::json!({
                "qualified_name": g.qualified_name,
                "deps": deps,
            })
        })
        .collect();
    serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string())
}
