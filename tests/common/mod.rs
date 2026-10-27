//! Fixture builder for integration tests: temporary git repositories with
//! commits, merges, branches and path deletions built directly with `git2`,
//! plus subprocess helpers for running `qp` and generated scripts.
//!
//! Shared across test crates, so unused helpers are expected per target.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use git2::{Oid, Repository, Signature, Time, Tree};
use tempfile::TempDir;

/// A temporary repository.
pub struct Fx {
    /// The tempdir holding the repository (kept alive for `repo`).
    pub dir: TempDir,
    /// The repository itself.
    pub repo: Repository,
}

/// One tree mutation applied to a commit's first-parent tree.
pub enum Op {
    /// Write `content` at `path` with the given file mode.
    Write {
        /// Repository-relative file path.
        path: &'static str,
        /// File content.
        content: &'static str,
        /// Git file mode (`0o100644` regular, `0o100755` executable).
        mode: i32,
    },
    /// Remove `path`.
    Delete {
        /// Repository-relative file or directory path.
        path: &'static str,
    },
}

/// Write a regular file.
pub fn w(path: &'static str, content: &'static str) -> Op {
    Op::Write {
        path,
        content,
        mode: 0o100644,
    }
}

/// Write an executable file.
pub fn wx(path: &'static str, content: &'static str) -> Op {
    Op::Write {
        path,
        content,
        mode: 0o100755,
    }
}

/// Delete a file or directory.
pub fn d(path: &'static str) -> Op {
    Op::Delete { path }
}

impl Fx {
    /// Init a fresh repository in a tempdir.
    pub fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        Self { dir, repo }
    }

    /// Create a commit applying `ops` on top of the first parent's tree.
    pub fn commit(&self, parents: &[Oid], ops: &[Op], message: &str) -> Oid {
        let base = parents
            .first()
            .map(|id| self.repo.find_commit(*id).unwrap().tree().unwrap());
        let mut map = match &base {
            Some(t) => load_tree(&self.repo, t),
            None => BTreeMap::new(),
        };
        for op in ops {
            apply(&self.repo, &mut map, op);
        }
        let tree_id = write_tree(&self.repo, &map).unwrap();
        let tree = self.repo.find_tree(tree_id).unwrap();
        let sig = Signature::new("Fixture", "f@x", &Time::new(1_700_000_000, 0)).unwrap();
        let parents: Vec<_> = parents
            .iter()
            .map(|id| self.repo.find_commit(*id).unwrap())
            .collect();
        let parent_refs: Vec<_> = parents.iter().collect();
        self.repo
            .commit(None, &sig, &sig, message, &tree, &parent_refs)
            .unwrap()
    }

    /// Create or reset branch `name` at `target`.
    pub fn branch(&self, name: &str, target: Oid) {
        self.repo
            .reference(&format!("refs/heads/{name}"), target, true, "fixture")
            .unwrap();
    }

    /// Resolve a revision to a commit id.
    pub fn oid(&self, rev: &str) -> Oid {
        self.repo
            .revparse_single(rev)
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .id()
    }
}

/// The working representation of a tree: named entries of blobs or subtrees.
enum Node {
    Blob(Oid, i32),
    Dir(BTreeMap<String, Node>),
}

fn load_tree(repo: &Repository, tree: &Tree) -> BTreeMap<String, Node> {
    let mut map = BTreeMap::new();
    for entry in tree.iter() {
        let name = entry.name().unwrap().to_string();
        let node = match entry.kind() {
            Some(git2::ObjectType::Tree) => Node::Dir(load_tree(
                repo,
                &entry.to_object(repo).unwrap().peel_to_tree().unwrap(),
            )),
            _ => Node::Blob(entry.id(), entry.filemode()),
        };
        map.insert(name, node);
    }
    map
}

fn apply(repo: &Repository, map: &mut BTreeMap<String, Node>, op: &Op) {
    let (path, value) = match op {
        Op::Write {
            path,
            content,
            mode,
        } => (path, Some((*content, *mode))),
        Op::Delete { path } => (path, None),
    };
    let components: Vec<&str> = Path::new(path)
        .components()
        .map(|c| c.as_os_str().to_str().unwrap())
        .collect();
    set(repo, map, &components, value);
}

