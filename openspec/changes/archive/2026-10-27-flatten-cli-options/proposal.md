# Proposal

## Why

The v1 interface grew organically: three subcommands (`propose`, `generate`,
`configure`), six shorthands, and per-range flags that predate the bound
grammar. The tool's actual job is one thing — emit a script — so the
subcommand split forces a decision the user shouldn't have to make, and the
`configure` UI duplicates what a documented config file does better.

## What Changes

- **BREAKING**: Remove all subcommands and shorthands (`propose`/`p`,
  `generate`/`g`, `configure`/`c`/`config`). `qp <path>` emits the script.
- **BREAKING**: Add `--explain` (today's full plan output) and `--summary`
  (condensed plan) output-mode flags, mutually exclusive; the default mode
  emits the script.
- **BREAKING**: Replace `--from`/`--to` with `--range`/`-r <a..b>` using git
  semantics (`a` exclusive). Each side accepts the bound grammar
  `branch|commit|branch@commit`; either side may be empty.
- **BREAKING**: `--onto` accepts the same bound grammar, so
  `release-26.05@abc` asserts the target commit lives on that line.
- **BREAKING**: `--source` loses its dedicated meaning as a range selector;
  the bound names the source line. `source` survives as a config key
  (default `master`) used for bare-commit bounds and the anchor search.
- Add `--show-config`, which prints the merged settings.
- Remove the `configure` subcommand; `quickpick.toml` is hand-edited.
- Bump the version to 0.2.0 to signal the breaking CLI change.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `quickpick-cli`: the command surface is flattened (no subcommands), output
  becomes a mode rather than a subcommand, range selection moves to `a..b`
  bounds that can name their branch line, configuration gains
  `--show-config`, and the configure UI is removed.

## Impact

- `src/cli.rs`: subcommand enum replaced by a flat flag set.
- `src/config.rs`: unchanged except for supporting `--show-config`.
- `src/main.rs`: dispatches on output mode instead of subcommand.
- `src/plan.rs` / `src/repo.rs`: range bounds parsed from `a..b` with the
  `branch|commit|branch@commit` grammar; `a` becomes exclusive, which
  simplifies the anchor default.
- `src/render/*`: add the condensed `--summary` output.
- `README.md`, `openspec/specs/quickpick-cli/spec.md`: surface docs and
  spec rewritten.
- Tests: subcommand/alias parsing tests replaced by flag/mode tests.
