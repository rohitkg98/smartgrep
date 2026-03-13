use anyhow::Result;

use crate::daemon::client;
use crate::format::path_alias;
use crate::index::auto;
use crate::index::types::Index;
use crate::ir::types::Dependency;
use crate::format::OutputFormat;

/// Run the `deps` command: show what a symbol depends on.
pub fn run(name: &str, format_str: &str, project_root: &Option<std::path::PathBuf>, no_daemon: bool) -> Result<()> {
    let root = super::resolve_root(project_root)?;

    // Try daemon first (auto-starts if needed, skipped if --no-daemon)
    if let Some(output) = client::try_daemon(&root, "deps", name, format_str, no_daemon) {
        println!("{}", output);
        return Ok(());
    }

    let start = std::time::Instant::now();
    let index = auto::ensure_index(&root)?;

    let results = collect_deps(&index, name);

    if results.is_empty() {
        super::log_direct(&root, "deps", name, "", start.elapsed().as_millis() as u64);
        eprintln!("No symbol found matching '{}'", name);
        return Ok(());
    }

    let output = match OutputFormat::from_str(format_str) {
        OutputFormat::Json => format_json(&results),
        OutputFormat::Text => format_text(&results),
    };

    super::log_direct(&root, "deps", name, &output, start.elapsed().as_millis() as u64);
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

fn format_text(groups: &[DepsGroup]) -> String {
    if groups.iter().all(|g| g.deps.is_empty()) {
        return "No dependencies found.".to_string();
    }

    let mut lines = Vec::new();
    let multiple_groups = groups.len() > 1;

    // Collect file paths for alias detection
    let file_paths: Vec<&str> = groups
        .iter()
        .flat_map(|g| g.deps.iter())
        .map(|d| d.loc.file.to_str().unwrap_or(""))
        .collect();
    let alias = path_alias::compute_path_alias(&file_paths);

    // Emit alias header if applicable
    if let Some(ref a) = alias {
        lines.push(a.header());
        lines.push(String::new());
    }

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
            let raw_file = dep.loc.file.to_string_lossy();
            let file_str = if let Some(ref a) = alias {
                a.shorten(&raw_file)
            } else {
                raw_file.to_string()
            };
            let loc = format!("{}:{}", file_str, dep.loc.line);
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
