//! End-to-end tests of the `qp` binary against fixture repositories.

mod common;

use common::*;

#[test]
fn explain_and_script_report_the_same_steps() {
    let h = history();
    let (explain, _h1) = qp(&[history_args(&h), vec!["--explain".into()]].concat());
    let (script, _h2) = qp(&history_args(&h));
    assert!(explain.status.success(), "{}", stderr(&explain));
    assert!(script.status.success(), "{}", stderr(&script));
    // The commits the plan names are the same in both outputs: full ids in
    // the script, shorts in the explanation.
    let c2 = h.c2.to_string();
    let c3 = h.c3.to_string();
    let script_out = stdout(&script);
    let explanation = stdout(&explain);
    for id in [&c2, &c3] {
        assert!(script_out.contains(id), "{script_out}");
        assert!(explanation.contains(&id[..7]), "{explanation}");
    }
    assert!(explanation.contains("PICK"), "{explanation}");
    sh_check(&script_out);
}

#[test]
fn summary_is_a_condensed_plan() {
    let h = history();
    let (out, _h) = qp(&[history_args(&h), vec!["--summary".into()]].concat());
    assert!(out.status.success(), "{}", stderr(&out));
    let s = stdout(&out);
    for (i, id) in [&h.c2, &h.c3].iter().enumerate() {
        let id = id.to_string();
        assert!(
            s.contains(&format!("  {}. PICK {}", i + 1, &id[..7])),
            "{s}"
        );
    }
    assert!(s.contains("verification: OK"), "{s}");
    // Condensed: no author dates, commit summaries or covered-commit detail.
    assert!(!s.contains("2023"), "{s}");
    assert!(!s.contains("covers"), "{s}");
    assert!(!s.contains("pkg: v"), "{s}");
}

#[test]
fn explain_and_summary_conflict() {
    let h = history();
    let (out, _h) = qp(&[
        "--repo",
        &h.fx.dir.path().to_string_lossy(),
        "--explain",
        "--summary",
        "pkg",
    ]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
}

#[test]
fn nothing_to_do_exits_zero_with_empty_script_stdout() {
    let History { fx, mb, c3, .. } = history();
    let ot = fx.commit(&[mb], &[w("pkg/file", "v4")], "target in sync");
    fx.branch("onto", ot);
    let _ = c3;
    let (out, _h) = qp(&[
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
fn show_config_prints_merged_values_outside_a_repo() {
    let (mut cmd, home) = qp_cmd();
    let cfgdir = home.path().join("cfg").join("quickpick");
    std::fs::create_dir(&cfgdir).unwrap();
    std::fs::write(
        cfgdir.join("quickpick.toml"),
        "vcs = \"jujutsu\"\nonto = \"release-26.05\"\n",
    )
    .unwrap();
    // Run in a directory that is not a repository and without --repo:
    // show-config must exit before repository opening and without a path.
    let dir = tempfile::tempdir().unwrap();
    let out = cmd
        .current_dir(dir.path())
        .arg("--show-config")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let s = stdout(&out);
    for line in [
        "repo = <unset>",
        "source = master",
        "onto = release-26.05",
        "vcs = jujutsu",
        "branch = <unset>",
    ] {
        assert!(s.contains(line), "missing {line:?} in:\n{s}");
    }
}

#[test]
fn missing_onto_exits_one_and_names_the_env_var() {
    let History { fx, .. } = history();
    let (out, _h) = qp(&["--repo", &fx.dir.path().to_string_lossy(), "pkg"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("--onto"), "{err}");
    assert!(err.contains("QUICKPICK__ONTO"), "{err}");
    assert!(err.contains("quickpick.toml"), "{err}");
}

#[test]
fn warnings_go_to_stderr_while_stdout_stays_a_script() {
    let History { fx, c1, .. } = history();
    // A commit that also touches a shared definition file warns on stderr.
    let c4 = fx.commit(
        &[c1],
        &[
            w("pkg/file", "v3"),
            w("pkgs/top-level/all-packages.nix", "ap"),
        ],
        "bump + shared file",
    );
    fx.branch("master", c4);
    let (out, _h) = qp(&[
        "--repo",
        &fx.dir.path().to_string_lossy(),
        "--onto",
        "onto",
        "pkg",
    ]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stderr(&out).contains("warning"), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("all-packages.nix"),
        "{}",
        stderr(&out)
    );
    sh_check(&stdout(&out));
}

#[test]
fn bad_path_exits_two() {
    let History { fx, .. } = history();
    let (out, _h) = qp(&[
        "--repo",
        &fx.dir.path().to_string_lossy(),
        "--onto",
        "onto",
        "../outside",
    ]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
}