fn set(
    repo: &Repository,
    map: &mut BTreeMap<String, Node>,
    components: &[&str],
    value: Option<(&str, i32)>,
) {
    let (name, rest) = components.split_first().unwrap();
    if rest.is_empty() {
        match value {
            Some((content, mode)) => {
                let blob = repo.blob(content.as_bytes()).unwrap();
                map.insert(name.to_string(), Node::Blob(blob, mode));
            }
            None => {
                map.remove(*name);
            }
        }
        return;
    }
    if value.is_none() {
        // A delete of a missing intermediate dir is a no-op; if the dir
        // exists and becomes empty, drop it.
        if let Some(Node::Dir(sub)) = map.get_mut(*name) {
            set(repo, sub, rest, None);
            if sub.is_empty() {
                map.remove(*name);
            }
        }
        return;
    }
    match map
        .entry(name.to_string())
        .or_insert_with(|| Node::Dir(BTreeMap::new()))
    {
        Node::Dir(sub) => set(repo, sub, rest, value),
        Node::Blob(_, _) => panic!("fixture path crosses a file entry"),
    }
}

fn write_tree(repo: &Repository, map: &BTreeMap<String, Node>) -> Option<Oid> {
    if map.is_empty() {
        return None;
    }
    let mut builder = repo.treebuilder(None).unwrap();
    for (name, node) in map {
        match node {
            Node::Blob(id, mode) => builder.insert(name, *id, *mode).unwrap(),
            Node::Dir(sub) => {
                let id = write_tree(repo, sub).unwrap();
                builder.insert(name, id, 0o040000).unwrap()
            }
        };
    }
    Some(builder.write().unwrap())
}

/// A standard two-branch history used by several tests:
///
/// ```text
///        mb ── ot                        (branch "onto", path = "v2")
///        |
///        └── c1 ── c2 ── c3             (branch "master")
/// ```
///
/// `mb` sets `pkg/file` to `"v1"`; `c1` bumps it to `"v2"` (the anchor
/// candidate: `onto` holds the same content); `c2`/`c3` take it to `"v3"`
/// then `"v4"`.
pub struct History {
    /// The two-branch fixture.
    pub fx: Fx,
    /// The merge base.
    pub mb: Oid,
    /// First source commit (`pkg/file` = `"v2"`, the anchor).
    pub c1: Oid,
    /// Second source commit (`pkg/file` = `"v3"`).
    pub c2: Oid,
    /// Tip of master (`pkg/file` = `"v4"`).
    pub c3: Oid,
    /// Tip of onto (`pkg/file` = `"v2"`).
    pub ot: Oid,
}

/// Build [`History`].
pub fn history() -> History {
    let fx = Fx::new();
    let mb = fx.commit(&[], &[w("pkg/file", "v1"), w("other", "o")], "base");
    let c1 = fx.commit(&[mb], &[w("pkg/file", "v2")], "pkg: v1 -> v2");
    let c2 = fx.commit(&[c1], &[w("pkg/file", "v3")], "pkg: v2 -> v3");
    let c3 = fx.commit(&[c2], &[w("pkg/file", "v4")], "pkg: v3 -> v4");
    let ot = fx.commit(&[mb], &[w("pkg/file", "v2"), w("target", "t")], "target");
    fx.branch("master", c3);
    fx.branch("onto", ot);
    History {
        fx,
        mb,
        c1,
        c2,
        c3,
        ot,
    }
}

/// A fixture whose plan for `pkg` is one merged SYNC (a conflicting commit
/// covered together with an outside-path commit) followed by a PICK:
///
/// ```text
///        mb ── ot                        (branch "onto", pkg/file = "vX")
///        |
///        └── c1 ── s1 ── s2 ── s3        (branch "master")
/// ```
///
/// `onto` diverged (`vX` where `c1` has `v2`), so anchor detection fails and
/// the plan needs `--from s1` — which also produces the from-mismatch
/// warning.
pub struct Mixed {
    /// The two-branch fixture.
    pub fx: Fx,
    /// The merge base.
    pub mb: Oid,
    /// Anchor parent; `pkg/file` = `"v2"`.
    pub c1: Oid,
    /// First commit of `--from`: an in-path change that conflicts (vX vs v3).
    pub s1: Oid,
    /// Second commit: in-path change plus a file outside `pkg`.
    pub s2: Oid,
    /// Master tip and `--to`: a clean in-path change that picks.
    pub s3: Oid,
}

