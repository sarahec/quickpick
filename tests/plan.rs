//! Tests for path entry/touches, range and anchor detection, step
//! classification, sync merging and verification.

mod common;

use std::path::Path;

use common::*;
use quickpick::error::Error;
use quickpick::plan::{PlanRequest, Reason, Step, plan};
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

fn step_ids(p: &quickpick::plan::Plan) -> Vec<git2::Oid> {
    p.steps
        .iter()
        .map(|s| match s {
            Step::Pick { commit } => commit.id,
            Step::Sync { target, .. } => target.id,
        })
        .collect()
}

// ---- 4.1 entry lookup and "touches the path" ----

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
fn anchor_found_and_range_planned() {
    let History { fx, c1, c2, c3, .. } = history();
    let p = plan(&fx.repo, &request("pkg")).unwrap();
    assert_eq!(p.anchor, Some(c1));
    assert_eq!(p.to, c3);
    assert!(p.verified);
    assert_eq!(step_ids(&p), vec![c2, c3]);
    assert!(
        p.steps.iter().all(|s| matches!(s, Step::Pick { .. })),
        "expected all picks: {:?}",
        p.steps
    );
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
    assert_eq!(step_ids(&p), vec![c1, c2, c3]);
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
    assert!(p.verified);
    let mut ids = step_ids(&p);
    ids.sort();
    let mut expected = vec![c2, s1, c3];
    expected.sort();
    assert_eq!(ids, expected);
}

// ---- 4.3 range ends ----

#[test]
fn explicit_to_not_on_source_errors() {
    let History { fx, ot, .. } = history();
    let mut req = request("pkg");
    req.to = Some(ot.to_string());
    let err = plan(&fx.repo, &req).unwrap_err();
    assert!(
        err.to_string().contains("not a commit on the source"),
        "{err}"
    );
}

#[test]
fn explicit_to_is_respected() {
    let History { fx, c2, .. } = history();
    let mut req = request("pkg");
    req.to = Some(c2.to_string());
    let p = plan(&fx.repo, &req).unwrap();
    assert_eq!(p.to, c2);
    assert_eq!(step_ids(&p), vec![c2]);
}

#[test]
fn explicit_from_is_inclusive() {
    let History { fx, c2, c3, .. } = history();
    let mut req = request("pkg");
    req.from = Some(c2.to_string());
    let p = plan(&fx.repo, &req).unwrap();
    assert_eq!(step_ids(&p), vec![c2, c3]);
    assert_eq!(
        p.anchor,
        Some(fx.repo.find_commit(c2).unwrap().parent_id(0).unwrap())
    );
}

#[test]
fn from_not_an_ancestor_of_to_errors() {
    let History { fx, ot, .. } = history();
    let mut req = request("pkg");
    req.from = Some(ot.to_string());
    let err = plan(&fx.repo, &req).unwrap_err();
    assert!(err.to_string().contains("not an ancestor of --to"), "{err}");
}

#[test]
fn from_mismatch_warns_and_conflicts_to_sync() {
    // onto's path disagrees with the content just before --from.
    let History { fx, mb, c2, c3, .. } = history();
    let ot = fx.commit(&[mb], &[w("pkg/file", "vX")], "target diverged");
    fx.branch("onto", ot);
    let mut req = request("pkg");
    req.from = Some(c2.to_string());
    let p = plan(&fx.repo, &req).unwrap();
    assert!(
        p.warnings.iter().any(|w| w.contains("--from")),
        "warnings: {:?}",
        p.warnings
    );
    // pkg/file is a single line: target's "vX" vs c2's "v3" both changed
    // it relative to c1's "v2" — c2 conflicts, c3 picks on top of the sync.
    match &p.steps[..] {
        [
            Step::Sync {
                covered, target, ..
            },
            Step::Pick { commit },
        ] => {
            assert!(matches!(covered[0].1, Reason::Conflict(_)));
            assert_eq!(target.id, c2);
            assert_eq!(commit.id, c3);
        }
        other => panic!("unexpected steps: {other:?}"),
    }
    assert!(p.verified);
}

// ---- 5.1 outside paths and shared-definition warnings ----

