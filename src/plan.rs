//! Planning: range and anchor detection, step classification, sync merging
//! and plan verification.

use std::path::{Path, PathBuf};

use git2::{Oid, Repository, Sort};

use crate::error::Error;
use crate::repo;

/// Inputs to [`plan`], after configuration resolution.
pub struct PlanRequest {
    /// Repository-relative path (file or directory) to bring across.
    pub path: String,
    /// Source revision expression.
    pub source: String,
    /// Target revision expression.
    pub onto: String,
    /// Optional inclusive range start.
    pub from: Option<String>,
    /// Optional range end; defaults to the newest source commit touching `path`.
    pub to: Option<String>,
}

/// Owned commit metadata needed by the plan renderers.
#[derive(Clone, Debug)]
pub struct CommitInfo {
    /// Commit id.
    pub id: Oid,
    /// First line of the commit message.
    pub summary: String,
    /// Full commit message.
    pub message: String,
    /// Author time, seconds since the epoch.
    pub author_time: i64,
    /// Author timezone offset in minutes.
    pub author_offset_minutes: i32,
}

/// Why a commit could not be cherry-picked.
#[derive(Debug)]
pub enum Reason {
    /// The commit also touches files outside the path.
    OutsidePaths(Vec<String>),
    /// The commit would conflict with the simulated state.
    Conflict(Vec<String>),
}

/// One ordered step of a [`Plan`].
#[derive(Debug)]
pub enum Step {
    /// Cherry-pick the commit.
    Pick {
        /// The commit to pick.
        commit: CommitInfo,
    },
    /// Set the path to its content at `target`, covering several commits.
    Sync {
        /// The commit whose path content the sync restores.
        target: CommitInfo,
        /// Every covered commit, oldest first, with its reason.
        covered: Vec<(CommitInfo, Reason)>,
        /// Whether the sync removes the path.
        removes_path: bool,
    },
}

/// A computed plan, owned and free of `git2` lifetimes.
#[derive(Debug)]
pub struct Plan {
    /// Source revision expression as requested.
    pub source_rev: String,
    /// Target revision expression as requested.
    pub onto_rev: String,
    /// Resolved source commit.
    pub source: Oid,
    /// Resolved target commit.
    pub onto: Oid,
    /// First commit in the range (`None` when nothing needs doing).
    pub from: Option<Oid>,
    /// Last commit in the range.
    pub to: Oid,
    /// Detected anchor commit, or the explicit `--from`'s first parent.
    pub anchor: Option<Oid>,
    /// Ordered steps.
    pub steps: Vec<Step>,
    /// The path-touching commits of the range, oldest first, before
    /// classification into steps.
    pub range: Vec<Oid>,
    /// Warnings to print on stderr.
    pub warnings: Vec<String>,
    /// Whether the simulated result matches the path at `to`.
    pub verified: bool,
}

/// Compute the plan for `req` against `repo`.
///
/// Installs the in-memory object backend first, so the repository is never
/// modified — simulation writes stay in memory.
pub fn plan(repo: &Repository, req: &PlanRequest) -> Result<Plan, Error> {
    repo::install_mempack(repo)?;
    let path = normalize_path(&req.path)?;
    let onto = repo::resolve(repo, &req.onto, "--onto")?;
    let source = repo::resolve(repo, &req.source, "--source")?;
    let target = repo::path_entry(repo, onto, &path)?;

    let to = resolve_to(repo, req, source, &path)?;
    let to_entry = repo::path_entry(repo, to, &path)?;
    if to_entry == target {
        return Ok(Plan {
            source_rev: req.source.clone(),
            onto_rev: req.onto.clone(),
            source,
            onto,
            from: None,
            to,
            anchor: None,
            steps: vec![],
            range: vec![],
            warnings: vec![],
            verified: true,
        });
    }

    let mb = repo.merge_base(onto, to).map_err(|_| {
        Error::Planning(format!(
            "no merge base between '{}' and '{}'",
            req.onto, req.source
        ))
    })?;

    let warnings = Vec::new();
    if req.from.is_some() {
        return Err(Error::Planning(
            "explicit --from is not supported yet".into(),
        ));
    }
    let anchor = detect_anchor(repo, to, mb, target, &path, req)?;

    let range = range_walk(repo, to, anchor, None, &path)?;

    // `from` is the first commit of the range for reporting purposes.
    let from_oid = range.first().copied();
    Ok(Plan {
        source_rev: req.source.clone(),
        onto_rev: req.onto.clone(),
        source,
        onto,
        from: from_oid,
        to,
        anchor: Some(anchor),
        steps: vec![],
        range,
        warnings,
        verified: false,
    })
}

/// Normalize a repository-relative path: require UTF-8, relative, no `..`,
/// and strip a trailing slash.
fn normalize_path(raw: &str) -> Result<PathBuf, Error> {
    let path = PathBuf::from(raw.strip_suffix('/').unwrap_or(raw));
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(Error::Usage(format!(
            "'{raw}' is not a repository-relative path"
        )));
    }
    Ok(path)
}

/// Resolve `--to`, or find the newest non-merge source commit touching `path`.
fn resolve_to(
    repo: &Repository,
    req: &PlanRequest,
    source: Oid,
    path: &Path,
) -> Result<Oid, Error> {
    if let Some(rev) = &req.to {
        return repo::resolve(repo, rev, "--to");
    }
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    walk.push(source)?;
    for id in walk {
        let c = repo.find_commit(id?)?;
        if c.parent_count() > 1 {
            continue;
        }
        if repo::touches(&c, path)? {
            return Ok(c.id());
        }
    }
    Err(Error::Planning(format!(
        "path '{}' is not found on the source '{}' (a shallow clone may be missing its history)",
        req.path, req.source
    )))
}

/// Find the anchor: the newest path-touching candidate whose content equals
/// the target's, else the merge base when it matches.
fn detect_anchor(
    repo: &Repository,
    to: Oid,
    mb: Oid,
    target: Option<(Oid, i32)>,
    path: &Path,
    req: &PlanRequest,
) -> Result<Oid, Error> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    walk.push(to)?;
    walk.hide(mb)?;
    for id in walk {
        let id = id?;
        let c = repo.find_commit(id)?;
        if c.parent_count() > 1 || !repo::touches(&c, path)? {
            continue;
        }
        if repo::path_entry(repo, id, path)? == target {
            return Ok(id);
        }
    }
    if repo::path_entry(repo, mb, path)? == target {
        return Ok(mb);
    }
    Err(Error::Planning(format!(
        "no common state found between '{}' and '{}' for '{}'; pass --from to set the range \
         start (a shallow clone may be missing the shared history)",
        req.onto, req.source, req.path
    )))
}

/// Collect the range: non-merge commits that touch `path`, oldest first.
/// The walk hides the anchor (or `--from`'s parents) and everything behind it.
fn range_walk(
    repo: &Repository,
    to: Oid,
    anchor: Oid,
    from: Option<Oid>,
    path: &Path,
) -> Result<Vec<Oid>, Error> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;
    walk.push(to)?;
    match from {
        Some(from) => {
            for i in 0..repo.find_commit(from)?.parent_count() {
                walk.hide(repo.find_commit(from)?.parent_id(i)?)?;
            }
        }
        None => walk.hide(anchor)?,
    }
    let mut range = Vec::new();
    for id in walk {
        let c = repo.find_commit(id?)?;
        if c.parent_count() > 1 || !repo::touches(&c, path)? {
            continue;
        }
        range.push(c.id());
    }
    Ok(range)
}
