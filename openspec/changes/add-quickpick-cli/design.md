# Design

## Context

- **Starting point:** `Cargo.toml` has no `[[bin]]` section, so the executable is currently named `quickpick`. `src/main.rs` contains only a `clap` derive skeleton: a global `--verbose` flag and a `config` subcommand backed by `clapfig::ConfigArgs`. Nothing else exists yet. `Cargo.toml` declares `clap` (derive), `clapfig` 0.21.4, `serde` and `url`, and no git library. See proposal.md for motivation and specs/quickpick-cli/spec.md for behavior.
- **Scale constraint:** nixpkgs has close to a million commits and very large trees. Any operation that walks all of history, or diffs whole trees for every commit, is too slow. The design limits history walks to the range between the source and the target, and compares paths by object ID rather than by content.

## Goals / Non-Goals

**Goals:**
- Planning logic lives in a library crate, is pure apart from object-database reads, and is unit-testable against small fixture repositories.
- The repository is never modified. That covers refs, index, working tree, and also the on-disk object database (see decision 3).
- One plan model is rendered by three independent renderers: propose, git script, and jj script.

**Non-Goals:**
- Executing scripts, or interacting with GitHub or remotes (no fetching).
- Parsing Nix, or resolving packages by attribute name.
- Shallow clones whose history doesn't reach the anchor. These produce the "no anchor" error.

## Decisions

### 1. Crate layout and executable name

- **Executable name:** an explicit `[[bin]] name = "qp", path = "src/main.rs"` in `Cargo.toml`. The package stays `quickpick`, so `cargo install quickpick` installs `qp`, and the library crate stays `quickpick`. `#[command(name = "qp")]` makes help and usage text say `qp`.
- **Names that stay `quickpick`:** the config file `quickpick.toml`, the environment prefix `QUICKPICK__`, and the default git branch prefix `quickpick/`. They identify the tool, not the binary.
  - *Alternative considered:* renaming the package to `qp`. It's shorter, but `qp` is too generic as a crate name.
- **Shorthands:** clap's `visible_alias` on each `Commands` variant: `p` for `propose` and `g` for `generate`. `configure` gets `visible_aliases = ["c", "config"]`, where `config` keeps the existing scaffold's name working.

```
src/
  main.rs        thin: parse CLI, load config, call lib, map errors to exit codes
  lib.rs         pub fn run(...) wiring; module declarations
  cli.rs         clap derive: Cli, Commands { Propose, Generate, Configure }, PlanArgs
  config.rs      clapfig Schema struct: repo, source, onto, vcs, branch; Vcs enum
  repo.rs        git2 helpers: open, resolve rev, path entry id, touched-paths
  plan.rs        Range, anchor search, classification, sync merging, verification
  render/
    propose.rs   human-readable plan
    git.rs       git dialect script
    jj.rs        jujutsu dialect script
    message.rs   cherry-pick and sync commit messages (shared by both dialects)
  error.rs       Error enum (hand-written Display; no extra crate)
```

`PlanArgs` (`--onto`, `--source`, `--from`, `--to`, `--vcs`, `--branch`, `<path>`) is flattened into both `Propose` and `Generate`. This makes it impossible for the two subcommands to drift apart.

### 2. Configuration via clapfig

- **Settings struct:** a `#[derive(clapfig::Schema)]` struct holds `repo: Option<PathBuf>`, `source` (default `"master"`), `onto: Option<String>`, `vcs: Vcs` (default `git`), and `branch: Option<String>`. `Vcs` is a unit-only enum, so clapfig itself rejects values other than `git` and `jujutsu` and lists the allowed set.
- **Layers:** CLI flags are applied as clapfig's `cli_override` layer, the environment prefix is `QUICKPICK`, and the file is `quickpick.toml` in the platform config directory. That gives exactly the precedence the spec requires.
- **Required target:** `onto` is checked after loading, so the error message can name all three ways to set it.
- **Version used:** clapfig is pinned at `=0.25.0` — the newest release implementing the `Schema`-derive API described here that was published at least a week ago (0.26.0 is newer but too fresh).

*Alternative considered:* clap's built-in `env = "..."` attribute. It covers flags and environment variables, but not the config file the `configure` subcommand manages, and it would duplicate defaults in two places.

### 3. Git access with git2, with an in-memory object database

