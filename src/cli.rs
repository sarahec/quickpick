//! Command-line parsing (clap derive).

use std::path::PathBuf;

use clap::Parser;
use serde::Serialize;

use crate::config::Vcs;

/// Command line for `qp`.
///
/// The `repo`, `source`, `onto`, `vcs` and `branch` fields double as the
/// clapfig CLI-override layer; the remaining fields are inputs or mode
/// selectors only and are ignored by [`clapfig`]'s schema matching.
#[derive(Parser, Serialize, Debug)]
#[command(
    name = "qp",
    version,
    about = "Plan path-scoped backports across git branches",
    long_about = "Plan path-scoped backports across git branches.\n\n\
        quickpick finds the commits on a source line that changed one \
        path, works out which of them can be cherry-picked onto a target \
        revision, and emits a self-contained shell script that performs \
        the plan (or, with --explain or --summary, describes it), for git \
        or jujutsu. Planning is strictly read-only: nothing in the \
        repository is modified."
)]
pub struct Cli {
    /// Repository-relative path to a file or directory.
    #[serde(skip_serializing)]
    #[arg(required_unless_present = "show_config")]
    pub path: Option<PathBuf>,
    /// Explain the planned steps in human-readable form instead of
    /// printing the script.
    #[serde(skip_serializing)]
    #[arg(long, conflicts_with = "summary")]
    pub explain: bool,
    /// Print a condensed plan instead of the script.
    #[serde(skip_serializing)]
    #[arg(long)]
    pub summary: bool,
    /// Commit range `a..b` on the source: `a` is exclusive and either side
    /// may be empty. Each bound is `branch`, a commit, or `branch@commit`.
    #[serde(skip_serializing)]
    #[arg(long, short = 'r', value_name = "A..B")]
    pub range: Option<String>,
    /// Target revision to bring the path onto (required; may come from
    /// QUICKPICK__ONTO or quickpick.toml instead). Accepts the same bound
    /// syntax as a range side.
    #[arg(long)]
    pub onto: Option<String>,
    /// Source line used for bare-commit bounds and the anchor search.
    #[arg(long)]
    pub source: Option<String>,
    /// Script dialect.
    #[arg(long, value_enum)]
    pub vcs: Option<Vcs>,
    /// Branch name for the git dialect (default `quickpick/<last path component>`).
    #[arg(long)]
    pub branch: Option<String>,
    /// Repository to operate on (default: current working directory).
    #[arg(long)]
    pub repo: Option<PathBuf>,
    /// Print the merged settings and exit.
    #[serde(skip_serializing)]
    #[arg(long)]
    pub show_config: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, error::ErrorKind};

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(args)
    }

    #[test]
    fn bare_path_selects_script_mode() {
        let cli = parse(&["qp", "pkgs/x"]).unwrap();
        assert!(!cli.explain && !cli.summary);
        assert_eq!(cli.path.as_deref(), Some("pkgs/x".as_ref()));
        assert!(cli.range.is_none());
    }

    #[test]
    fn explain_and_summary_conflict() {
        let err = parse(&["qp", "--explain", "--summary", "p"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn missing_path_is_a_usage_error() {
        let err = parse(&["qp", "--onto", "release-26.05"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn short_range_flag_parses_like_the_long_one() {
        for flag in ["--range", "-r"] {
            let cli = parse(&["qp", flag, "a..b", "p"]).unwrap();
            assert_eq!(cli.range.as_deref(), Some("a..b"), "{flag}");
        }
    }

    #[test]
    fn show_config_does_not_require_a_path() {
        let cli = parse(&["qp", "--show-config"]).unwrap();
        assert!(cli.show_config);
        assert!(cli.path.is_none());
    }

    #[test]
    fn help_lists_the_modes_and_options() {
        let help = Cli::command().render_long_help().to_string();
        for flag in [
            "--explain",
            "--summary",
            "--range",
            "--onto",
            "--vcs",
            "--show-config",
        ] {
            assert!(help.contains(flag), "help missing {flag}:\n{help}");
        }
        assert!(help.contains("Usage: qp"), "{help}");
    }
}
