pub mod client;
pub mod logger;
pub mod protocol;
/// The daemon listens on a Unix domain socket; it only exists on Unix.
#[cfg(unix)]
pub mod server;

/// Entry point for the hidden `run-server` subcommand.
pub fn run_server_cmd(
    project_root: &Option<std::path::PathBuf>,
    idle_timeout: u64,
) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        server::run_server_cmd(project_root, idle_timeout)
    }
    #[cfg(not(unix))]
    {
        let _ = (project_root, idle_timeout);
        anyhow::bail!("daemon not supported on this platform")
    }
}
