use clap::Parser;

use smartgrep::cli::{Cli, Command};
use smartgrep::commands;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Context { file } => {
            commands::context::run(&file, &cli.format)?;
        }
        Command::Ls { symbol_type } => {
            commands::ls::run(&symbol_type, &cli.format, &cli.project_root)?;
        }
        Command::Show { name } => {
            commands::show::run(&name, &cli.format, &cli.project_root)?;
        }
        Command::Deps { name } => {
            commands::deps::run(&name, &cli.format, &cli.project_root)?;
        }
        Command::Refs { name } => {
            commands::refs::run(&name, &cli.format, &cli.project_root)?;
        }
        Command::Index => {
            commands::index_cmd::run(&cli.project_root)?;
        }
        Command::Query { query } => {
            commands::query::run(&query, &cli.format, &cli.project_root)?;
        }
    }

    Ok(())
}
