//! quickpick: plan path-scoped backports across git branches and render them
//! as an explanation or a self-contained git/jujutsu shell script.

pub mod cli;
pub mod config;
pub mod error;
pub mod plan;
pub mod render;
pub mod repo;

use clap::Parser;
use cli::Cli;

/// Run the selected subcommand.
pub fn run(cli: Cli) {
    match cli.command {
        cli::Commands::Config(_) => {}
    }
}

/// Parse process argv into a [`Cli`].
pub fn parse() -> Cli {
    Cli::parse()
}
