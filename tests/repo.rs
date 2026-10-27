//! Tests for the `git2` fixture builder and repository access.

mod common;

use std::path::Path;

use common::*;

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
