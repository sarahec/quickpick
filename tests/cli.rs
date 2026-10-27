//! End-to-end tests of the `qp` binary against fixture repositories.

mod common;

use std::io::Write as _;
use std::process::{Command, Output, Stdio};

use common::*;

/// Run `qp` with a scrubbed environment: only PATH, HOME and an empty
/// XDG_CONFIG_HOME survive, so no user `quickpick.toml` or `QUICKPICK__*`
/// variable leaks in.
fn qp<S: AsRef<std::ffi::OsStr>>(args: &[S]) -> (Output, tempfile::TempDir) {
    let home = tempfile::tempdir().unwrap();
    let cfg = home.path().join("cfg");
    std::fs::create_dir(&cfg).unwrap();
    let path = std::env::var_os("PATH").unwrap_or_default();
    let out = Command::new(env!("CARGO_BIN_EXE_qp"))
        .args(args)
        .env_clear()
        .env("PATH", path)
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", &cfg)
        .output()
        .unwrap();
    (out, home)
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// `sh -n` syntax check; skipped when `sh` is not on PATH.
fn sh_check(script: &str) {
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

fn mixed_args(cmd: &str, m: &Mixed) -> Vec<String> {
    vec![
        cmd.into(),
        "--repo".into(),
        m.fx.dir.path().to_string_lossy().into_owned(),
        "--onto".into(),
        "onto".into(),
        "--from".into(),
        m.s1.to_string(),
        "pkg".into(),
    ]
}

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
