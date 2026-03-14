mod cli;
mod commands;
mod config;
mod downloader;
mod sandbox;
mod tui;

use std::io::Write;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use colored::Colorize;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{} {}", "Error:".red().bold(), e);
        for cause in e.chain().skip(1) {
            eprintln!("  {}", cause);
        }
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = cli::Cli::parse();

    match cli.command {
        Some(cli::Commands::Yt(args)) => commands::yt::run(args).await,
        Some(cli::Commands::Tk(args)) => commands::tk::run(args).await,
        Some(cli::Commands::Ig(args)) => commands::ig::run(args).await,
        Some(cli::Commands::Tw(args)) => commands::tw::run(args).await,
        Some(cli::Commands::Sp(args)) => commands::sp::run(args).await,
        Some(cli::Commands::Update) => commands::update::run().await,
        Some(cli::Commands::Config) => commands::config_cmd::run().await,
        None => {
            if config::Config::ensure_exists()? {
                let path = config::config_path().context("Failed to resolve vdl config path")?;
                tui::print_first_run(&commands::display_path(&path));
                return Ok(());
            }

            let mut command = cli::Cli::command();
            command.print_help().context("Failed to render CLI help")?;
            std::io::stdout()
                .write_all(b"\n")
                .context("Failed to write CLI help footer")?;
            Ok(())
        }
    }
}
