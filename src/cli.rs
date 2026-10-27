//! Command-line parsing (clap derive).

use clap::{Parser, Subcommand};
use clapfig::ConfigArgs;

/// Command line for `qp`.
#[derive(Parser, Debug)]
#[command(
    name = "qp",
    version,
    about = "Plan path-scoped backports across git branches"
)]
pub struct Cli {
    /// The subcommand to run.
    #[command(subcommand)]
    pub command: Commands,
}

/// The `qp` subcommands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Manage the configuration file (gen, get, set, list).
    Config(ConfigArgs),
}
