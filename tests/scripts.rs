//! Executing the emitted scripts: they must actually produce the planned
//! state, for both dialects.

mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use common::*;
use git2::Oid;
use quickpick::repo;
use tempfile::TempDir;

#[test]
fn git_script_applies_the_plan() {
    if !on_path("git") {
        eprintln!("skipping: git is not on PATH");
        return;
    }
    let h = history();
    let (out, _h) = qp(&history_args(&h));
    assert!(out.status.success(), "{}", stderr(&out));
    let script = stdout(&out);

    let result = run_script(
        &script,
        &[
            ("GIT_CONFIG_GLOBAL", Path::new("/dev/null")),
            ("GIT_CONFIG_NOSYSTEM", Path::new("1")),
            ("GIT_AUTHOR_NAME", Path::new("T")),
            ("GIT_AUTHOR_EMAIL", Path::new("t@x")),
            ("GIT_COMMITTER_NAME", Path::new("T")),
            ("GIT_COMMITTER_EMAIL", Path::new("t@x")),
        ],
    );
    assert!(result.status.success(), "{}", stderr(&result));

    // The new branch tip carries exactly the path's state at the range end.
    let tip = h.fx.oid("quickpick/pkg");
    let got = repo::path_entry(&h.fx.repo, tip, Path::new("pkg")).unwrap();
    let want = repo::path_entry(&h.fx.repo, h.c3, Path::new("pkg")).unwrap();
    assert_eq!(got, want);

    // The last step was a PICK, so the tip is a cherry-pick with the trailer.
    let tipc = h.fx.repo.find_commit(tip).unwrap();
    let msg = tipc.message().unwrap().to_string();
    assert!(
        msg.contains(&format!("cherry picked from commit {}", h.c3)),
        "{msg}"
    );
}

#[test]
fn jj_script_applies_the_plan() {
    if !on_path("jj") {
        eprintln!("skipping: jj is not on PATH");
        return;
    }
    let h = history();
    let cfg_dir = TempDir::new().unwrap();
    let cfg = cfg_dir.path().join("jj.toml");
    fs::write(&cfg, "[user]\nname = \"T\"\nemail = \"t@x\"\n").unwrap();
    let envs = [("JJ_CONFIG", cfg.as_path())];
    let run_jj = |args: &[&str]| -> std::process::Output {
        Command::new("jj")
            .args(args)
            .current_dir(h.fx.dir.path())
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .envs(envs.iter().map(|(k, v)| (k, v.as_os_str())))
            .output()
            .unwrap()
    };
    let init = run_jj(&["git", "init", "--colocate"]);
    assert!(init.status.success(), "{}", stderr(&init));

    let mut args = history_args(&h);
    args.insert(0, "jujutsu".into());
    args.insert(0, "--vcs".into());
    let (out, _h) = qp(&args);
    assert!(out.status.success(), "{}", stderr(&out));

    let result = run_script(&stdout(&out), &envs);
    assert!(result.status.success(), "{}", stderr(&result));

    // @ points at a fresh change on top of the synced state; @- is it.
    let at = run_jj(&["log", "-r", "@-", "--no-graph", "-T", "commit_id"]);
    assert!(at.status.success(), "{}", stderr(&at));
    let at_id: Oid = stdout(&at).trim().parse().unwrap();
    let got = repo::path_entry(&h.fx.repo, at_id, Path::new("pkg")).unwrap();
    let want = repo::path_entry(&h.fx.repo, h.c3, Path::new("pkg")).unwrap();
    assert_eq!(got, want);

    let conflicts = run_jj(&["log", "-r", "conflicts()", "--no-graph", "-T", "commit_id"]);
    assert!(conflicts.status.success(), "{}", stderr(&conflicts));
    assert_eq!(stdout(&conflicts).trim(), "");
}
