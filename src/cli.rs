//! Command-line parsing (clap derive).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use clapfig::ConfigArgs;

use crate::config::Vcs;

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
    /// Explain the planned steps in human-readable form.
    Propose(PlanArgs),
    /// Print a self-contained shell script that performs the plan.
    Generate(PlanArgs),
    /// Manage the configuration file (gen, list, get, set, unset, schema).
    Configure(ConfigArgs),
}

/// Planning options shared by `propose` and `generate`.
#[derive(Args, Debug)]
pub struct PlanArgs {
    /// Repository-relative path to a file or directory.
    pub path: PathBuf,
    /// Target revision to bring the path onto (required unless configured).
    #[arg(long)]
    pub onto: Option<String>,
    /// Source revision.
    #[arg(long)]
    pub source: Option<String>,
    /// First source commit to copy (inclusive); defaults to just after the anchor.
    #[arg(long)]
    pub from: Option<String>,
    /// Last source commit to copy; defaults to the newest commit touching the path.
    #[arg(long)]
    pub to: Option<String>,
    /// Script dialect.
    #[arg(long, value_enum)]
    pub vcs: Option<Vcs>,
    /// Branch name for the git dialect (default `quickpick/<last path component>`).
    #[arg(long)]
    pub branch: Option<String>,
    /// Repository to operate on (default: current working directory).
    #[arg(long)]
    pub repo: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(args)
    }

    #[test]
    fn missing_subcommand_is_a_usage_error() {
        let err = parse(&["qp"]).unwrap_err();
        assert_eq!(
            err.kind(),
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
    }

    #[test]
    fn missing_path_is_a_usage_error() {
        let err = parse(&["qp", "propose", "--onto", "release-26.05"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    }
}
