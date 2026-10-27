//! Tests for path entry lookup and "touches the path" detection.

mod common;

use std::path::Path;

use common::*;
use quickpick::repo;

#[test]
fn entry_and_touches_for_file_dir_and_absent_path() {
    let History { fx, c1, c2, .. } = history();
    let pkg = Path::new("pkg");
    let file = Path::new("pkg/file");

    // A file: c1 changes pkg/file, so it touches both the file and the dir.
    let c = fx.repo.find_commit(c1).unwrap();
    assert!(repo::touches(&c, file).unwrap());
    assert!(repo::touches(&c, pkg).unwrap());
    // ...but not an unrelated path, nor a path that does not exist.
    assert!(!repo::touches(&c, Path::new("other")).unwrap());
    assert!(!repo::touches(&c, Path::new("nope")).unwrap());
    assert_eq!(
        repo::path_entry(&fx.repo, c1, Path::new("nope")).unwrap(),
        None
    );

    // c2 changed pkg/file, so it and its parent disagree on the entry id.
    let e_c1 = repo::path_entry(&fx.repo, c1, file).unwrap().unwrap();
    let e_c2 = repo::path_entry(&fx.repo, c2, file).unwrap().unwrap();
    assert_ne!(e_c1, e_c2);
}

#[test]
fn filemode_only_change_counts_as_a_touch() {
    let fx = Fx::new();
    let a = fx.commit(&[], &[w("pkg/run", "#!/bin/sh\n")], "add");
    let b = fx.commit(&[a], &[wx("pkg/run", "#!/bin/sh\n")], "chmod +x");
    assert!(repo::touches(&fx.repo.find_commit(b).unwrap(), Path::new("pkg/run")).unwrap());
}
