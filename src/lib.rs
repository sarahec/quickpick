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
use config::{Settings, Vcs};
use error::Error;
use git2::Repository;
use plan::{Plan, PlanRequest};

/// Parse argv and run the selected output mode.
///
/// Usage errors are reported by clap itself (exit code 2); everything else is
/// returned as an [`Error`] for the caller to print and map to an exit code.
pub fn run(cli: Cli) -> Result<(), Error> {
    if cli.show_config {
        let settings = config::merged(&cli)?;
        print!("{}", config::show(&settings));
        return Ok(());
    }
    let (repo, plan, settings) = load_plan(&cli)?;
    if cli.explain {
        print!("{}", render::explain::render(&plan, settings.vcs));
        return Ok(());
    }
    if cli.summary {
        print!("{}", render::summary::render(&plan, settings.vcs));
        return Ok(());
    }
    if plan.steps.is_empty() {
        eprintln!(
            "qp: nothing to do: {} on {} already matches {}",
            plan.path, plan.onto_rev, plan.source_rev
        );
        return Ok(());
    }
    let root = repo
        .workdir()
        .ok_or_else(|| Error::Planning("a working tree is required to generate a script".into()))?;
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

/// Load settings, open the repository, compute the plan and print any
/// warnings to stderr.
fn load_plan(cli: &Cli) -> Result<(Repository, Plan, Settings), Error> {
    let settings = config::load(cli)?;
    let repo = repo::open(settings.repo.as_deref())?;
    let path = cli
        .path
        .as_ref()
        .ok_or_else(|| Error::Usage("a path is required".into()))?
        .to_str()
        .ok_or_else(|| Error::Usage("path is not valid UTF-8".into()))?;
    let req = PlanRequest {
        path: path.into(),
        source: settings.source.clone(),
        onto: settings.onto.clone().ok_or(Error::MissingOnto)?,
        range: cli.range.clone().unwrap_or_default(),
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