#[test]
fn outside_touching_commit_syncs_with_paths_and_warns() {
    let History { fx, c1, c3, .. } = history();
    let outside = fx.commit(
        &[c1],
        &[
            w("pkg/file", "v3"),
            w("other", "o2"),
            w("pkgs/top-level/all-packages.nix", "ap"),
        ],
        "bump + outside",
    );
    let c4 = fx.commit(&[outside], &[w("pkg/file", "v9")], "follow-up");
    let _ = c3;
    fx.branch("master", c4);

    let p = plan(&fx.repo, &request("pkg")).unwrap();
    match &p.steps[..] {
        [
            Step::Sync {
                covered, target, ..
            },
            Step::Pick { commit },
        ] => {
            let Reason::OutsidePaths(paths) = &covered[0].1 else {
                panic!("expected OutsidePaths: {:?}", covered[0].1)
            };
            assert!(paths.contains(&"other".to_string()));
            assert!(paths.contains(&"pkgs/top-level/all-packages.nix".to_string()));
            assert_eq!(target.id, outside);
            assert_eq!(commit.id, c4);
        }
        other => panic!("unexpected steps: {other:?}"),
    }
    // The shared definition file warning names the commit and the file.
    assert!(
        p.warnings
            .iter()
            .any(|w| w.contains("all-packages.nix") && w.contains(&outside.to_string()[..7])),
        "warnings: {:?}",
        p.warnings
    );
}

// ---- 5.2 classification ----

#[test]
fn path_deletion_in_a_sync_removes_the_path() {
    let History { fx, c1, .. } = history();
    let del = fx.commit(&[c1], &[d("pkg"), w("other", "o2")], "remove pkg + outside");
    fx.branch("master", del);
    let p = plan(&fx.repo, &request("pkg")).unwrap();
    match &p.steps[..] {
        [
            Step::Sync {
                removes_path,
                target,
                ..
            },
        ] => {
            assert!(*removes_path);
            assert_eq!(target.id, del);
        }
        other => panic!("unexpected steps: {other:?}"),
    }
    assert!(p.verified);
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
    assert!(plan(&fx.repo, &req).unwrap().verified);
}

// ---- 5.3 sync merging and verification ----

#[test]
fn adjacent_syncs_merge_into_one_step() {
    let History { fx, c1, .. } = history();
    let s1 = fx.commit(&[c1], &[w("pkg/file", "v3"), w("other", "x")], "sync 1");
    let s2 = fx.commit(&[s1], &[w("pkg/file", "v4"), w("other2", "y")], "sync 2");
    let pick = fx.commit(&[s2], &[w("pkg/file", "v5")], "pick");
    fx.branch("master", pick);

    let p = plan(&fx.repo, &request("pkg")).unwrap();
    match &p.steps[..] {
        [
            Step::Sync {
                covered, target, ..
            },
            Step::Pick { commit },
        ] => {
            assert_eq!(covered.len(), 2);
            assert_eq!(covered[0].0.id, s1);
            assert_eq!(covered[1].0.id, s2);
            assert_eq!(target.id, s2);
            assert_eq!(commit.id, pick);
        }
        other => panic!("unexpected steps: {other:?}"),
    }
    assert!(p.verified);
}

#[test]
fn evil_merge_fails_verification() {
    let fx = Fx::new();
    // 5-line file so the target's change (line 5) and the post-merge
    // commit's change (line 2) merge cleanly but leave line 5 wrong.
    let mb = fx.commit(
        &[],
        &[w("pkg/f", "a\nb\nc\nd\ne\n"), w("other", "o")],
        "base",
    );
    let c1 = fx.commit(&[mb], &[w("pkg/f", "a\nb\nc\nd\ny\n")], "anchor");
    let s = fx.commit(&[mb], &[w("other", "s")], "side");
    // Evil merge: resolves the path to a third content ("m" on line 5).
    let m = fx.commit(&[c1, s], &[w("pkg/f", "a\nb\nc\nd\nm\n")], "evil merge");
    let c2 = fx.commit(&[m], &[w("pkg/f", "a\nz\nc\nd\nm\n")], "tip");
    let ot = fx.commit(&[mb], &[w("pkg/f", "a\nb\nc\nd\ny\n")], "target");
    fx.branch("master", c2);
    fx.branch("onto", ot);

    let err = plan(&fx.repo, &request("pkg")).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("verification failed"), "{msg}");
    assert!(msg.contains("merge"), "{msg}");
    assert_eq!(err.exit_code(), 1);
}
