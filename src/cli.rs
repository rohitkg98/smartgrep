use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "smartgrep", about = "Structural code navigation for agents")]
pub struct Cli {
    /// Output format: text or json
    #[arg(long, default_value = "text", global = true)]
    pub format: String,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Project root directory
    #[arg(long, global = true)]
    pub project_root: Option<PathBuf>,

    /// Use the background daemon for faster repeated queries (opt-in)
    #[arg(long, global = true)]
    pub daemon: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Structural summary of a file
    Context {
        /// Path to the file to analyze
        file: PathBuf,
    },
    /// List symbols (functions, structs, traits, interfaces, etc.)
    Ls {
        /// Symbol type to filter by
        symbol_type: Option<String>,
        /// Filter by file path substring (e.g. --in go/services/)
        #[arg(long = "in")]
        in_path: Option<String>,
    },
    /// Show detail for a named symbol
    Show {
        /// Symbol name
        name: String,
    },
    /// Show what a symbol depends on
    Deps {
        /// Symbol name
        name: String,
    },
    /// Show what references a symbol
    Refs {
        /// Symbol name
        name: String,
    },
    /// Force re-index
    Index,
    /// Run a composable query against the index
    Query {
        /// Query string (e.g. "structs where visibility = public | with fields")
        query: String,
    },
    /// Show the query log
    Log {
        /// Number of recent entries to show (default: 20)
        #[arg(long, default_value = "20")]
        limit: usize,
        /// Show summary statistics instead of recent entries
        #[arg(long)]
        stats: bool,
    },
    /// Internal: run the daemon server in the foreground (used by auto-start)
    #[command(hide = true)]
    RunServer {
        /// Idle timeout in seconds
        #[arg(long, default_value = "1800")]
        idle_timeout: u64,
    },
    /// Install the Claude Code skill so agents automatically use smartgrep
    InstallSkill {
        /// Install globally (~/.claude/skills/) instead of repo-local (.claude/skills/)
        #[arg(long)]
        global: bool,
    },
    /// Update smartgrep to the latest release
    Update,
}
