## Why

Maintaining Nixpkgs often requires backporting package updates from `master` to a release branch. Identifying the exact commits for a package and producing the right `git cherry-pick -x` commands is tedious and error-prone. `NIXPKGS` automates this lookup so backporting becomes a single CLI invocation.

## What Changes

- Add a new Rust CLI binary named `NIXPKGS` to the `quickpick` crate.
- Accept a Nixpkgs package attribute path (e.g. `python3Packages.openai`) with optional `--from` and `--to` version filters.
- Resolve the package's Nix expression path and use the local `nixpkgs` Git repository.
- Compare the source branch (`master` by default) and current release branch (`release-26.05` by default) histories for that expression path.
- Output a list of `git cherry-pick -x <commit>` commands for commits present on the source branch but absent from the release branch.
- Support environment variables for the repository path (`NIXPKGS`), source branch (`NIXPKGS_SOURCE_BRANCH`), and release branch (`NIXPKGS_RELEASE_BRANCH`), in addition to CLI flags.
- Add a `--dry-run` flag that prints a human-readable plan (commit SHA and summary) instead of executable cherry-pick commands.
- Add `-n` as a synonym for `--dry-run`.
- Add minimal CLI parsing and basic tests for commit filtering, command formatting, and option resolution.

## Capabilities

### New Capabilities

- `quickpick-cli`: A command-line tool that discovers backport commits for a Nixpkgs package and emits `git cherry-pick -x` commands.

### Modified Capabilities

- None.

## Impact

- New Rust dependencies for Git operations and CLI argument parsing will be added to `Cargo.toml`.
- A new `src/main.rs` structure (or additional modules) will replace the current "Hello, world!" placeholder.
- The tool assumes a local `nixpkgs` clone with source and release branches that default to `master` and `release-26.05`.
