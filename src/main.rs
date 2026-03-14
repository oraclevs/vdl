mod cli;
mod commands;
mod config;
mod downloader;
mod sandbox;
mod tui;

use clap::Parser;

fn main() {
    let _ = cli::Cli::parse();
}
