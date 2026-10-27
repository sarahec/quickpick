//! Tests for repository opening, revision resolution and the read-only
//! guarantee (mempack).

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::*;
use quickpick::plan::{PlanRequest, plan};
use quickpick::repo;

#[test]
fn fixture_builder_self_test() {
    let fx = Fx::new();
    let base = fx.commit(&[], &[w("a/b", "1"), w("top", "t")], "base");
    let exec = fx.commit(&[base], &[wx("a/run", "#!")], "exec");
    let del_file = fx.commit(&[exec], &[d("a/run")], "delete file");
    let del_dir = fx.commit(&[del_file], &[d("a")], "delete dir");
    let side = fx.commit(&[del_dir], &[w("s", "s")], "side");
    let merge = fx.commit(&[del_dir, side], &[w("m", "m")], "merge");
    fx.branch("master", merge);
    assert_eq!(fx.oid("master"), merge);
    let m = fx.repo.find_commit(merge).unwrap();
    assert_eq!(m.parent_count(), 2);
    // Deletions left no trace of `a`.
    assert!(m.tree().unwrap().get_path(Path::new("a")).is_err());
    // The executable blob carried mode 0o100755.
    let e = fx
        .repo
        .find_commit(exec)
        .unwrap()
        .tree()
        .unwrap()
        .get_path(Path::new("a/run"))
        .unwrap();
    assert_eq!(e.filemode(), 0o100755);
}

#[test]
fn open_rejects_non_repository() {
    let dir = tempfile::tempdir().unwrap();
    let err = repo::open(Some(dir.path())).err().unwrap();
    assert!(err.to_string().contains("is not a git repository"), "{err}");
    assert_eq!(err.exit_code(), 1);
}

#[test]
fn resolve_names_the_role_and_revision() {
    let fx = Fx::new();
    let c = fx.commit(&[], &[w("f", "x")], "c");
    fx.branch("master", c);
    assert_eq!(repo::resolve(&fx.repo, "master", "--source").unwrap(), c);

    let err = repo::resolve(&fx.repo, "nope", "--onto").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("--onto"), "{msg}");
    assert!(msg.contains("nope"), "{msg}");
}

#[test]
fn bound_resolves_branch_commit_and_pin() {
    let History { fx, c1, c3, .. } = history();
    // A bare branch resolves to its tip and names the line.
    let b = repo::resolve_bound(&fx.repo, "master", "--range").unwrap();
    assert_eq!(b.oid, c3);
    assert_eq!(b.line.as_deref(), Some("master"));
    // A bare commit resolves but names no line.
    let b = repo::resolve_bound(&fx.repo, &c1.to_string(), "--range").unwrap();
    assert_eq!(b.oid, c1);
    assert_eq!(b.line, None);
    // `branch@commit` on the line pins the commit and names the line.
    let b = repo::resolve_bound(&fx.repo, &format!("master@{c1}"), "--range").unwrap();
    assert_eq!(b.oid, c1);
    assert_eq!(b.line.as_deref(), Some("master"));
    // A remote-tracking branch is a line too.
    fx.repo
        .reference("refs/remotes/upstream/stable", c3, true, "fixture")
        .unwrap();
    let b = repo::resolve_bound(&fx.repo, "upstream/stable", "--range").unwrap();
    assert_eq!(b.oid, c3);
    assert_eq!(b.line.as_deref(), Some("upstream/stable"));
}

#[test]
fn bound_pin_off_its_line_errors() {
    let History { fx, c1, .. } = history();
    // c1 is not reachable from `onto`, which branched at the merge base.
    let err = repo::resolve_bound(&fx.repo, &format!("onto@{c1}"), "--onto").unwrap_err();
    assert!(err.to_string().contains("is not on onto"), "{err}");
}

#[test]
fn bound_prefers_a_real_refname_containing_at() {
    let History { fx, c1, c3, .. } = history();
    // A branch literally named `foo@<c3>` resolves whole rather than as the
    // pin form `foo` + `<c3>` — even though the right side resolves, the
    // refname must win, so the bound lands on `foo@<c3>`'s tip (c1).
    let bar = format!("{c3}");
    fx.branch(&format!("foo@{bar}"), c1);
    let b = repo::resolve_bound(&fx.repo, &format!("foo@{bar}"), "--onto").unwrap();
    assert_eq!(b.oid, c1, "the refname must win over the pin form");
    assert_eq!(b.line.as_deref(), Some(format!("foo@{bar}").as_str()));
}

