//! Command-line parsing (clap derive).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use clapfig::ConfigArgs;
use serde::Serialize;

use crate::config::Vcs;

/// Command line for `qp`.
#[derive(Parser, Debug)]
#[command(
    name = "qp",
    version,
    about = "Plan path-scoped backports across git branches",
    long_about = "Plan path-scoped backports across git branches.\n\n\
        quickpick finds the commits on a source revision that changed one \
        path, works out which of them can be cherry-picked onto a target \
        revision, and either explains the plan (propose) or emits a \
        self-contained shell script that performs it (generate), for git \
        or jujutsu. Planning is strictly read-only: nothing in the \
        repository is modified."
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
    #[command(
        visible_alias = "p",
        long_about = "Explain the planned steps in human-readable form: the \
            anchor, the commit range (from --from, inclusive, to --to), and \
            each PICK or SYNC step with its reason."
    )]
    Propose(PlanArgs),
    /// Print a self-contained shell script that performs the plan.
    #[command(
        visible_alias = "g",
        long_about = "Print a self-contained POSIX shell script that performs \
            the plan: cherry-picks for PICK steps and content-restoring \
            commits for SYNC steps, in the selected dialect (git or \
            jujutsu). Review it, then run it with `sh` inside the repository."
    )]
    Generate(PlanArgs),
    /// Manage the configuration file (gen, list, get, set, unset, schema).
    #[command(visible_aliases = ["c", "config"])]
    Configure(ConfigArgs),
}

/// Planning options shared by `propose` and `generate`.
///
/// The `repo`, `source`, `onto`, `vcs` and `branch` fields double as the
/// clapfig CLI-override layer; `from`, `to` and `path` are planning inputs
/// only and are ignored by [`clapfig`]'s schema matching.
#[derive(Args, Serialize, Debug)]
pub struct PlanArgs {
    /// Repository-relative path to a file or directory.
    #[serde(skip_serializing)]
    pub path: PathBuf,
    /// Target revision to bring the path onto (required; may come from
    /// QUICKPICK__ONTO or quickpick.toml instead).
    #[arg(long)]
    pub onto: Option<String>,
    /// Source revision.
    #[arg(long)]
    pub source: Option<String>,
    /// First source commit to copy (inclusive); defaults to just after the anchor.
    #[arg(long)]
    #[serde(skip_serializing)]
    pub from: Option<String>,
    /// Last source commit to copy; defaults to the newest commit touching the path.
    #[arg(long)]
    #[serde(skip_serializing)]
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
    use clap::{CommandFactory, error::ErrorKind};

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

    #[test]
    fn aliases_parse_to_the_same_commands() {
        for (full, short) in [
            ("propose", "p"),
            ("generate", "g"),
            ("configure", "c"),
            ("configure", "config"),
        ] {
            let args = match full {
                "configure" => vec!["qp", short, "list"],
                _ => vec!["qp", short, "--onto", "t", "p/x"],
            };
            let cli = parse(&args).unwrap_or_else(|e| panic!("{short}: {e}"));
            match (&cli.command, full) {
                (Commands::Propose(_), "propose")
                | (Commands::Generate(_), "generate")
                | (Commands::Configure(_), "configure") => {}
                (other, _) => panic!("alias {short} parsed to {other:?}, expected {full}"),
            }
        }
    }

    #[test]
    fn help_lists_the_aliases() {
        let help = Cli::command().render_long_help().to_string();
        for alias in ["p", "g", "c", "config"] {
            assert!(help.contains(alias), "help missing alias {alias}:\n{help}");
        }
        assert!(help.contains("Usage: qp"), "{help}");
    }
}
