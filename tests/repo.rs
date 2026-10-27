//! Tests for the `git2` fixture builder and repository access.

mod common;

use std::path::Path;

use common::*;
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
fn source_master_does_not_fall_back_to_remotes() {
    let fx = Fx::new();
    let c = fx.commit(&[], &[w("f", "x")], "c");
    fx.repo
        .reference("refs/remotes/upstream/master", c, true, "fixture")
        .unwrap();
    let err = repo::resolve(&fx.repo, "master", "--source").unwrap_err();
    assert!(err.to_string().contains("master"), "{err}");
}
