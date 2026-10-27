//! Planning: range and anchor detection, step classification, sync merging
//! and plan verification.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use git2::{Commit, FileMode, Index, Oid, Repository, Sort, Tree};

use crate::error::Error;
use crate::repo;

/// A parsed `--range <a>..<b>`: each side is an optional bound spec.
///
/// `a` is the exclusive floor (`None` derives it from the anchor); `b` is
/// the inclusive range end (`None` means the newest source commit touching
/// the path).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Range {
    /// Left side (exclusive).
    pub start: Option<String>,
    /// Right side (inclusive end).
    pub end: Option<String>,
}

impl std::str::FromStr for Range {
    type Err = String;

    /// Parse `<a>..<b>`; either side may be empty but `..` is required.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || format!("malformed range '{s}': expected <a>..<b>");
        let Some((a, b)) = s.split_once("..") else {
            return Err(err());
        };
        if b.contains("..") {
            return Err(err());
        }
        let side = |s: &str| (!s.is_empty()).then(|| s.to_string());
        Ok(Range {
            start: side(a),
            end: side(b),
        })
    }
}

/// Inputs to [`plan`], after configuration resolution.
pub struct PlanRequest {
    /// Repository-relative path (file or directory) to bring across.
    pub path: String,
    /// Source line used to resolve an empty or bare-commit range end and
    /// to bound the anchor search.
    pub source: String,
    /// Target revision expression (a bound: branch, commit or
    /// `branch@commit`).
    pub onto: String,
    /// The requested `<a>..<b>`; an all-empty range uses the defaults.
    pub range: Range,
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

impl CommitInfo {
    fn of(repo: &Repository, id: Oid) -> Result<Self, Error> {
        let c = repo.find_commit(id)?;
        Ok(Self {
            id,
            summary: c
                .summary()
                .ok()
                .flatten()
                .map(str::to_string)
                .unwrap_or_default(),
            message: c.message().ok().map(str::to_string).unwrap_or_default(),
            author_time: c.time().seconds(),
            author_offset_minutes: c.time().offset_minutes(),
        })
    }
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
    /// The normalized repository-relative path.
    pub path: String,
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
    /// Last commit in the range (the resolved `b` end).
    pub to: Oid,
    /// The commit the plan continues from: the detected anchor, or the
    /// explicit range start `a`.
    pub anchor: Option<Oid>,
    /// Ordered steps.
    pub steps: Vec<Step>,
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
    let onto = repo::resolve_bound(repo, &req.onto, "--onto")?.oid;
    let target = repo::path_entry(repo, onto, &path)?;

    let mut entries = HashMap::new();
    let end = resolve_end(repo, req, &path, &mut entries)?;
    let to_entry = repo::path_entry(repo, end.to, &path)?;
    if to_entry == target {
        return Ok(Plan {
            path: path.to_string_lossy().into_owned(),
            source_rev: end.line_rev,
            onto_rev: req.onto.clone(),
            source: end.line,
            onto,
            from: None,
            to: end.to,
            anchor: None,
            steps: vec![],
            warnings: vec![],
            verified: true,
        });
    }

    let mut warnings = Vec::new();
    let floor = match &req.range.start {
        Some(spec) => explicit_start(repo, spec, target, &path, &mut warnings)?,
        None => detect_anchor(repo, onto, &end, target, &path, req, &mut entries)?,
    };

    let range = range_walk(repo, end.to, floor, &path, &mut entries)?;

    let mut sim = Simulation {
        repo,
        path: &path,
        state: repo.find_commit(onto)?.tree()?,
        warnings: &mut warnings,
        warned: BTreeSet::new(),
        steps: Vec::new(),
    };
    for id in &range {
        sim.step(repo.find_commit(*id)?)?;
    }
    let verified = verify(&mut sim, &to_entry)?;
    let steps = merge_syncs(std::mem::take(&mut sim.steps));

