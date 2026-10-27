//! Repository access via `git2`: opening, revision resolution and
//! path-entry lookup against the object database.

use std::path::{Path, PathBuf};

use git2::{Commit, Oid, Repository, Tree};

use crate::error::Error;

impl From<git2::Error> for Error {
    fn from(e: git2::Error) -> Self {
        Self::Planning(e.message().to_string())
    }
}

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
    try_commit(repo, rev)
        .map_err(|_| Error::Planning(format!("{role} '{rev}' does not resolve to a commit")))
}

/// `rev` resolved to a commit `Oid`, or a git2 error.
fn try_commit(repo: &Repository, rev: &str) -> Result<Oid, git2::Error> {
    repo.revparse_single(rev)
        .and_then(|o| o.peel_to_commit())
        .map(|c| c.id())
}

/// A resolved bound: the commit a `--range` side or `--onto` points at,
/// plus the branch line it names, when the spec names one.
#[derive(Clone, Debug)]
pub struct Bound {
    /// The resolved commit.
    pub oid: Oid,
    /// The branch line the bound names (`branch` and `branch@commit`
    /// forms; `None` for a bare commit or another revision expression).
    pub line: Option<String>,
}

/// Resolve a bound spec of the form `branch|commit|branch@commit`.
///
/// The whole spec is tried as a revision first, so a refname that itself
/// contains `@` (which git permits) wins over the pin interpretation.
/// When that fails, the spec splits at its last `@`: the right side must
/// resolve to a commit and the left to a branch, and the commit must be
/// reachable from the branch. `role` names the input in error messages.
pub fn resolve_bound(repo: &Repository, spec: &str, role: &str) -> Result<Bound, Error> {
    if let Ok(oid) = try_commit(repo, spec) {
        return Ok(Bound {
            oid,
            line: branch_line(repo, spec),
        });
    }
    let unresolvable = || Error::Planning(format!("{role} '{spec}' does not resolve to a commit"));
    let Some((left, right)) = spec.rsplit_once('@') else {
        return Err(unresolvable());
    };
    if left.is_empty() || right.is_empty() {
        return Err(unresolvable());
    }
    let commit = try_commit(repo, right).map_err(|_| {
        Error::Planning(format!(
            "{role} '{spec}': '{right}' does not resolve to a commit"
        ))
    })?;
    let tip = branch_tip(repo, left)
        .ok_or_else(|| Error::Planning(format!("{role} '{spec}': '{left}' is not a branch")))?;
    if commit != tip && !repo.graph_descendant_of(tip, commit)? {
        return Err(Error::Planning(format!(
            "{role} '{spec}': {right} is not on {left}"
        )));
    }
    Ok(Bound {
        oid: commit,
        line: Some(left.to_string()),
    })
}

/// `spec` as a branch name (local or remote-tracking), when it is one.
fn branch_line(repo: &Repository, spec: &str) -> Option<String> {
    branch_tip(repo, spec).map(|_| spec.to_string())
}

/// The tip commit of the branch named `spec` (local or remote-tracking).
fn branch_tip(repo: &Repository, spec: &str) -> Option<Oid> {
    for bt in [git2::BranchType::Local, git2::BranchType::Remote] {
        if let Ok(b) = repo.find_branch(spec, bt)
            && let Ok(c) = b.get().peel_to_commit()
        {
            return Some(c.id());
        }
    }
    None
}

/// Install an in-memory object-database backend at the highest priority so
/// that objects written during planning never touch `.git/objects`.
pub fn install_mempack(repo: &Repository) -> Result<(), Error> {
    let odb = repo
        .odb()
        .map_err(|e| Error::Planning(format!("cannot open the object database: {e}")))?;
    // The odb owns the backend once registered; the handle may be dropped.
    odb.add_new_mempack_backend(1000)
        .map_err(|e| Error::Planning(format!("cannot install in-memory object database: {e}")))?;
    Ok(())
}

/// The `(entry id, filemode)` of `path` at `commit`, or `None` when the path
/// does not exist there. The filemode is part of the identity, so a
/// mode-only change counts as a change.
pub fn path_entry(
    repo: &Repository,
    commit: Oid,
    path: &Path,
) -> Result<Option<(Oid, i32)>, Error> {
    let tree = repo.find_commit(commit)?.tree()?;
    tree_entry(&tree, path)
}

/// The `(entry id, filemode)` of `path` in `tree`, or `None` when absent.
pub fn tree_entry(tree: &Tree, path: &Path) -> Result<Option<(Oid, i32)>, Error> {
    match tree.get_path(path) {
        Ok(e) => Ok(Some((e.id(), e.filemode()))),
        Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Whether `commit` changes `path` relative to its first parent. A missing
/// parent counts as an absent path.
pub fn touches(commit: &Commit<'_>, path: &Path) -> Result<bool, Error> {
    let cur = tree_entry(&commit.tree()?, path)?;
    let parent = match commit.parent(0) {
        Ok(p) => tree_entry(&p.tree()?, path)?,
        Err(e) if e.code() == git2::ErrorCode::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    Ok(cur != parent)
}
