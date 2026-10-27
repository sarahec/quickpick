//! quickpick: plan path-scoped backports across git branches and render them
//! as an explanation or a self-contained git/jujutsu shell script.

pub mod cli;
pub mod config;
pub mod error;
pub mod plan;
pub mod render;
pub mod repo;

use clap::Parser;
use cli::{Cli, Commands};
use error::Error;

/// Parse argv and run the selected subcommand.
///
/// Usage errors are reported by clap itself (exit code 2); everything else is
/// returned as an [`Error`] for the caller to print and map to an exit code.
pub fn run(cli: Cli) -> Result<(), Error> {
    match cli.command {
        Commands::Configure(args) => {
            let action = args.into_action();
            config::builder()
                .handle_and_print(&action)
                .map_err(|e| Error::Config(e.to_string()))
        }
        Commands::Propose(args) | Commands::Generate(args) => {
            let _settings = config::load(&args)?;
            Ok(())
        }
    }
}

/// Parse process argv into a [`Cli`].
pub fn parse() -> Cli {
    Cli::parse()
}
