# Tasks

## 1. Flatten the CLI surface

- [x] 1.1 Replace `Commands`/`PlanArgs` with one flat `#[derive(Parser)]` struct: positional `path`, `--explain`/`--summary` (mutually exclusive via clap), `--range`/`-r`, `--onto`, `--source`, `--vcs`, `--repo`, `--branch`, and `--show-config`. Remove the clapfig `ConfigArgs` wiring and all aliases. Verify: unit tests show `qp <path>` parses to script mode, `--explain --summary` together fails, and a missing path is a usage error.
- [x] 1.2 Add the bound parser (`branch|commit|branch@commit`): resolve the whole value as a revision first, else split at the last `@` and assert the commit is reachable from the branch. A bound records its resolved `Oid` plus the line it names, if any. Verify: fixture-repo tests cover a bare branch, a bare commit, an on-line `branch@commit`, an off-line `branch@commit` (error), a ref literally named `foo@bar`, and a non-resolving value (error).

## 2. Settings: `--show-config`

- [x] 2.1 Implement `--show-config`: print merged settings as `key = value` and exit 0 before repository opening and without requiring `onto`. Verify: a test prints merged values (including a file-layer override) in a directory that is not a repository.

## 3. Range plumbing

- [x] 3.1 Parse `--range`/`-r a..b` into two optional bounds: require `..`, allow either side empty. Verify: parsing tests cover `a..b`, `a..`, `..b`, and rejection of a value without `..`.
- [x] 3.2 Replace the inclusive `--from` with the exclusive `a` floor throughout the planner: empty `a` defaults to the anchor commit itself (dropping the "first commit after" step), `source` is consulted only for an empty or bare-commit `b` and the anchor search bound, and a bare-commit `b` must be reachable from `source`. Drop the "from must be an ancestor of to" rule. Verify: existing plan tests ported to range inputs pass (old `--from X --to Y` ≡ `--range X^..Y`), plus new tests for the explicit-`a` content warning and an `a`/`b` pair on different lines.
- [ ] 3.3 Apply bound semantics to `--onto`, including the `branch@commit` reachability assertion. Verify: a test accepts an on-line pin and rejects an off-line pin naming the branch.

## 4. Output modes

- [ ] 4.1 Dispatch on mode in `main`: default script, `--explain` to the existing detailed renderer, `--summary` to the new one. Verify: an integration test runs each mode against a fixture and confirms script/explain/summary outputs and the flag-conflict error.
- [ ] 4.2 Implement the summary renderer: resolved source and target, anchor, range, one numbered line per step (`PICK`/`SYNC` + short id), and the verification verdict, without covered-commit detail. Verify: a renderer test on a mixed plan produces exactly the condensed lines.

## 5. Cleanup, docs and release

- [ ] 5.1 Remove dead code and stale references (subcommand/alias tests, `--from`/`--to` leftovers, `ConfigArgs`), bump the crate version to `0.2.0`, and update `README.md` with the new surface plus a 0.1 → 0.2 migration table. Verify: a search finds no `propose`/`generate`/`configure`/`--from`/`--to` references outside history, and `qp --help` shows the flat options.
- [ ] 5.2 Run the full checklist: `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, and the generated-script fixture tests in both dialects. Verify: all pass.

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied.