/// Build [`Mixed`].
pub fn mixed() -> Mixed {
    let fx = Fx::new();
    let mb = fx.commit(&[], &[w("pkg/file", "v1"), w("other", "o")], "base");
    let c1 = fx.commit(&[mb], &[w("pkg/file", "v2")], "pkg: v1 -> v2");
    let s1 = fx.commit(&[c1], &[w("pkg/file", "v3")], "pkg: v2 -> v3");
    let s2 = fx.commit(
        &[s1],
        &[w("pkg/file", "v4"), w("other", "o2")],
        "pkg: v3 -> v4 + outside",
    );
    let s3 = fx.commit(&[s2], &[w("pkg/file", "v5")], "pkg: v4 -> v5");
    let ot = fx.commit(&[mb], &[w("pkg/file", "vX")], "target diverged");
    fx.branch("master", s3);
    fx.branch("onto", ot);
    Mixed {
        fx,
        mb,
        c1,
        s1,
        s2,
        s3,
    }
}

/// A `qp` command with a scrubbed environment — only PATH, HOME and an
/// empty XDG_CONFIG_HOME survive — plus the TempDir holding HOME. Tests
/// that need to seed `$XDG_CONFIG_HOME/quickpick/quickpick.toml` or env
/// vars build on this and add `.args()`/`.output()` themselves.
pub fn qp_cmd() -> (Command, TempDir) {
    let home = TempDir::new().unwrap();
    let cfg = home.path().join("cfg");
    std::fs::create_dir(&cfg).unwrap();
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_qp"));
    cmd.env_clear()
        .env("PATH", path)
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", &cfg);
    (cmd, home)
}

/// Run `qp` with a scrubbed environment: only PATH, HOME and an empty
/// XDG_CONFIG_HOME survive, so no user `quickpick.toml` or `QUICKPICK__*`
/// variable leaks in.
pub fn qp<S: AsRef<std::ffi::OsStr>>(args: &[S]) -> (Output, TempDir) {
    let (mut cmd, home) = qp_cmd();
    let out = cmd.args(args).output().unwrap();
    (out, home)
}

/// The `qp` arguments for a [`Mixed`] fixture: an explicit range starting
/// after `c1` (exclusive), which is the old inclusive `--from s1`.
pub fn mixed_args(m: &Mixed) -> Vec<String> {
    vec![
        "--repo".into(),
        m.fx.dir.path().to_string_lossy().into_owned(),
        "--onto".into(),
        "onto".into(),
        "--range".into(),
        format!("{}..", m.c1),
        "pkg".into(),
    ]
}

/// The `qp` arguments for a [`History`] fixture with the default range.
pub fn history_args(h: &History) -> Vec<String> {
    vec![
        "--repo".into(),
        h.fx.dir.path().to_string_lossy().into_owned(),
        "--onto".into(),
        "onto".into(),
        "pkg".into(),
    ]
}

/// `sh -n` syntax check; skipped when `sh` is not on PATH.
pub fn sh_check(script: &str) {
    let Ok(mut cmd) = Command::new("sh").arg("-n").stdin(Stdio::piped()).spawn() else {
        return;
    };
    cmd.stdin
        .as_mut()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    assert!(cmd.wait().unwrap().success(), "sh -n rejected:\n{script}");
}

/// Whether `name` resolves to an executable on PATH.
pub fn on_path(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Run `script` through `sh` with extra environment variables, in a
/// scrubbed environment.
pub fn run_script(script: &str, envs: &[(&str, &Path)]) -> Output {
    let home = TempDir::new().unwrap();
    let mut cmd = Command::new("sh");
    cmd.stdin(Stdio::piped())
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", home.path());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

pub fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

pub fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}
