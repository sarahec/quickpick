# Tasks

## 1. Project setup

- [x] 1.1 Add a `[[bin]]` entry to `Cargo.toml` with `name = "qp"` and `path = "src/main.rs"`, keeping the package name `quickpick`, and set `#[command(name = "qp")]`; verify `cargo build` produces `target/debug/qp` and no `target/debug/quickpick` executable, and `cargo run -- --help` prints `Usage: qp`
- [x] 1.2 Add `git2` with `cargo add git2` and `tempfile` with `cargo add --dev tempfile`; remove `url` with `cargo remove url` if nothing needs it; verify `cargo build` succeeds
- [x] 1.3 Check the clapfig 0.21.4 API against design decision 2 (`Schema` derive, `env_prefix`, `cli_override`, unit-enum validation, `ConfigArgs`); upgrade with `cargo add clapfig@<version>` if needed; record the version used in design.md; verify `cargo build` succeeds
- [x] 1.4 Create the crate layout from design decision 1 (`lib.rs`, `cli.rs`, `config.rs`, `repo.rs`, `plan.rs`, `render/{propose,git,jj,message}.rs`, `error.rs`) with doc comments on every module; make `main.rs` a thin wrapper around `lib::run`; verify `cargo build` and `cargo clippy -- -D warnings` pass

## 2. CLI and configuration

- [x] 2.1 Define `Cli` with the `propose`, `generate` and `configure` subcommands (replacing the scaffolded `config` variant), and a shared flattened `PlanArgs` (`--onto`, `--source`, `--from`, `--to`, `--vcs`, `--branch`, positional `<path>`); verify with unit tests that a missing subcommand and a missing path both fail with usage errors
- [x] 2.2 Add visible aliases `p` (propose), `g` (generate), and `c` plus `config` (configure); verify with unit tests that each alias parses to the same command as its full name, and that `qp --help` output lists the aliases
- [x] 2.3 Define the clapfig settings struct (`repo`, `source` defaulting to `master`, `onto`, `vcs` as a `git`/`jujutsu` enum defaulting to `git`, `branch`) with `QUICKPICK` as the env prefix and `quickpick.toml` as the file; verify with unit tests the precedence flag > env > file > default, and that an invalid `vcs` lists the allowed values
- [x] 2.4 Report a missing `onto` with a message naming the flag, `QUICKPICK__ONTO` and the config key; verify with a unit test
- [x] 2.5 Implement `Error` with human-readable `Display` and the exit-code mapping from design decision 8 (0 / 1 / 2); write errors to stderr; verify with unit tests on the mapping

## 3. Repository access

- [x] 3.1 Add a test fixture builder (under `tests/` or a `#[cfg(test)]` module) that creates a temporary git repository with configurable source and target branches, in-path commits, outside-touching commits, conflicting commits, merge commits and path deletions; verify with a self-test that builds every fixture shape
- [x] 3.2 Implement repository opening (`repo` setting or cwd), revision resolution for `source`/`onto`/`from`/`to`, and the "not a git repository" and "unresolved revision" errors, including `source` defaulting to exactly `master` with no remote fallback; verify with fixture tests
- [x] 3.3 Install the mempack object-database backend before any write (design decision 3); verify with a test that planning leaves the refs, index, working tree and the `.git/objects` file listing unchanged, or apply the documented fallback and adjust that test

## 4. Range and anchor

- [x] 4.1 Implement path entry lookup and "touches the path" by comparing entry IDs with the first parent; verify with unit tests for files, directories and absent paths
- [x] 4.2 Implement the bounded candidate walk and anchor search from design decision 5, including merge-commit skipping, the merge-base check and oldest-first ordering; verify with fixture tests for: anchor found, no anchor, target already in sync, anchor at the merge base
- [x] 4.3 Implement the `--to` default and validation (on source), the `--from` default (first after the anchor), inclusive `--from`, the ancestry check, and the warning when the explicit `--from` doesn't match the target; verify with fixture tests for each scenario in the spec's range requirements

## 5. Simulation and classification

- [x] 5.1 Implement "touches outside the path" via a tree diff, and the shared definition file warning for `pkgs/top-level/`; verify with fixture tests
- [x] 5.2 Implement the simulation loop from design decision 6 (`merge_trees` for picks; entry replacement or removal for syncs) and step classification with reasons; verify with fixture tests covering: clean pick, outside-touching sync, conflicting sync, picks continuing after a sync, path deletion
- [x] 5.3 Merge adjacent SYNC steps, and verify the final state against the path at `--to`; verify with unit tests for merged syncs and a fixture in which verification fails (evil merge) and produces an error

## 6. Rendering

- [ ] 6.1 Implement the cherry-pick message (original message, blank line, `(cherry picked from commit <full sha>)`) and the sync message format from the spec; verify with string-comparison unit tests, including a multi-commit sync and a path removal
- [ ] 6.2 Implement `propose` output: resolved source, target, range, anchor, ordered steps with kind, SHA, date and summary, sync reasons, warnings, verification result, plus the jj conflict note when `vcs = jujutsu`; verify with snapshot-style string tests
- [ ] 6.3 Implement the git dialect script (shebang, `set -eu`, `cd`, `git switch -c`, `cherry-pick -x`, `rm` + `checkout` + `commit -F -` for syncs, removal syncs, `quickpick/<last component>` as the default branch) with safe single-quoting and here-documents; verify with string tests, including paths and messages containing quotes
- [ ] 6.4 Implement the jujutsu dialect script (`jj new`, `jj duplicate -B @` + `jj describe @-`, `jj restore --from` with a `root:` fileset + `describe` + `new` for syncs, final `jj log -r 'conflicts()'`); verify with string tests

## 7. Integration

- [ ] 7.1 Wire `propose` and `generate` end to end in `lib::run`, with warnings on stderr and plan or script on stdout; verify with binary tests (`CARGO_BIN_EXE_qp`) against fixtures, showing that both subcommands report the same steps and that "nothing to do" exits 0 with empty script steps
- [ ] 7.2 Add an integration test that runs the generated git script with `sh` in a fixture clone and asserts the path at the new branch tip equals the path at `--to`; add the equivalent jj test, skipped when `jj` is not on `PATH`; verify both pass, or the jj test is reported skipped
- [ ] 7.3 Run `qp propose` against a real local nixpkgs clone, if available, for one package with a known backport history; record the run time and the plan's plausibility in design.md risks; narrow `merge_trees` to the subtree if it's too slow
- [ ] 7.4 Document usage (subcommands, settings, precedence, dialects, examples) in a `README.md` and in clap help text; verify that `qp --help`, `qp propose --help` and `qp generate --help` show it
- [ ] 7.5 Run the project's verification checklist: `cargo build`, `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`; verify that all pass