- **Library:** `git2`, i.e. libgit2, run in process. Everything needed is available: `revparse_single`, `revwalk`, `merge_base`, `graph_descendant_of`, `Tree::get_path`, `diff_tree_to_tree`, and `merge_trees(ancestor, ours, theirs)`.
- **Keeping writes off disk:** simulation needs to write intermediate trees. Before planning, quickpick adds a **mempack backend** to the repository's object database with the highest priority (`Odb::add_new_mempack_backend`). New objects then go to memory and are discarded on exit. This is what lets the "repository unchanged" requirement include `.git/objects`.
  - *Fallback if mempack misbehaves:* allow unreferenced loose objects. They are invisible to users and removed by `git gc`. This would be recorded as a spec clarification.
- *Alternatives considered:*
  - **Shelling out to `git`:** needs output parsing, and `git merge-tree --write-tree` writes objects to disk.
  - **`gix` (gitoxide):** pure Rust, but its merge and cherry-pick API is less mature than libgit2's.
  - **`cherrypick_commit`:** requires a commit for "ours", which means writing throwaway commits.

### 4. Comparing paths by object ID

- **Path content:** the content of `<path>` at commit C is `C.tree().get_path(path)`, i.e. an entry ID (a tree for a directory, a blob for a file) or "absent". Two states are equal exactly when the entry IDs are equal, including both being absent. This is O(depth) per commit, with no content reads.
- **"Touches the path":** a commit touches `<path>` when its entry ID differs from its first parent's.
- **"Touches outside the path":** computed only for commits that touch the path, using `diff_tree_to_tree(parent, commit)` and collecting deltas whose paths are not under `<path>`. A commit like that has already been inspected, so the cost is limited to the commits that actually matter.

### 5. Range and anchor

```
  mb := merge_base(onto, to)
  T  := entry(onto, path)
  walk from `to` (all parents), hiding ancestors of mb, plus mb itself:
      non-merge commits whose entry != parent's entry  -> candidate list
  anchor := newest candidate c with entry(c) == T,
            else mb if entry(mb) == T
  default from := first candidate after the anchor (oldest-first order)
  range := candidates in [from, to], sorted TOPOLOGICAL | REVERSE (oldest first)
```

- **Bounded walk:** hiding `mb`'s ancestors keeps the walk to roughly the commits since the branch-off point. That's tens of thousands for a release branch, not the whole history.
- **No anchor:** if neither a candidate nor `mb` matches `T`, the target has diverged, and the "no anchor" error asks for `--from`.
- **Explicit `--from`:** checked with `graph_descendant_of(to, from)`, or `from == to`. The warning compares `entry(from^1)` with `T`.

*Alternative considered:* following only first parents. That misses the actual package commits in nixpkgs, which sit on the second-parent side of PR merge commits. That's why merge commits themselves are skipped, and the commits they bring in are walked.

### 6. Simulation and classification

```
  state := onto.tree()
  for c in range (oldest first):
      if touches_outside(c):              step SYNC(c, OutsidePaths)
      else:
          idx := merge_trees(c^1.tree(), state, c.tree())
          if idx.has_conflicts():         step SYNC(c, Conflict(paths))
          else:                           step PICK(c); state := idx.write_tree_to(repo)
      on SYNC: state := replace_entry(state, path, entry(c))   # TreeBuilder along path; remove if absent
  merge adjacent SYNCs (keep last commit as target state, union of reasons)
  verify entry(state, path) == entry(to, path)
```

- **Rebuilding the tree:** `replace_entry` rebuilds only the trees along `path`'s components (a handful of `TreeBuilder` writes into mempack).
- **Why merging syncs is cheap:** consecutive SYNCs collapse without re-simulation. A SYNC to the last covered commit produces exactly the same state as applying each SYNC in turn.
- **Ancestor choice:** `merge_trees` uses the commit's own parent as the ancestor. That is exactly how `git cherry-pick` computes a pick, so a predicted clean PICK should behave the same in the git dialect.

### 7. Plan model and renderers

`Plan { source, onto, from, to, anchor: Option<Oid>, steps: Vec<Step>, warnings: Vec<Warning>, verified: bool }`, where `Step` is either `Pick { commit }` or `Sync { target: commit, covered: Vec<(commit, Reason)>, removes_path: bool }`. Renderers are pure functions of `Plan` (plus commit metadata), which keeps them easy to snapshot-test as plain strings.

