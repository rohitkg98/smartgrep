use std::collections::HashMap;

use anyhow::Result;

use crate::daemon::logger;

/// Run the `log` command: display query log entries or statistics.
pub fn run(
    limit: usize,
    stats: bool,
    project_root: &Option<std::path::PathBuf>,
) -> Result<()> {
    let root = super::resolve_root(project_root)?;

    if stats {
        print_stats(&root)?;
    } else {
        print_recent(&root, limit)?;
    }

    Ok(())
}

fn print_recent(root: &std::path::Path, limit: usize) -> Result<()> {
    let entries = logger::read_last_n(root, limit);

    if entries.is_empty() {
        println!("No queries logged yet.");
        return Ok(());
    }

    // Compute column widths
    let cmd_width = entries
        .iter()
        .map(|e| e.command.len())
        .max()
        .unwrap_or(7)
        .max(7);
    let args_width = entries
        .iter()
        .map(|e| e.args.len().min(60))
        .max()
        .unwrap_or(4)
        .max(4);

    // Header
    println!(
        "{:<20}  {:<cw$}  {:<aw$}  {:>7}  {:>6}",
        "TIMESTAMP", "COMMAND", "ARGS", "RESULTS", "MS",
        cw = cmd_width, aw = args_width,
    );
    println!("{}", "-".repeat(20 + 2 + cmd_width + 2 + args_width + 2 + 7 + 2 + 6));

    for entry in &entries {
        let args_display = if entry.args.len() > 60 {
            format!("{}...", &entry.args[..57])
        } else {
            entry.args.clone()
        };
        println!(
            "{:<20}  {:<cw$}  {:<aw$}  {:>7}  {:>6}",
            &entry.ts[..entry.ts.len().min(20)],
            entry.command,
            args_display,
            entry.results,
            entry.duration_ms,
            cw = cmd_width, aw = args_width,
        );
    }

    Ok(())
}

fn print_stats(root: &std::path::Path) -> Result<()> {
    let entries = logger::read_entries(root);

    if entries.is_empty() {
        println!("No queries logged yet.");
        return Ok(());
    }

    println!("=== Query Log Statistics ===\n");
    println!("Total queries: {}\n", entries.len());

    // Queries by command type
    let mut by_command: HashMap<String, Vec<&logger::LogEntry>> = HashMap::new();
    for entry in &entries {
        by_command
            .entry(entry.command.clone())
            .or_default()
            .push(entry);
    }

    println!("Queries by command type:");
    let mut commands: Vec<_> = by_command.iter().collect();
    commands.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    for (cmd, entries) in &commands {
        let avg_ms: u64 = entries.iter().map(|e| e.duration_ms).sum::<u64>() / entries.len() as u64;
        println!(
            "  {:<10}  {:>5} queries  avg {:>4}ms",
            cmd,
            entries.len(),
            avg_ms,
        );
    }

    // Most queried args (top 10)
    println!("\nMost queried arguments:");
    let mut by_args: HashMap<String, usize> = HashMap::new();
    for entry in &entries {
        if !entry.args.is_empty() {
            *by_args.entry(entry.args.clone()).or_default() += 1;
        }
    }
    let mut args_sorted: Vec<_> = by_args.into_iter().collect();
    args_sorted.sort_by(|a, b| b.1.cmp(&a.1));
    for (args, count) in args_sorted.iter().take(10) {
        let args_display = if args.len() > 50 {
            format!("{}...", &args[..47])
        } else {
            args.clone()
        };
        println!("  {:>4}x  {}", count, args_display);
    }

    // Queries with 0 results
    let zero_results: Vec<_> = entries.iter().filter(|e| e.results == 0).collect();
    if !zero_results.is_empty() {
        println!("\nQueries with 0 results ({} total):", zero_results.len());
        // Deduplicate and show counts
        let mut zero_by_query: HashMap<(&str, &str), usize> = HashMap::new();
        for entry in &zero_results {
            *zero_by_query
                .entry((&entry.command, &entry.args))
                .or_default() += 1;
        }
        let mut zero_sorted: Vec<_> = zero_by_query.into_iter().collect();
        zero_sorted.sort_by(|a, b| b.1.cmp(&a.1));
        for ((cmd, args), count) in zero_sorted.iter().take(10) {
            let args_display = if args.len() > 40 {
                format!("{}...", &args[..37])
            } else {
                args.to_string()
            };
            println!("  {:>4}x  {} {}", count, cmd, args_display);
        }
    }

    Ok(())
}
