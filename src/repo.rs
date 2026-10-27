//! Repository access via `git2`: opening and revision resolution.

use std::path::{Path, PathBuf};

use git2::{Oid, Repository};

use crate::error::Error;

/// Open the repository at `dir`, or the current working directory when
/// `dir` is `None`. Parent directories are searched for the repository root.
pub fn open(dir: Option<&Path>) -> Result<Repository, Error> {
    let dir: PathBuf = match dir {
        Some(d) => d.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|e| Error::Planning(format!("cannot determine the current directory: {e}")))?,
    };
    Repository::discover(&dir)
        .map_err(|_| Error::Planning(format!("{} is not a git repository", dir.display())))
}

/// Resolve `rev` to a commit `Oid`. `role` names the input (`--onto`,
/// `--source`, ...) in the error message.
pub fn resolve(repo: &Repository, rev: &str, role: &str) -> Result<Oid, Error> {
    repo.revparse_single(rev)
        .and_then(|o| o.peel_to_commit())
        .map(|c| c.id())
        .map_err(|_| Error::Planning(format!("{role} '{rev}' does not resolve to a commit")))
}