#[test]
fn bound_unresolvable_errors() {
    let History { fx, .. } = history();
    let err = repo::resolve_bound(&fx.repo, "nope", "--onto").unwrap_err();
    assert!(err.to_string().contains("does not resolve"), "{err}");
    // `@` alone is git shorthand for HEAD and resolves; empty sides do not.
    for bad in ["nope@", "@nope"] {
        let err = repo::resolve_bound(&fx.repo, bad, "--onto").unwrap_err();
        assert!(err.to_string().contains("does not resolve"), "{bad}: {err}");
    }
}

#[test]
fn source_master_does_not_fall_back_to_remotes() {
    let fx = Fx::new();
    let c = fx.commit(&[], &[w("f", "x")], "c");
    fx.repo
        .reference("refs/remotes/upstream/master", c, true, "fixture")
        .unwrap();
    let err = repo::resolve(&fx.repo, "master", "--source").unwrap_err();
    assert!(err.to_string().contains("master"), "{err}");
}

/// Collect all file paths under `dir`, relative to `dir`, sorted.
fn collect_paths(dir: &Path, out: &mut Vec<String>) {
    collect_paths_under(dir, dir, out);
    out.sort();
}

fn collect_paths_under(root: &Path, dir: &Path, out: &mut Vec<String>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_paths_under(root, &p, out);
        } else {
            out.push(p.strip_prefix(root).unwrap().to_string_lossy().into_owned());
        }
    }
}

#[test]
fn mempack_keeps_written_objects_out_of_the_object_store() {
    let fx = Fx::new();
    fx.commit(&[], &[w("f", "x")], "c");
    let objects = fx.repo.path().join("objects");
    let listing = |out: &mut Vec<String>| collect_paths(&objects, out);

    let mut before = Vec::new();
    listing(&mut before);

    repo::install_mempack(&fx.repo).unwrap();
    // Write a tree object through the odb; it must land in the mempack
    // backend, not on disk.
    let blob = fx.repo.blob(b"in-memory only").unwrap();
    let mut builder = fx.repo.treebuilder(None).unwrap();
    builder.insert("f", blob, 0o100644).unwrap();
    builder.write().unwrap();

    let mut after = Vec::new();
    listing(&mut after);
    assert_eq!(before, after);
}

/// Snapshot of everything `plan` must not modify.
struct Snapshot {
    refs: BTreeMap<String, String>,
    index: Option<Vec<u8>>,
    workdir: BTreeMap<String, Vec<u8>>,
    objects: Vec<String>,
}

fn snapshot(fx: &Fx) -> Snapshot {
    let repo_dir = fx.dir.path();
    let git_dir = fx.repo.path().to_path_buf();
    let mut refs = BTreeMap::new();
    for r in fx.repo.references().unwrap().flatten() {
        refs.insert(r.name().unwrap().to_string(), format!("{:?}", r.target()));
    }
    let index = std::fs::read(git_dir.join("index")).ok();
    let mut workdir = BTreeMap::new();
    collect_files(repo_dir, &git_dir, &mut workdir);
    let mut objects = Vec::new();
    collect_paths(&git_dir.join("objects"), &mut objects);
    Snapshot {
        refs,
        index,
        workdir,
        objects,
    }
}

fn collect_files(dir: &Path, git_dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p == *git_dir {
            continue;
        }
        if p.is_dir() {
            collect_files(&p, git_dir, out);
        } else {
            out.insert(
                p.strip_prefix(dir).unwrap().to_string_lossy().into_owned(),
                std::fs::read(&p).unwrap(),
            );
        }
    }
}

#[test]
fn planning_leaves_the_repository_untouched() {
    let History { fx, .. } = history();
    // Add an outside-touching commit so the plan contains a SYNC too.
    let c4 = fx.commit(
        &[fx.oid("master")],
        &[w("pkg/file", "v5"), w("other", "o2")],
        "pkg: v4 -> v5 + outside",
    );
    fx.branch("master", c4);

    let before = snapshot(&fx);
    let plan = plan(
        &fx.repo,
        &PlanRequest {
            path: "pkg".into(),
            source: "master".into(),
            onto: "onto".into(),
            from: None,
            to: None,
        },
    )
    .unwrap();
    let after = snapshot(&fx);
    assert_eq!(before.refs, after.refs);
    assert_eq!(before.index, after.index);
    assert_eq!(before.workdir, after.workdir);
    assert_eq!(before.objects, after.objects);
    assert!(plan.verified);
}