**Script skeleton, both dialects:** `#!/bin/sh`, `set -eu`, then `cd -- '<absolute repo path>'`. All interpolated values are single-quoted, with embedded `'` escaped. Messages are passed through quoted here-documents (`<<'QUICKPICK_EOF'`) so no shell expansion happens.

| Step | git dialect | jujutsu dialect |
|---|---|---|
| start | `git switch -c '<branch>' '<onto>'` | `jj new '<onto>'` |
| PICK | `git cherry-pick -x <sha>` | `jj duplicate <sha> -B @` then `jj describe @- -m "$(cat <<'QUICKPICK_EOF' … )"` |
| SYNC (path exists) | `git rm -r -q --ignore-unmatch -- '<path>'`, then `git checkout <sha> -- '<path>'`, then `git commit -F - <<'QUICKPICK_EOF'` | `jj restore --from <sha> 'root:"<path>"'`, then `jj describe -m …`, then `jj new` |
| SYNC (path removed) | `git rm -r -q -- '<path>'` then commit | same `jj restore` (it deletes), then describe and new |

Implementation notes on these choices:
- **`git rm` before `checkout`:** `git checkout <sha> -- <path>` works in overlay mode. On its own it would leave behind files that exist on the target but not at `<sha>`, so the path is removed first.
- **The `root:` fileset in jj:** it anchors the path to the repository root regardless of the current directory.
- **Message for jj picks:** quickpick writes it itself (the original message plus the `-x` trailer), because `jj duplicate` in jj 0.46 has no `-m`. Using `--config templates.duplicate_description=…` instead would save a command, but it depends on whether `commit_id` refers to the original commit in that template, which is still open.

### 8. Output streams and exit codes

The plan or script goes to stdout. Warnings and errors go to stderr. The exit code is 0 on success, including "nothing to do", 1 for planning errors (no anchor, verification failed, revision not on source), and 2 for usage errors, which clap already uses for those.

### 9. Testing approach

- **Fixture repositories:** tests build them with `git2` in a temporary directory (dev-dependency `tempfile`). A small builder helper creates a source and target branch with chosen commits: in-path, outside-touching, conflicting, a merge commit, and path deletion.
- **Unit tests:** cover anchor search, range validation, classification, sync merging, verification, message rendering, and shell quoting.
- **Integration tests:** run the binary (`CARGO_BIN_EXE_qp`) against fixtures. One test executes the generated **git** script with `sh` in a throwaway clone and asserts that the path at the branch tip equals the path at `--to`. The equivalent jj test runs only when `jj` is on `PATH`, and is skipped otherwise.

## Risks / Trade-offs

- **jj and libgit2 disagree on conflicts:** in the jujutsu dialect, jj's merge may produce a conflicted change where libgit2 predicted a clean pick. jj records conflicts rather than stopping. → `propose` notes this when `vcs = jujutsu`, and the script ends with `jj log -r 'conflicts()'` so the user can check for conflicted changes.
- **Evil merges:** a merge commit that changes `<path>` itself (e.g. a conflict resolved inside a merge) is skipped, so verification fails. → The error explains the likely cause and suggests a narrower `--to`. A final catch-up SYNC could be added later.
- **Shallow or partial clones:** missing history leads to "no anchor" or an unresolved revision. → Error messages mention shallow clones as a possible cause.
- **Merge performance:** `merge_trees` across whole nixpkgs trees is fast in practice, because libgit2 skips identical subtrees. That's unmeasured on real data. → Measure against a real nixpkgs clone during the integration task, and narrow the merge to the path's subtree if it's too slow.
- **clapfig version drift:** the pinned 0.21.4 may differ from the documented 0.26 API. → Verify first, and upgrade with `cargo add` if needed.
- **Mempack behaviour:** `add_new_mempack_backend` may be awkward to use from git2. → The fallback in decision 3 applies.
- **Unused dependency:** `url` is currently declared but unused. → Remove it with `cargo remove url`, unless clapfig's URL feature turns out to need it.

## Open Questions

- In jj's `templates.duplicate_description`, does `commit_id` refer to the original commit? This only matters for the one-command optimisation. The two-command form in decision 7 is the baseline.
- What should the exact column layout and wording of `propose` output be? The spec fixes the required fields, so the layout can be settled during implementation.
