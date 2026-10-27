//! quickpick: plan path-scoped backports across git branches and render them
//! as an explanation or a self-contained git/jujutsu shell script.

pub mod cli;
pub mod config;
pub mod error;
pub mod plan;
pub mod render;
pub mod repo;

use clap::Parser;
use cli::{Cli, Commands, PlanArgs};
use config::{Settings, Vcs};
use error::Error;
use git2::Repository;
use plan::{Plan, PlanRequest};

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
        Commands::Propose(args) => {
            let (_repo, plan, settings) = load_plan(&args)?;
            print!("{}", render::propose::render(&plan, settings.vcs));
            Ok(())
        }
        Commands::Generate(args) => {
            let (repo, plan, settings) = load_plan(&args)?;
            if plan.steps.is_empty() {
                eprintln!(
                    "qp: nothing to do: {} on {} already matches {}",
                    plan.path, plan.onto_rev, plan.source_rev
                );
                return Ok(());
            }
            let root = repo.workdir().ok_or_else(|| {
                Error::Planning("a working tree is required to generate a script".into())
            })?;
            let script = match settings.vcs {
                Vcs::Git => {
                    let branch = settings
                        .branch
                        .unwrap_or_else(|| render::git::default_branch(&plan.path));
                    render::git::render(&plan, &branch, root)
                }
                Vcs::Jujutsu => render::jj::render(&plan, root),
            };
            print!("{script}");
            Ok(())
        }
    }
}

/// Load settings, open the repository, compute the plan and print any
/// warnings to stderr.
fn load_plan(args: &PlanArgs) -> Result<(Repository, Plan, Settings), Error> {
    let settings = config::load(args)?;
    let repo = repo::open(settings.repo.as_deref())?;
    let path = args
        .path
        .to_str()
        .ok_or_else(|| Error::Usage(format!("'{}' is not valid UTF-8", args.path.display())))?;
    let req = PlanRequest {
        path: path.into(),
        source: settings.source.clone(),
        onto: settings.onto.clone().ok_or(Error::MissingOnto)?,
        from: args.from.clone(),
        to: args.to.clone(),
    };
    let plan = plan::plan(&repo, &req)?;
    for w in &plan.warnings {
        eprintln!("qp: warning: {w}");
    }
    Ok((repo, plan, settings))
}

/// Parse process argv into a [`Cli`].
pub fn parse() -> Cli {
    Cli::parse()
}