    Ok(Plan {
        path: path.to_string_lossy().into_owned(),
        source_rev: end.line_rev,
        onto_rev: req.onto.clone(),
        source: end.line,
        onto,
        from: range.first().copied(),
        to: end.to,
        anchor: Some(floor),
        steps,
        warnings,
        verified,
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

/// Whether `p` equals `path` or lives under it.
fn under(path: &Path, p: &Path) -> bool {
    p == path || p.starts_with(path)
}

/// Memoized `path_entry`: history walks revisit each commit and its parents
/// many times, so each commit's entry is computed at most once per `plan()`.
type Entries = HashMap<Oid, Option<(Oid, i32)>>;

fn cached_entry(
    repo: &Repository,
    cache: &mut Entries,
    commit: Oid,
    path: &Path,
) -> Result<Option<(Oid, i32)>, Error> {
    match cache.get(&commit) {
        Some(e) => Ok(*e),
        None => {
            let e = repo::path_entry(repo, commit, path)?;
            cache.insert(commit, e);
            Ok(e)
        }
    }
}

/// [`repo::touches`] against the [`Entries`] cache: the parent's entry is a
/// cache lookup, so an already-seen parent costs no tree descent.
fn touches_cached(
    repo: &Repository,
    cache: &mut Entries,
    commit: &Commit<'_>,
    path: &Path,
) -> Result<bool, Error> {
    let cur = cached_entry(repo, cache, commit.id(), path)?;
    let parent = if commit.parent_count() == 0 {
        None
    } else {
        cached_entry(repo, cache, commit.parent_id(0)?, path)?
    };
    Ok(cur != parent)
}

/// The resolved range end `b`, plus the source-line context it implies
/// for reporting.
struct End {
    /// The range-end commit.
    to: Oid,
    /// The line the range is drawn from: `b`'s named branch, or the
    /// `source` setting for an empty or bare-commit `b`.
    line_rev: String,
    /// `line_rev`'s resolved commit (the `b` bound's own oid for a named
    /// line, else the resolved `source`).
    line: Oid,
}

/// Resolve the range end `b`. A bound naming a branch supplies its own
/// line; an empty `b` is the newest non-merge commit touching `path` on
/// the `source` line, and a bare-commit `b` must be reachable from it.
fn resolve_end(
    repo: &Repository,
    req: &PlanRequest,
    path: &Path,
    entries: &mut Entries,
) -> Result<End, Error> {
    if let Some(spec) = &req.range.end {
        let b = repo::resolve_bound(repo, spec, "--range")?;
        if let Some(line) = b.line {
            return Ok(End {
                to: b.oid,
                line_rev: line,
                line: b.oid,
            });
        }
        let source = repo::resolve(repo, &req.source, "--source")?;
        if b.oid != source && !repo.graph_descendant_of(source, b.oid)? {
            return Err(Error::Planning(format!(
                "the range end '{spec}' is not a commit on the source '{}'",
                req.source
            )));
        }
        return Ok(End {
            to: b.oid,
            line_rev: req.source.clone(),
            line: source,
        });
    }
    let source = repo::resolve(repo, &req.source, "--source")?;
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    walk.push(source)?;
    for id in walk {
        let c = repo.find_commit(id?)?;
        if c.parent_count() > 1 {
            continue;
        }
        if touches_cached(repo, entries, &c, path)? {
            return Ok(End {
                to: c.id(),
                line_rev: req.source.clone(),
                line: source,
            });
        }
    }
    Err(Error::Planning(format!(
        "path '{}' is not found on the source '{}' (a shallow clone may be missing its history)",
        req.path, req.source
    )))
}

/// Handle an explicit range start `a`: warn when the path's content at
/// `a` differs from the target's and return `a` as the range floor.
/// `a` is exclusive and need not be an ancestor of `b`.
fn explicit_start(
    repo: &Repository,
    spec: &str,
    target: Option<(Oid, i32)>,
    path: &Path,
    warnings: &mut Vec<String>,
) -> Result<Oid, Error> {
    let a = repo::resolve_bound(repo, spec, "--range")?.oid;
    if repo::path_entry(repo, a, path)? != target {
        warnings.push(format!(
            "the range start '{spec}' does not continue the target's state: the path content at \
             {short} differs from the target; the plan may not apply cleanly",
            short = &a.to_string()[..7]
        ));
    }
    Ok(a)
}

/// Find the anchor: the newest path-touching commit reachable from `b`
/// whose content equals the target's, else the merge base when it matches.
fn detect_anchor(
    repo: &Repository,
    onto: Oid,
    end: &End,
    target: Option<(Oid, i32)>,
    path: &Path,
    req: &PlanRequest,
    entries: &mut Entries,
) -> Result<Oid, Error> {
    let mb = repo.merge_base(onto, end.to).map_err(|_| {
        Error::Planning(format!(
            "no merge base between '{}' and '{}'",
            req.onto, end.line_rev
        ))
    })?;
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    walk.push(end.to)?;
    walk.hide(mb)?;
    for id in walk {
        let id = id?;
        let c = repo.find_commit(id)?;
        if c.parent_count() > 1 || !touches_cached(repo, entries, &c, path)? {
            continue;
        }
        if cached_entry(repo, entries, id, path)? == target {
            return Ok(id);
        }
    }
    if repo::path_entry(repo, mb, path)? == target {
        return Ok(mb);
    }
    Err(Error::Planning(format!(
        "no common state found between '{}' and '{}' for '{}'; pass a range start like \
         '--range <a>..' to set the range floor (a shallow clone may be missing the shared \
         history)",
        req.onto, end.line_rev, req.path
    )))
}

/// Collect the range: non-merge commits reachable from `to` but not from
/// `floor` (exclusive), oldest first, that touch `path`.
fn range_walk(
    repo: &Repository,
    to: Oid,
    floor: Oid,
    path: &Path,
    entries: &mut Entries,
) -> Result<Vec<Oid>, Error> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;
    walk.push(to)?;
    walk.hide(floor)?;
    let mut range = Vec::new();
    for id in walk {
        let c = repo.find_commit(id?)?;
        if c.parent_count() > 1 || !touches_cached(repo, entries, &c, path)? {
            continue;
        }
        range.push(c.id());
    }
    Ok(range)
}

/// Mutable simulation state threaded through [`Simulation::step`].
struct Simulation<'a, 'b> {
    repo: &'a Repository,
    path: &'b Path,
    state: Tree<'a>,
    warnings: &'b mut Vec<String>,
    warned: BTreeSet<String>,
    steps: Vec<Step>,
}

impl Simulation<'_, '_> {
    /// Classify `commit` as PICK or SYNC and advance the simulated state.
    fn step(&mut self, commit: Commit<'_>) -> Result<(), Error> {
        let c_tree = commit.tree()?;
        let parent_tree = match commit.parent(0) {
            Ok(p) => p.tree()?,
            Err(e) if e.code() == git2::ErrorCode::NotFound => empty_tree(self.repo)?,
            Err(e) => return Err(e.into()),
        };
        let outside = self.outside_paths(&commit, &parent_tree, &c_tree)?;
        let info = CommitInfo::of(self.repo, commit.id())?;
        if !outside.is_empty() {
            return self.sync(info, Reason::OutsidePaths(outside));
        }
        // The commit only changes `<path>`; when `<path>` is a directory in
        // all three trees, merging the `<path>` subtrees is equivalent to
        // merging whole trees and much cheaper on big repos.
        let is_dir = |t: &Tree<'_>| -> Result<bool, Error> {
            Ok(match repo::tree_entry(t, self.path)? {
                Some((_, mode)) => mode == i32::from(FileMode::Tree),
                None => true,
            })
        };
        if is_dir(&parent_tree)? && is_dir(&self.state)? && is_dir(&c_tree)? {
            return self.pick_subtree(info, &parent_tree, &c_tree);
        }
        let mut index = self
            .repo
            .merge_trees(&parent_tree, &self.state, &c_tree, None)?;
        if index.has_conflicts() {
            return self.sync(info, Reason::Conflict(conflict_paths(&mut index)?));
        }
        self.state = self.repo.find_tree(index.write_tree_to(self.repo)?)?;
        self.steps.push(Step::Pick { commit: info });
        Ok(())
    }

    /// Paths changed by `commit` that lie outside the planned path; also
    /// records shared-definition-file warnings for `pkgs/top-level/`.
    fn outside_paths(
        &mut self,
        commit: &Commit<'_>,
        parent: &Tree<'_>,
        tree: &Tree<'_>,
    ) -> Result<Vec<String>, Error> {
        let mut outside = Vec::new();
        let diff = self
            .repo
            .diff_tree_to_tree(Some(parent), Some(tree), None)?;
        for delta in diff.deltas() {
            for file in [delta.old_file(), delta.new_file()] {
                let Some(p) = file.path() else { continue };
                let s = p.to_string_lossy().into_owned();
                if !under(self.path, p) {
                    outside.push(s.clone());
                }
                if s.starts_with("pkgs/top-level/")
                    && self.warned.insert(format!("{}:{s}", commit.id()))
                {
                    self.warnings.push(format!(
                        "{} also touches {s}, which quickpick does not copy; the target may need \
                         that change too",
                        &commit.id().to_string()[..7]
                    ));
                }
            }
        }
        outside.sort();
        outside.dedup();
        Ok(outside)
    }

    /// Merge the `<path>` subtrees of `parent`/`state`/`commit` and graft the
    /// result back into `state` — valid because the commit changes nothing
    /// outside `<path>`.
    fn pick_subtree(
        &mut self,
        info: CommitInfo,
        parent_tree: &Tree<'_>,
        c_tree: &Tree<'_>,
    ) -> Result<(), Error> {
        let sub = |t: &Tree<'_>| -> Result<Tree<'_>, Error> {
            match repo::tree_entry(t, self.path)? {
                Some((id, _)) => Ok(self.repo.find_tree(id)?),
                None => empty_tree(self.repo),
            }
        };
        let mut index =
            self.repo
                .merge_trees(&sub(parent_tree)?, &sub(&self.state)?, &sub(c_tree)?, None)?;
        if index.has_conflicts() {
            return self.sync(info, Reason::Conflict(self.conflict_paths(&mut index)?));
        }
        let merged = index.write_tree_to(self.repo)?;
        let entry = if self.repo.find_tree(merged)?.is_empty() {
            None
        } else {
            Some((merged, i32::from(FileMode::Tree)))
        };
        self.state = replace_entry(self.repo, &self.state, self.path, entry)?;
        self.steps.push(Step::Pick { commit: info });
        Ok(())
    }

    /// Conflicting paths in the subtree merge index, reported relative to
    /// the repository root (the index is rooted at `<path>`).
    fn conflict_paths(&self, index: &mut Index) -> Result<Vec<String>, Error> {
        Ok(conflict_paths(index)?
            .into_iter()
            .map(|p| self.path.join(p).to_string_lossy().into_owned())
            .collect())
    }

    /// Record a SYNC step and replace the path in the simulated state.
    fn sync(&mut self, info: CommitInfo, reason: Reason) -> Result<(), Error> {
        let entry = repo::path_entry(self.repo, info.id, self.path)?;
        self.state = replace_entry(self.repo, &self.state, self.path, entry)?;
        self.steps.push(Step::Sync {
            removes_path: entry.is_none(),
            covered: vec![(info.clone(), reason)],
            target: info,
        });
        Ok(())
    }
}

/// Conflicting paths in a merge index.
fn conflict_paths(index: &mut Index) -> Result<Vec<String>, Error> {
    let mut paths = Vec::new();
    for conflict in index.conflicts()? {
        let conflict = conflict?;
        for entry in [conflict.our, conflict.their, conflict.ancestor]
            .into_iter()
            .flatten()
        {
            paths.push(String::from_utf8_lossy(&entry.path).into_owned());
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// The empty tree, used as the merge ancestor for root commits.
fn empty_tree(repo: &Repository) -> Result<Tree<'_>, Error> {
    let id = repo.treebuilder(None)?.write()?;
    Ok(repo.find_tree(id)?)
}

/// Rebuild `tree` with the entry at `path` set to `entry` (or removed when
/// `entry` is `None`, pruning parent trees that become empty).
fn replace_entry<'a>(
    repo: &'a Repository,
    tree: &Tree<'a>,
    path: &Path,
    entry: Option<(Oid, i32)>,
) -> Result<Tree<'a>, Error> {
    let components: Vec<String> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str().map(str::to_string))
        .collect();
    let id = match set_component(repo, Some(tree), &components, entry)? {
        Some(id) => id,
        None => repo.treebuilder(None)?.write()?,
    };
    Ok(repo.find_tree(id)?)
}

/// Recursive worker for [`replace_entry`]: returns the new subtree id, or
/// `None` when the subtree became empty and should be dropped by its parent.
fn set_component(
    repo: &Repository,
    tree: Option<&Tree<'_>>,
    components: &[String],
    entry: Option<(Oid, i32)>,
) -> Result<Option<Oid>, Error> {
    let Some((name, rest)) = components.split_first() else {
        return Ok(tree.map(Tree::id));
    };
    let mut builder = repo.treebuilder(tree)?;
    if rest.is_empty() {
        match entry {
            Some((id, mode)) => {
                builder.insert(name, id, mode)?;
            }
            None => builder.remove(name)?,
        }
    } else {
        let sub = match tree.and_then(|t| t.get_name(name)) {
            Some(e) if e.kind() == Some(git2::ObjectType::Tree) => {
                Some(e.to_object(repo)?.peel_to_tree()?)
            }
            _ => None,
        };
        match set_component(repo, sub.as_ref(), rest, entry)? {
            Some(id) => {
                builder.insert(name, id, i32::from(FileMode::Tree))?;
            }
            None => builder.remove(name)?,
        };
    }
    if builder.is_empty() {
        return Ok(None);
    }
    Ok(Some(builder.write()?))
}

/// Merge adjacent SYNC steps: keep the last covered commit as the target and
/// every covered commit with its reason, in order.
fn merge_syncs(steps: Vec<Step>) -> Vec<Step> {
    let mut merged: Vec<Step> = Vec::with_capacity(steps.len());
    for step in steps {
        match (merged.last_mut(), step) {
            (
                Some(Step::Sync {
                    target,
                    covered,
                    removes_path,
                }),
                Step::Sync {
                    target: new_target,
                    covered: new_covered,
                    removes_path: new_removes,
                },
            ) => {
                *target = new_target;
                *removes_path = new_removes;
                covered.extend(new_covered);
            }
            (_, step) => merged.push(step),
        }
    }
    merged
}

/// Verify the simulated state against `to`'s path content.
fn verify(sim: &mut Simulation<'_, '_>, to_entry: &Option<(Oid, i32)>) -> Result<bool, Error> {
    let state_entry = repo::tree_entry(&sim.state, sim.path)?;
    if &state_entry == to_entry {
        return Ok(true);
    }
    let differing = differing_files(sim.repo, &sim.state, sim.path, *to_entry)?;
    Err(Error::Planning(format!(
        "verification failed: the simulated result differs from the path at the range end \
         ({}); a merge commit that changes the path itself is a likely cause — \
         try a narrower --range",
        differing.join(", ")
    )))
}

/// List the paths under `path` whose content differs between the simulated
/// state tree and `to`'s tree (or a single "path present/absent" entry).
fn differing_files(
    repo: &Repository,
    state: &Tree<'_>,
    path: &Path,
    to_entry: Option<(Oid, i32)>,
) -> Result<Vec<String>, Error> {
    let prefix = path.to_string_lossy().into_owned();
    let state_sub = match repo::tree_entry(state, path)? {
        Some((id, mode)) if mode == i32::from(FileMode::Tree) => Some(repo.find_tree(id)?),
        _ => None,
    };
    let to_sub = match to_entry {
        Some((id, mode)) if mode == i32::from(FileMode::Tree) => Some(repo.find_tree(id)?),
        _ => None,
    };
    if state_sub.is_none() || to_sub.is_none() {
        return Ok(vec![format!(
            "{prefix} (present on one side only or blob differs)"
        )]);
    }
    let mut files = Vec::new();
    let diff = repo.diff_tree_to_tree(state_sub.as_ref(), to_sub.as_ref(), None)?;
    for delta in diff.deltas() {
        if let Some(p) = delta.new_file().path().or_else(|| delta.old_file().path()) {
            files.push(format!("{prefix}/{}", p.to_string_lossy()));
        }
        if files.len() >= 20 {
            files.push("...".to_string());
            break;
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Signature, Time};

    #[test]
    fn range_parses_both_sides_either_empty() {
        use std::str::FromStr;
        let cases = [
            ("a..b", Some("a"), Some("b")),
            ("a..", Some("a"), None),
            ("..b", None, Some("b")),
            ("..", None, None),
        ];
        for (spec, start, end) in cases {
            let r = Range::from_str(spec).unwrap_or_else(|e| panic!("{spec}: {e}"));
            assert_eq!(r.start.as_deref(), start, "{spec} start");
            assert_eq!(r.end.as_deref(), end, "{spec} end");
        }
    }

    #[test]
    fn range_without_dots_is_rejected() {
        use std::str::FromStr;
        for bad in ["ab", "a", "a..b..c"] {
            let err = Range::from_str(bad).unwrap_err();
            assert!(err.contains("expected <a>..<b>"), "{bad}: {err}");
        }
    }

    /// `touches_cached` must agree with the simple `repo::touches` definition
    /// on file, directory, absent-path and filemode-only cases.
    #[test]
    fn cached_touches_matches_touches() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let sig = Signature::new("t", "t@x", &Time::new(1_700_000_000, 0)).unwrap();
        let commit = |parents: &[Oid], files: &[(&str, &str, i32)]| -> Oid {
            let mut b = repo.treebuilder(None).unwrap();
            for (name, content, mode) in files {
                let id = repo.blob(content.as_bytes()).unwrap();
                b.insert(name, id, *mode).unwrap();
            }
            let tree = b.write().unwrap();
            let parents: Vec<Commit> = parents
                .iter()
                .map(|id| repo.find_commit(*id).unwrap())
                .collect();
            let refs: Vec<&Commit> = parents.iter().collect();
            repo.commit(None, &sig, &sig, "c", &repo.find_tree(tree).unwrap(), &refs)
                .unwrap()
        };
        let base = commit(&[], &[("f", "x", i32::from(FileMode::Blob))]);
        let changed = commit(&[base], &[("f", "y", i32::from(FileMode::Blob))]);
        let mode = commit(
            &[changed],
            &[("f", "x", i32::from(FileMode::BlobExecutable))],
        );
        let gone = commit(&[mode], &[]);
        // The cache keys on the commit id alone, so each planned path gets
        // its own pass — as in `plan()`. Walking the chain newest-first
        // exercises cache hits on already-seen parents.
        for path in ["f", "missing"] {
            let mut cache = Entries::new();
            for id in [gone, mode, changed, base] {
                let c = repo.find_commit(id).unwrap();
                assert_eq!(
                    repo::touches(&c, Path::new(path)).unwrap(),
                    touches_cached(&repo, &mut cache, &c, Path::new(path)).unwrap(),
                    "{id} {path}"
                );
            }
        }
    }
}
