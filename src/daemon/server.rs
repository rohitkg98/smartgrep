use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use notify::{Event, RecursiveMode, Watcher};

use crate::index::auto;
use crate::index::types::Index;

use super::client::{pid_path, socket_path};
use super::protocol::{Request, Response};

/// Entry point for the hidden `run-server` CLI subcommand.
pub fn run_server_cmd(project_root: &Option<std::path::PathBuf>, idle_timeout: u64) -> anyhow::Result<()> {
    let root = crate::commands::resolve_root(project_root)?;
    run_server(&root, idle_timeout)
}

/// Run the daemon server. This function blocks until shutdown.
///
/// It:
/// 1. Builds the index
/// 2. Starts a file watcher for incremental re-indexing
/// 3. Listens on a Unix socket for requests
/// 4. Auto-shuts down after idle_timeout_secs of inactivity
pub fn run_server(project_root: &Path, idle_timeout_secs: u64) -> Result<()> {
    let project_root = project_root.canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());

    let sock_path = socket_path(&project_root);
    let pid_file = pid_path(&project_root);

    // Clean up stale socket if it exists
    if sock_path.exists() {
        let _ = std::fs::remove_file(&sock_path);
    }

    // Write PID file
    std::fs::write(&pid_file, std::process::id().to_string())?;

    // Build the initial index
    eprintln!("[daemon] Building index for {}...", project_root.display());
    let index = auto::rebuild_index(&project_root)
        .context("Failed to build initial index")?;
    eprintln!(
        "[daemon] Index built: {} symbols, {} deps",
        index.symbols.len(),
        index.deps.len()
    );

    let index = Arc::new(Mutex::new(index));
    let last_activity = Arc::new(Mutex::new(Instant::now()));
    let shutdown = Arc::new(AtomicBool::new(false));

    // Start file watcher
    let watcher_index = Arc::clone(&index);
    let watcher_root = project_root.clone();
    let watcher_shutdown = Arc::clone(&shutdown);
    let _watcher = start_file_watcher(watcher_root, watcher_index, watcher_shutdown)?;

    // Bind the Unix socket listener
    let listener = UnixListener::bind(&sock_path)
        .with_context(|| format!("Cannot bind Unix socket at {}", sock_path.display()))?;

    // Set a timeout on accept so we can check the idle timer periodically
    listener.set_nonblocking(true)?;

    eprintln!("[daemon] Listening on {}", sock_path.display());

    // Main loop
    let idle_timeout = Duration::from_secs(idle_timeout_secs);

    while !shutdown.load(Ordering::Relaxed) {
        // Check idle timeout
        {
            let last = last_activity.lock().unwrap();
            if last.elapsed() >= idle_timeout {
                eprintln!("[daemon] Idle timeout reached, shutting down.");
                break;
            }
        }

        // Try to accept a connection (non-blocking)
        match listener.accept() {
            Ok((stream, _addr)) => {
                // Update activity timestamp
                {
                    let mut last = last_activity.lock().unwrap();
                    *last = Instant::now();
                }

                // Handle the request
                if let Err(e) = handle_connection(
                    stream,
                    &index,
                    &project_root,
                    &shutdown,
                ) {
                    eprintln!("[daemon] Error handling connection: {}", e);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No pending connection; sleep briefly to avoid busy-looping
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                eprintln!("[daemon] Accept error: {}", e);
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }

    // Clean up
    eprintln!("[daemon] Cleaning up...");
    let _ = std::fs::remove_file(&sock_path);
    let _ = std::fs::remove_file(&pid_file);

    Ok(())
}

/// Handle a single client connection: read one request, execute, send response.
fn handle_connection(
    stream: std::os::unix::net::UnixStream,
    index: &Arc<Mutex<Index>>,
    project_root: &Path,
    shutdown: &Arc<AtomicBool>,
) -> Result<()> {
    // Set blocking mode for this connection
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;

    let request: Request = match serde_json::from_str(&line) {
        Ok(r) => r,
        Err(e) => {
            let resp = Response::error(format!("Invalid request: {}", e));
            send_response(&stream, &resp)?;
            return Ok(());
        }
    };

    let start = Instant::now();
    let response = dispatch_request(&request, index, project_root, shutdown);
    let elapsed_ms = start.elapsed().as_millis() as u64;

    // Log the query (skip internal commands like ping/shutdown)
    if !matches!(request.command.as_str(), "ping" | "shutdown") {
        let result_count = response
            .output
            .as_ref()
            .map(|o| super::logger::count_results(o))
            .unwrap_or(0);
        let entry = super::logger::make_entry(
            &request.command,
            &request.args,
            result_count,
            elapsed_ms,
        );
        super::logger::append(project_root, &entry);
    }

    send_response(&stream, &response)?;
    Ok(())
}

fn send_response(
    mut stream: &std::os::unix::net::UnixStream,
    response: &Response,
) -> Result<()> {
    let mut payload = serde_json::to_string(response)?;
    payload.push('\n');
    stream.write_all(payload.as_bytes())?;
    stream.flush()?;
    Ok(())
}

/// Dispatch a request to the appropriate handler.
fn dispatch_request(
    request: &Request,
    index: &Arc<Mutex<Index>>,
    project_root: &Path,
    shutdown: &Arc<AtomicBool>,
) -> Response {
    match request.command.as_str() {
        "ping" => Response::ok("pong".to_string()),

        "shutdown" => {
            shutdown.store(true, Ordering::Relaxed);
            Response::ok("shutting down".to_string())
        }

        "ls" => {
            let idx = index.lock().unwrap();
            dispatch_ls(&request.args, &request.format, &idx)
        }

        "show" => {
            let idx = index.lock().unwrap();
            dispatch_show(&request.args, &request.format, &idx)
        }

        "deps" => {
            let idx = index.lock().unwrap();
            dispatch_deps(&request.args, &request.format, &idx)
        }

        "refs" => {
            let idx = index.lock().unwrap();
            dispatch_refs(&request.args, &request.format, &idx)
        }

        "context" => {
            // Context parses a single file — doesn't need the index
            dispatch_context(&request.args, &request.format, project_root)
        }

        "query" => {
            let idx = index.lock().unwrap();
            dispatch_query(&request.args, &request.format, &idx)
        }

        "index" => {
            // Force re-index
            match auto::rebuild_index(project_root) {
                Ok(new_index) => {
                    let summary = format!(
                        "Indexed {} symbols, {} dependencies",
                        new_index.symbols.len(),
                        new_index.deps.len()
                    );
                    let mut idx = index.lock().unwrap();
                    *idx = new_index;
                    Response::ok(summary)
                }
                Err(e) => Response::error(format!("Re-index failed: {}", e)),
            }
        }

        other => Response::error(format!("Unknown command: {}", other)),
    }
}

// --- Individual command dispatchers that work against the in-memory Index ---

fn dispatch_ls(args: &str, format: &str, index: &Index) -> Response {
    use crate::format::OutputFormat;
    use crate::commands::ls::parse_kind_filter;

    let kind_filter = if args.is_empty() {
        None
    } else {
        parse_kind_filter(args)
    };

    let symbols: Vec<_> = if let Some(ref kind) = kind_filter {
        index.by_kind(kind)
    } else {
        index.symbols.iter().collect()
    };

    let output = match OutputFormat::from_str(format) {
        OutputFormat::Json => serde_json::to_string_pretty(&symbols).unwrap_or_else(|_| "[]".to_string()),
        OutputFormat::Text => format_ls_text(&symbols),
    };

    Response::ok(output)
}

fn format_ls_text(symbols: &[&crate::ir::types::Symbol]) -> String {
    use crate::commands::ls::display_name;

    if symbols.is_empty() {
        return "No symbols found.".to_string();
    }

    let kind_width = symbols.iter().map(|s| format!("{}", s.kind).len()).max().unwrap_or(0);
    let name_width = symbols.iter().map(|s| display_name(s).len()).max().unwrap_or(0);

    let mut lines = Vec::new();
    for sym in symbols {
        let kind_str = format!("{}", sym.kind);
        let name = display_name(sym);
        let loc = format!("{}:{}", sym.loc.file.display(), sym.loc.line);

        lines.push(format!(
            "{:<kw$}  {:<nw$}  {}",
            kind_str, name, loc,
            kw = kind_width, nw = name_width,
        ));
    }
    lines.join("\n")
}

fn dispatch_show(args: &str, _format: &str, index: &Index) -> Response {
    let symbols = index.by_name(args);
    if symbols.is_empty() {
        return Response::ok(format!("No symbol found matching '{}'", args));
    }
    let output = crate::commands::show::format_text(&symbols);
    Response::ok(output)
}

fn dispatch_deps(args: &str, _format: &str, index: &Index) -> Response {
    let results = crate::commands::deps::collect_deps(index, args);
    if results.is_empty() {
        return Response::ok(format!("No symbol found matching '{}'", args));
    }

    // Use a simple text format for deps
    let mut lines = Vec::new();
    for group in &results {
        if results.len() > 1 {
            lines.push(format!("# {}", group.qualified_name));
        }
        if group.deps.is_empty() {
            if results.len() > 1 {
                lines.push("  (no dependencies)".to_string());
            } else {
                lines.push("No dependencies found.".to_string());
            }
            continue;
        }
        for dep in &group.deps {
            let kind_str = format!("{}", dep.kind);
            let loc = format!("{}:{}", dep.loc.file.display(), dep.loc.line);
            lines.push(format!("{}  {}  {}", kind_str, dep.to_name, loc));
        }
    }
    Response::ok(lines.join("\n"))
}

fn dispatch_refs(args: &str, _format: &str, index: &Index) -> Response {
    let refs = index.refs_to(args);
    if refs.is_empty() {
        return Response::ok(format!("No references found for '{}'.", args));
    }

    let mut lines = Vec::new();
    for dep in &refs {
        let kind_str = format!("{}", dep.kind);
        let loc = format!("{}:{}", dep.loc.file.display(), dep.loc.line);
        lines.push(format!("{}  {}  {}", kind_str, dep.from_qualified, loc));
    }
    Response::ok(lines.join("\n"))
}

fn dispatch_context(args: &str, format: &str, project_root: &Path) -> Response {
    use std::path::PathBuf;
    use crate::format::OutputFormat;
    use crate::parser::java as java_parser;
    use crate::parser::rust as rust_parser;

    let file = PathBuf::from(args);
    let full_path = if file.is_absolute() {
        file.clone()
    } else {
        project_root.join(&file)
    };

    let source = match std::fs::read_to_string(&full_path) {
        Ok(s) => s,
        Err(e) => return Response::error(format!("Cannot read {}: {}", full_path.display(), e)),
    };

    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
    let result = match ext {
        "java" => java_parser::parse_file(&file, &source),
        _ => rust_parser::parse_file(&file, &source),
    };

    match result {
        Ok(ir) => {
            let output = match OutputFormat::from_str(format) {
                OutputFormat::Json => crate::format::json::format_symbols(&ir),
                OutputFormat::Text => crate::format::text::format_symbols(&ir),
            };
            Response::ok(output)
        }
        Err(e) => Response::error(format!("Parse error: {}", e)),
    }
}

fn dispatch_query(args: &str, format: &str, index: &Index) -> Response {
    use crate::query::{engine, parser};

    match parser::parse(args) {
        Ok(batch) => match engine::execute_batch(&batch, index, format) {
            Ok(output) => Response::ok(output),
            Err(e) => Response::error(format!("Query execution error: {}", e)),
        },
        Err(e) => Response::error(format!("Query parse error: {}", e)),
    }
}

/// Start a file watcher that re-indexes when source files change.
fn start_file_watcher(
    project_root: PathBuf,
    index: Arc<Mutex<Index>>,
    shutdown: Arc<AtomicBool>,
) -> Result<notify::RecommendedWatcher> {
    let root_clone = project_root.clone();

    let mut watcher = notify::recommended_watcher(
        move |res: std::result::Result<Event, notify::Error>| {
            if shutdown.load(Ordering::Relaxed) {
                return;
            }
            match res {
                Ok(event) => {
                    // Only re-index on file modifications/creations/deletions of .rs/.java files
                    let dominated_by_source = event.paths.iter().any(|p| {
                        p.extension().map_or(false, |e| e == "rs" || e == "java")
                    });
                    if !dominated_by_source {
                        return;
                    }

                    eprintln!("[daemon] File change detected, re-indexing...");
                    match auto::rebuild_index(&root_clone) {
                        Ok(new_index) => {
                            eprintln!(
                                "[daemon] Re-indexed: {} symbols, {} deps",
                                new_index.symbols.len(),
                                new_index.deps.len()
                            );
                            let mut idx = index.lock().unwrap();
                            *idx = new_index;
                        }
                        Err(e) => {
                            eprintln!("[daemon] Re-index failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[daemon] Watch error: {}", e);
                }
            }
        },
    )?;

    // Watch the src/ directory recursively
    let src_dir = project_root.join("src");
    if src_dir.exists() {
        watcher.watch(&src_dir, RecursiveMode::Recursive)?;
    }
    // Also watch the project root for new top-level .rs files
    watcher.watch(&project_root, RecursiveMode::NonRecursive)?;

    Ok(watcher)
}
