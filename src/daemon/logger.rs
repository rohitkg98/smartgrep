use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const LOG_DIR: &str = ".smartgrep";
const LOG_FILE: &str = "queries.log";
const MAX_LOG_ENTRIES: usize = 10_000;

/// A single query log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// ISO 8601 timestamp
    pub ts: String,
    /// Command type (query, ls, show, deps, refs, context)
    pub command: String,
    /// Command arguments (query string, symbol name, file path, etc.)
    pub args: String,
    /// Number of result rows/symbols returned
    pub results: usize,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Return the path to the query log file for a project root.
pub fn log_path(project_root: &Path) -> PathBuf {
    project_root.join(LOG_DIR).join(LOG_FILE)
}

/// Append a log entry to the query log file.
/// Creates the .smartgrep directory and log file if they don't exist.
pub fn append(project_root: &Path, entry: &LogEntry) {
    let path = log_path(project_root);

    // Ensure directory exists
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }

    let line = match serde_json::to_string(entry) {
        Ok(json) => json,
        Err(_) => return,
    };

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path);

    if let Ok(mut file) = file {
        let _ = writeln!(file, "{}", line);
    }

    // Check if we need to truncate (best-effort, non-blocking)
    maybe_truncate(&path);
}

/// Read log entries from the log file.
/// Returns entries in chronological order (oldest first).
pub fn read_entries(project_root: &Path) -> Vec<LogEntry> {
    let path = log_path(project_root);
    if !path.exists() {
        return Vec::new();
    }

    let file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let reader = BufReader::new(file);
    reader
        .lines()
        .flatten()
        .filter_map(|line| serde_json::from_str::<LogEntry>(&line).ok())
        .collect()
}

/// Read the last N entries from the log file.
pub fn read_last_n(project_root: &Path, n: usize) -> Vec<LogEntry> {
    let entries = read_entries(project_root);
    let start = entries.len().saturating_sub(n);
    entries[start..].to_vec()
}

/// If the log file exceeds MAX_LOG_ENTRIES lines, truncate it by keeping
/// only the most recent half of entries.
fn maybe_truncate(path: &Path) {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };

    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().flatten().collect();

    if lines.len() <= MAX_LOG_ENTRIES {
        return;
    }

    // Keep the most recent half
    let keep_from = lines.len() / 2;
    let kept = &lines[keep_from..];

    // Write back atomically via temp file
    let tmp_path = path.with_extension("log.tmp");
    if let Ok(mut tmp) = fs::File::create(&tmp_path) {
        for line in kept {
            let _ = writeln!(tmp, "{}", line);
        }
        let _ = fs::rename(&tmp_path, path);
    }
}

/// Create a log entry with the current timestamp.
pub fn make_entry(command: &str, args: &str, results: usize, duration_ms: u64) -> LogEntry {
    let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    LogEntry {
        ts,
        command: command.to_string(),
        args: args.to_string(),
        results,
        duration_ms,
    }
}

/// Count the number of result items in command output.
/// Heuristic: count non-empty, non-header lines.
pub fn count_results(output: &str) -> usize {
    if output.is_empty() || output == "No results." || output == "No symbols found."
        || output.starts_with("No symbol found")
        || output.starts_with("No references found")
        || output.starts_with("No dependencies found")
    {
        return 0;
    }

    output
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty()
                && !trimmed.starts_with("[paths]")
                && !trimmed.starts_with("# Query")
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_make_entry() {
        let entry = make_entry("ls", "functions", 5, 3);
        assert_eq!(entry.command, "ls");
        assert_eq!(entry.args, "functions");
        assert_eq!(entry.results, 5);
        assert_eq!(entry.duration_ms, 3);
        assert!(!entry.ts.is_empty());
    }

    #[test]
    fn test_log_entry_roundtrip() {
        let entry = make_entry("query", "structs where visibility = public", 14, 3);
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: LogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.command, "query");
        assert_eq!(parsed.args, "structs where visibility = public");
        assert_eq!(parsed.results, 14);
    }

    #[test]
    fn test_append_and_read() {
        let dir = std::env::temp_dir().join("smartgrep-test-log");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let entry1 = make_entry("ls", "functions", 5, 2);
        let entry2 = make_entry("show", "Trip", 1, 1);

        append(&dir, &entry1);
        append(&dir, &entry2);

        let entries = read_entries(&dir);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].command, "ls");
        assert_eq!(entries[1].command, "show");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_read_last_n() {
        let dir = std::env::temp_dir().join("smartgrep-test-log-last-n");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        for i in 0..5 {
            let entry = make_entry("ls", &format!("query-{}", i), i, 1);
            append(&dir, &entry);
        }

        let last3 = read_last_n(&dir, 3);
        assert_eq!(last3.len(), 3);
        assert_eq!(last3[0].args, "query-2");
        assert_eq!(last3[2].args, "query-4");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_count_results() {
        assert_eq!(count_results(""), 0);
        assert_eq!(count_results("No results."), 0);
        assert_eq!(count_results("No symbols found."), 0);
        assert_eq!(count_results("Struct  Foo  src/main.rs:10"), 1);
        assert_eq!(
            count_results("[paths] src/long/path/ = [P]\n\nStruct  Foo  [P]main.rs:10\nStruct  Bar  [P]lib.rs:20"),
            2
        );
    }

    #[test]
    fn test_read_nonexistent_log() {
        let dir = std::env::temp_dir().join("smartgrep-test-nonexistent");
        let entries = read_entries(&dir);
        assert!(entries.is_empty());
    }
}
