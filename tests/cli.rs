//! End-to-end tests of the `qp` binary against fixture repositories.

mod common;

use common::*;

#[test]
fn propose_and_generate_report_the_same_steps() {
    let m = mixed();
    let (propose, _h1) = qp(&mixed_args("propose", &m));
    let (generate, _h2) = qp(&mixed_args("generate", &m));
    assert!(propose.status.success(), "{}", stderr(&propose));
    assert!(generate.status.success(), "{}", stderr(&generate));
    // The commits the plan names — the sync target and the pick — are the
    // same in both outputs: full ids in the script, shorts in the proposal.
    let s2 = m.s2.to_string();
    let s3 = m.s3.to_string();
    let script = stdout(&generate);
    let proposal = stdout(&propose);
    for id in [&s2, &s3] {
        assert!(script.contains(id), "{script}");
        assert!(proposal.contains(&id[..7]), "{proposal}");
    }
    // The covered (conflicting) source commit appears in both, too.
    let s1 = m.s1.to_string();
    assert!(script.contains(&s1[..7]), "{script}");
    assert!(proposal.contains(&s1[..7]), "{proposal}");
    assert!(proposal.contains("PICK"), "{proposal}");
    assert!(proposal.contains("SYNC"), "{proposal}");
    sh_check(&script);
}

#[test]
fn nothing_to_do_exits_zero_with_empty_generate_stdout() {
    let History { fx, mb, c3, .. } = history();
    let ot = fx.commit(&[mb], &[w("pkg/file", "v4")], "target in sync");
    fx.branch("onto", ot);
    let _ = c3;
    let (out, _h) = qp(&[
        "generate",
        "--repo",
        &fx.dir.path().to_string_lossy(),
        "--onto",
        "onto",
        "pkg",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "");
    assert!(stderr(&out).contains("nothing to do"), "{}", stderr(&out));
}

#[test]
fn missing_onto_exits_one_and_names_the_env_var() {
    let History { fx, .. } = history();
    let (out, _h) = qp(&["propose", "--repo", &fx.dir.path().to_string_lossy(), "pkg"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("--onto"), "{err}");
    assert!(err.contains("QUICKPICK__ONTO"), "{err}");
    assert!(err.contains("quickpick.toml"), "{err}");
}

#[test]
fn warnings_go_to_stderr_while_stdout_stays_a_script() {
    let m = mixed();
    let (out, _h) = qp(&mixed_args("generate", &m));
    assert!(out.status.success(), "{}", stderr(&out));
    // --from s1 does not continue the target's state (vX vs v2).
    assert!(stderr(&out).contains("warning"), "{}", stderr(&out));
    assert!(stderr(&out).contains("--from"), "{}", stderr(&out));
    sh_check(&stdout(&out));
}

#[test]
fn bad_path_exits_two() {
    let History { fx, .. } = history();
    let (out, _h) = qp(&[
        "propose",
        "--repo",
        &fx.dir.path().to_string_lossy(),
        "--onto",
        "onto",
        "../outside",
    ]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
}
