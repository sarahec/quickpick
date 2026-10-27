//! Renders a plan as a condensed one-line-per-step summary (`--summary`).

use std::fmt::Write;

use crate::config::Vcs;
use crate::plan::{Plan, Step};
use crate::render::short;

/// The `--summary` output for `plan`: the same resolved header, range and
/// anchor as the explain renderer, but one `KIND <short-id>` line per
/// step and no covered-commit detail. Warnings are not included; the
/// caller sends them to stderr.
pub fn render(plan: &Plan, vcs: Vcs) -> String {
    let mut out = format!(
        "source:  {} ({})\ntarget:  {} ({})\npath:    {}\n",
        plan.source_rev,
        short(plan.source),
        plan.onto_rev,
        short(plan.onto),
        plan.path,
    );
    if plan.steps.is_empty() {
        let _ = writeln!(
            out,
            "nothing to do: {} on {} already matches {} at {}",
            plan.path,
            plan.onto_rev,
            plan.source_rev,
            short(plan.to),
        );
        return out;
    }
    let anchor = match plan.anchor {
        Some(a) => format!(" (anchor {})", short(a)),
        None => String::new(),
    };
    let _ = writeln!(
        out,
        "range:   {}..{}{anchor}\nsteps:",
        short(plan.from.unwrap_or(plan.to)),
        short(plan.to),
    );
    for (i, step) in plan.steps.iter().enumerate() {
        let (kind, id) = match step {
            Step::Pick { commit } => ("PICK", commit.id),
            Step::Sync { target, .. } => ("SYNC", target.id),
        };
        let _ = writeln!(out, "  {}. {kind} {}", i + 1, short(id));
    }
    let _ = writeln!(
        out,
        "verification: OK, result matches {} at {}",
        plan.path,
        short(plan.to),
    );
    if vcs == Vcs::Jujutsu {
        out.push_str(
            "note: conflicts are predicted with libgit2; jj may still record a conflicted \
             change. The script ends with 'jj log -r conflicts()' so you can check.\n",
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{CommitInfo, Reason};
    use crate::render::message::tests::{commit, plan};

    fn mixed_plan() -> Plan {
        let mut p = plan("pkgs/by-name/op/openai");
        let a = commit(0x11, "openai: 1.0 -> 1.1");
        let b = commit(0x12, "openai: 1.1 -> 1.2");
        let c = commit(0x13, "openai: 1.2 -> 1.3");
        let d = commit(0x14, "openai: 1.3 -> 1.4");
        p.from = Some(a.id);
        p.to = d.id;
        p.anchor = Some(a.id);
        p.steps = vec![
            Step::Pick { commit: a },
            Step::Sync {
                target: c.clone(),
                covered: vec![
                    (
                        b,
                        Reason::OutsidePaths(vec!["pkgs/top-level/all-packages.nix".into()]),
                    ),
                    (
                        c,
                        Reason::Conflict(vec!["pkgs/by-name/op/openai/package.nix".into()]),
                    ),
                ],
                removes_path: false,
            },
            Step::Pick { commit: d },
        ];
        p
    }

    #[test]
    fn mixed_plan_renders_exactly_the_condensed_lines() {
        let out = render(&mixed_plan(), Vcs::Git);
        assert_eq!(
            out,
            "source:  master (0000001)\n\
             target:  release-26.05 (0000002)\n\
             path:    pkgs/by-name/op/openai\n\
             range:   0000011..0000014 (anchor 0000011)\n\
             steps:\n\
             \x20 1. PICK 0000011\n\
             \x20 2. SYNC 0000013\n\
             \x20 3. PICK 0000014\n\
             verification: OK, result matches pkgs/by-name/op/openai at 0000014\n",
            "{out}"
        );
    }

    #[test]
    fn empty_plan_says_nothing_to_do() {
        let mut p = plan("pkg");
        p.from = None;
        p.anchor = None;
        let out = render(&p, Vcs::Git);
        assert!(!out.contains("steps:"), "{out}");
        assert!(
            out.contains("nothing to do: pkg on release-26.05 already matches master at 0000004"),
            "{out}"
        );
    }

    #[test]
    fn jujutsu_note_only_for_jujutsu() {
        let p = mixed_plan();
        assert!(render(&p, Vcs::Jujutsu).contains("jj may still record a conflicted change"));
        assert!(!render(&p, Vcs::Git).contains("jj may still record"));
    }

    #[test]
    fn commit_helper() {
        let c: CommitInfo = commit(1, "s");
        assert_eq!(c.summary, "s");
    }
}
