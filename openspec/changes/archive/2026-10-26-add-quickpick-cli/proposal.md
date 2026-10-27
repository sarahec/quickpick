# Proposal

## Why

Bringing a package from one nixpkgs branch to another (for example `master` to `release-YY.MM` or `staging`) rarely comes down to a single cherry-pick. The source branch is often several commits ahead. Some of those commits touch unrelated files or no longer apply cleanly, and earlier backports may have used `cherry-pick -x` or ordinary commits. Maintainers currently work out the right sequence by hand. `quickpick` computes it: real cherry-picks where possible, and clearly explained sync commits where not, scoped to exactly the path the user names.

## What Changes

- Build the `quickpick` package's executable as `qp` (instead of the default `quickpick`). It is a CLI with three subcommands, each with a single-letter shorthand:
  - `qp propose` (`qp p`): explains the planned steps in human-readable form.
  - `qp generate` (`qp g`): prints a self-contained shell script that performs those steps.
  - `qp configure` (`qp c`; `config` is also accepted): manages the configuration file. It replaces the `config` subcommand already scaffolded via `clapfig`.
- Input is exactly one repository-relative path (file or directory). Package-name input is deferred to a later change.
- Range options:
  - `--onto <rev>` (required): the target.
  - `--source <rev>`: defaults to exactly `master`.
  - `--from <sha>`: inclusive. Defaults to the first commit after the automatically detected anchor.
  - `--to <sha>`: defaults to the newest source commit touching the path.
- Settings are resolved in the order CLI flag > `QUICKPICK__*` environment variable > `quickpick.toml` > built-in default.
- Planning:
  - The anchor is the newest source commit whose path content equals the target's.
  - Each commit in the range becomes a cherry-pick, unless it touches files outside the path or would conflict. In those cases it becomes a sync commit that sets the path to that commit's state.
  - Consecutive sync commits are merged.
  - The final state is verified against the path at `--to`.
- Script dialect is configurable: `git` (default) or `jujutsu`. Cherry-picks carry the `(cherry picked from commit <sha>)` trailer in both dialects. Sync commits carry a descriptive explanation.
- The tool never modifies the repository and never executes the commands it generates.

## Capabilities

### New Capabilities

- `quickpick-cli`: Planning how to sync a single path in a nixpkgs checkout from a source revision to a target revision. Explains the plan or emits it as a git or jujutsu shell script, without changing the repository.

### Modified Capabilities

None.

## Impact

- **Binary name:** `Cargo.toml` gains a `[[bin]]` entry named `qp`. The package, config file (`quickpick.toml`) and environment prefix (`QUICKPICK__*`) keep the name `quickpick`.
- **Code:** `src/main.rs` is restructured around a library crate (`src/lib.rs` plus modules) so the planning logic can be unit-tested.
- **Dependencies:** adds `git2` for in-process repository reads and in-memory cherry-pick simulation. The already declared `clap`, `clapfig` and `serde` get used. `url` has no planned use and may be removed.
- **Users:** need a local nixpkgs clone (git, or jj colocated with git) that contains the source and target revisions.
- **Out of scope for this change:**
  - package-name input;
  - selecting a range by version;
  - copying changes outside the path (`--allow-outside`);
  - multiple paths;
  - patch-ID detection of earlier backports;
  - executing the plan.
