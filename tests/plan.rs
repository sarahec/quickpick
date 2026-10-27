//! Tests for path entry/touches, and range and anchor detection.

mod common;

use std::path::Path;

use common::*;
use quickpick::error::Error;
use quickpick::plan::{PlanRequest, plan};
use quickpick::repo;

fn request(path: &str) -> PlanRequest {
    PlanRequest {
        path: path.into(),
        source: "master".into(),
        onto: "onto".into(),
        from: None,
        to: None,
    }
}

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

// ---- 4.2 anchor and bounded walk ----

#[test]
fn anchor_found_and_range_walked() {
    let History { fx, c1, c2, c3, .. } = history();
    let p = plan(&fx.repo, &request("pkg")).unwrap();
    assert_eq!(p.anchor, Some(c1));
    assert_eq!(p.to, c3);
    assert_eq!(p.range, vec![c2, c3]);
}

#[test]
fn anchor_at_the_merge_base() {
    let History {
        fx, mb, c1, c2, c3, ..
    } = history();
    // Retarget `onto` at a commit whose path equals the merge base's.
    let ot2 = fx.commit(&[mb], &[w("t", "t2")], "target untouched");
    fx.branch("onto", ot2);
    let p = plan(&fx.repo, &request("pkg")).unwrap();
    assert_eq!(p.anchor, Some(mb));
    assert_eq!(p.range, vec![c1, c2, c3]);
}

#[test]
fn no_anchor_errors_without_from() {
    let History { fx, mb, .. } = history();
    let ot = fx.commit(&[mb], &[w("pkg/file", "stray")], "target diverged");
    fx.branch("onto", ot);
    let err = plan(&fx.repo, &request("pkg")).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("no common state"), "{msg}");
    assert!(msg.contains("--from"), "{msg}");
}

#[test]
fn already_in_sync_is_an_empty_plan() {
    let History { fx, mb, c3, .. } = history();
    let ot = fx.commit(&[mb], &[w("pkg/file", "v4")], "target in sync");
    fx.branch("onto", ot);
    let p = plan(&fx.repo, &request("pkg")).unwrap();
    assert!(p.steps.is_empty());
    assert_eq!(p.from, None);
    assert!(p.verified);
    assert_eq!(p.to, c3);
}

#[test]
fn merge_commits_are_skipped_but_their_commits_are_walked() {
    let fx = Fx::new();
    // The side branch adds a disjoint file, so both merge parents' commits
    // pick cleanly regardless of walk order.
    let mb = fx.commit(&[], &[w("pkg/f", "a\nb\n"), w("other", "o")], "base");
    let c1 = fx.commit(&[mb], &[w("pkg/f", "a2\nb\n")], "anchor");
    let c2 = fx.commit(&[c1], &[w("pkg/f", "a3\nb\n")], "master edit");
    let s1 = fx.commit(&[c1], &[w("pkg/extra", "e")], "side edit");
    let m = fx.commit(&[c2, s1], &[w("pkg/extra", "e"), w("m", "m")], "merge side");
    let c3 = fx.commit(&[m], &[w("pkg/f", "a3\nx\n")], "after merge");
    let ot = fx.commit(&[mb], &[w("pkg/f", "a2\nb\n")], "target");
    fx.branch("master", c3);
    fx.branch("onto", ot);

    let p = plan(&fx.repo, &request("pkg")).unwrap();
    let mut ids = p.range;
    ids.sort();
    let mut expected = vec![c2, s1, c3];
    expected.sort();
    assert_eq!(ids, expected);
}

#[test]
fn usage_error_on_bad_path() {
    let History { fx, .. } = history();
    for bad in ["/abs/path", "../up", ""] {
        let mut req = request(bad);
        req.path = bad.into();
        let err = plan(&fx.repo, &req).unwrap_err();
        assert!(matches!(err, Error::Usage(_)), "{bad}: {err:?}");
        assert_eq!(err.exit_code(), 2);
    }
    // Trailing slash is normalized away.
    let mut req = request("pkg/");
    req.path = "pkg/".into();
    plan(&fx.repo, &req).unwrap();
}
