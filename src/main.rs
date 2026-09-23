use clap::Parser;

use smartgrep::cli::{Cli, Command};
use smartgrep::commands;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Context { file } => {
            commands::context::run(&file, &cli.format, cli.daemon)?;
        }
        Command::Ls { symbol_type, in_path } => {
            commands::ls::run(&symbol_type, &in_path, &cli.format, &cli.project_root, cli.daemon)?;
        }
        Command::Show { name } => {
            commands::show::run(&name, &cli.format, &cli.project_root, cli.daemon)?;
        }
        Command::Deps { name } => {
            commands::deps::run(&name, &cli.format, &cli.project_root, cli.daemon)?;
        }
        Command::Refs { name } => {
            commands::refs::run(&name, &cli.format, &cli.project_root, cli.daemon)?;
        }
        Command::Index => {
            commands::index_cmd::run(&cli.project_root)?;
        }
        Command::Query { query } => {
            commands::query::run(&query, &cli.format, &cli.project_root, cli.daemon)?;
        }
        Command::Log { limit, stats } => {
            commands::log_cmd::run(limit, stats, &cli.project_root)?;
        }
        Command::RunServer { idle_timeout } => {
            smartgrep::daemon::run_server_cmd(&cli.project_root, idle_timeout)?;
        }
        Command::InstallSkill { global } => {
            commands::install_skill::run(global)?;
        }
        Command::Init { agents_md, no_skill, dry_run } => {
            let opts = commands::init::InitOptions { agents_md, no_skill, dry_run };
            commands::init::run(&cli.project_root, &opts)?;
        }
        Command::Update => {
            commands::update::run()?;
        }
        Command::Map { in_path, all, depth, include_generated, symbols } => {
            commands::map::run(&in_path, all, depth, include_generated, symbols, &cli.format, &cli.project_root)?;
        }
    }

    Ok(())
}
