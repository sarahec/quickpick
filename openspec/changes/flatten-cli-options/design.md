# Design

## Context

See proposal.md for motivation. The current surface is three subcommands
sharing `PlanArgs`, clapfig layered settings (`repo`, `source`, `onto`,
`vcs`, `branch`), inclusive `--from`/`--to` range bounds that resolve
against the `source` revision, and a `configure` subcommand that edits
`quickpick.toml`. This change flattens the surface without touching the
planner's classification, simulation or rendering internals.

## Goals / Non-Goals

**Goals:**
- One invocation shape: `qp [flags] <path>` with no subcommands.
- `--range a..b` with git-native semantics (`a` exclusive) and a bound
  grammar that can name a branch line.
- `--show-config` as the replacement for the removed `configure` UI.

**Non-Goals:**
- No cross-repository bounds: `branch@commit` names a ref in the *same*
  repository, never a path or remote URL.
- No `a...b` (symmetric-difference) range form.
- No generic `--option <k> <v>` override flag: every settings key already
  has a dedicated flag, so a second spelling would only duplicate them and
  require a conflict rule. It can return later if a key ever lacks a flag.
- Multiple paths remain out of scope.

## Decisions

### Flat argument struct
`Commands` and `PlanArgs` collapse into one `#[derive(Parser)]` struct:
positional `path`, mode flags (`--explain`, `--summary`,
`conflicts_with`), `--range`/`-r`, `--onto`, `--source`, `--vcs`,
`--repo`, `--branch`, and `--show-config`. Deleting the
subcommand enum also removes clapfig's `ConfigArgs` wiring; clapfig is
kept only for `Settings` loading.

### Bound parsing: whole-rev first, then last `@`
`parse_bound(s)` resolves `s` as a git revision first. Only when that
fails does it split at the **last** `@`: the right side must resolve to a
commit, the left side to a branch, and the commit must be reachable from
that branch. This ordering makes `foo@bar` work when `foo@bar` is a real
refname (git allows `@` inside refnames except the `@{n}` reflog form) and
keeps `branch@commit` unambiguous in the common case. A bound records both
the resolved `Oid` and the line it names (`Some(branch)` for `branch` and
`branch@commit`, `None` for a bare commit).

### `a` exclusivity folds into the anchor
Today `--from` is inclusive and defaults to "the first commit after the
anchor". In `a..b` form the floor is exclusive, so the default `a` is
simply the anchor commit itself — the "first commit after" step
disappears and `a..b` literally reads as "the state at `a`, brought to the
state at `b`". An explicit `a` whose path content differs from the
target's still warns (same check, evaluated at `a` instead of just before
`--from`). The old "`--from` must be an ancestor of `--to`" rule is
dropped: `a` and `b` need only resolve; an empty or disjoint range falls
out of plan verification instead.

### `source` is a setting, not a bound
`source` (default `master`, no remote fallback) is used only where a line
isn't otherwise named: to resolve an empty or bare-commit `b` and to bound
the anchor search. A `b` bound of the form `branch`/`branch@commit`
supplies its own line, so `source` is never consulted. The two `--range`
sides may name *different* lines — `release-26.05..master` is the natural
backport idiom.

### `--show-config` bypasses planning
It prints the merged `Settings` as `key = value` lines and exits 0. It
runs before repository opening and does not require `onto`, so it works
anywhere.

### `--summary` reuses the proposal model
The summary renderer consumes the same resolved plan as the explain
renderer (source/target ids, anchor, range, steps, verdict) and emits the
condensed form: one numbered line per step (`1. PICK abc1234`) plus the
verdict. It lives next to the existing explain renderer.

### Version bump
`Cargo.toml` version becomes `0.2.0` alongside the breaking CLI change;
README gains a short old-invocation → new-invocation table.

## Risks / Trade-offs

- **`a..b` inclusivity change**: a v1 `--from X --to Y` command is not
  `--range X..Y` — it's `--range X^..Y`. The spec's migration note and the
  README table state this explicitly; the inclusive floor was an internal
  detail anyway, since most users relied on the anchor default.
- **Broad ranges pick up already-backported commits**:
  `release-26.05..master` includes originals whose `-x` cherry-picks
  already landed on the release branch. Simulation still classifies them
  (mostly extra SYNCs), and plan verification remains the safety net, but
  an explicit range can produce a noisier plan than the anchor default.
  Accepted: the default is unchanged, and `--summary` keeps noise cheap.
- **Refname containing `@` shadows the pin form**: when `foo@bar` exists
  as a ref *and* `bar` resolves, whole-rev resolution wins and the pin
  interpretation is unreachable for that name. Rare, deterministic, and
  documented in the spec.

## Migration Plan

- Bump the crate version to `0.2.0` in the same change.
- README gets a "Migrating from 0.1" table covering subcommand removal,
  `--from`/`--to` → `--range`, and `configure` → `quickpick.toml` +
  `--show-config`.
- The `quickpick.toml` schema is unchanged, so existing config files keep
  working.

## Open Questions

None. (`--range` also takes `-r`; decided after drafting.)
