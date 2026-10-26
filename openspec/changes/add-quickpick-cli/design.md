## Context

`quickpick` is a Rust 2024 crate with no external dependencies. It currently prints "Hello, world!". This change turns it into `NIXPKGS`, a small CLI utility for Nixpkgs maintainers who need to backport package updates from a source branch to a release branch.

The tool must:
1. Accept a Nixpkgs attribute path such as `python3Packages.openai`.
2. Locate the package's expression file within a local Nixpkgs Git clone.
3. Find commits on the source branch (`master` by default) that touched that file.
4. Exclude commits already reachable from the release branch (`release-26.05` by default).
5. Print `git cherry-pick -x <sha>` commands in chronological order, or a dry-run summary when requested.

## Goals / Non-Goals

**Goals:**
- Provide a single, fast CLI command to produce a backport cherry-pick list.
- Use robust Git operations on a local `nixpkgs` clone.
- Allow optional `--from` and `--to` version filters.
- Keep the implementation testable with unit tests for path resolution, commit filtering, and command formatting.

**Non-Goals:**
- Executing the cherry-picks (output only).
- Supporting remote repositories or shallow clones without enough history.
- Providing an interactive or TUI mode.
- Resolving arbitrary Nix expressions outside the `nixpkgs` package tree.
- Backporting non-package files (e.g. modules, NixOS tests).

## Decisions

### CLI parsing with `clap`
- **Choice:** Use the `clap` crate (derive or builder API) for argument parsing.
- **Rationale:** `clap` is the de facto standard in Rust. The derive API keeps the CLI definition declarative and easy to extend with `--from`, `--to`, `--repo`, `--source-branch`, `--release-branch`, and `--dry-run`. It also supports reading default values from environment variables.

### Environment variable configuration
- **Choice:** Expose `NIXPKGS`, `NIXPKGS_SOURCE_BRANCH`, and `NIXPKGS_RELEASE_BRANCH` as environment-variable defaults for their corresponding CLI flags.
- **Rationale:** Maintainers often work with the same repository and branches repeatedly. Environment variables let them set defaults once without typing flags on every invocation. CLI flags take precedence when both are present.

### Source branch naming
- **Choice:** Rename the `--master-branch` flag to `--source-branch` and back it with `NIXPKGS_SOURCE_BRANCH`, defaulting to `master`.
- **Rationale:** The source of backports is not always `master` (e.g. staging or feature branches). A generic name is clearer and matches the release-branch naming convention.

### Git operations with `git2`
- **Choice:** Use the `git2` crate (libgit2 bindings) instead of shelling out to the `git` CLI.
- **Rationale:** Programmatic access to revwalks and pathspec filters is more reliable and easier to unit-test with in-memory repositories. The `git2` API is mature and lets us filter commits by file path efficiently.
- **Alternative considered:** Shelling out to `git log -- <path>` and parsing output. Rejected because parsing is brittle and error handling is harder.

### Attribute path to file path resolution
- **Choice:** Resolve the attribute path to a file path by invoking the `nix` evaluator for the repository's `default.nix` and reading `builtins.unsafeGetAttrPos`.
- **Rationale:** This is the most reliable way to map `python3Packages.openai` to `pkgs/development/python-modules/openai/default.nix` regardless of aliases, overlays, or nested callPackage invocations.
- **Fallback considered:** Mapping well-known attribute prefixes (e.g. `python3Packages.*` → `pkgs/development/python-modules/*/default.nix`) heuristically. This will be used only if the `nix` command is unavailable, with a clear warning.

### Branch defaults and overrides
- **Choice:** Default source branch to `master` and release branch to `release-26.05`. Allow `--source-branch` and `--release-branch` overrides, and environment-variable defaults via `NIXPKGS_SOURCE_BRANCH` and `NIXPKGS_RELEASE_BRANCH`.
- **Rationale:** Nixpkgs convention uses these names, but forks, staging branches, or future release branches need to be configurable without repeating flags.

### Output ordering
- **Choice:** Print commands oldest-first (chronological order by commit date on the source branch).
- **Rationale:** Cherry-picking in chronological order minimizes merge conflicts when the user runs the generated commands sequentially.

### Dry-run mode
- **Choice:** Add a `--dry-run` flag that prints a human-readable plan (one line per selected commit with SHA, author date, and summary) instead of executable `git cherry-pick -x` commands.
- **Rationale:** A dry-run lets maintainers review exactly what will be backported before they run the generated commands. It is separate from the default executable-command output because the default behavior is already safe (it does not execute), but machine-readable commands and human-readable plans serve different workflows.

### Version filtering (`--from` / `--to`)
- **Choice:** Filter commits by the version string detected in the derivation's `version` attribute at that commit.
- **Rationale:** A maintainer often wants only commits between two released versions. Detecting the version requires evaluating the Nix expression at each candidate commit, which is expensive, so this step is applied only after path-based filtering.
- **Open detail:** Exact comparison semantics (prefix match, semver, exact equality) will be specified during implementation and documented in the spec.

## Risks / Trade-offs

- **Risk:** The `nix` command is required for reliable path resolution. → **Mitigation:** Add a `--path` flag so users can bypass evaluation; also provide heuristic fallback for common prefixes.
- **Risk:** `git2` performance degrades on very large repositories like `nixpkgs`. → **Mitigation:** Use `revwalk` with a pathspec filter so libgit2 can prune irrelevant commits; keep full history walk off by default.
- **Risk:** Version filtering requires evaluating Nix at each candidate commit and may be slow. → **Mitigation:** Make `--from`/`--to` optional; cache or short-circuit when possible; document performance expectations.
- **Risk:** Attribute paths with aliases or generated packages may not map cleanly to a single file. → **Mitigation:** Fail with a clear message and suggest `--path`.
- **Risk:** Commit already present on release branch but with different content cannot be detected purely by SHA. → **Mitigation:** Document that output is SHA-based; users must review before running.
- **Risk:** Environment variables and CLI flags can interact in surprising ways if precedence is unclear. → **Mitigation:** Document the precedence order (CLI flag > env var > built-in default) in help text and the README.

## Open Questions

1. Should `--from`/`--to` use exact version strings, semver ranges, or prefix matching?
2. Should the output include a leading `cd <nixpkgs-repo>` command for copy-paste convenience?
3. Should commits that touch the package file but only change formatting/whitespace be excluded?
4. Should `--dry-run` also include the full commit message body, or only the summary line?
