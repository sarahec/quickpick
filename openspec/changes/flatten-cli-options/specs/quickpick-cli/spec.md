# Spec Delta

## ADDED Requirements

### Requirement: Output modes
quickpick SHALL run in one of three output modes: script (the default), explain (`--explain`) and summary (`--summary`). `--explain` and `--summary` SHALL be mutually exclusive.

#### Scenario: Default mode emits the script
- **WHEN** the user runs `qp --onto release-26.05 pkgs/by-name/he/hello` with no mode flag
- **THEN** quickpick SHALL print the plan's script in the configured dialect

#### Scenario: Explain mode
- **WHEN** the user runs `qp --explain --onto release-26.05 pkgs/by-name/he/hello`
- **THEN** quickpick SHALL print the detailed plan instead of a script

#### Scenario: Summary mode
- **WHEN** the user runs `qp --summary --onto release-26.05 pkgs/by-name/he/hello`
- **THEN** quickpick SHALL print the condensed plan instead of a script

#### Scenario: Conflicting mode flags
- **WHEN** the user passes both `--explain` and `--summary`
- **THEN** quickpick SHALL report that the flags are mutually exclusive and exit with a non-zero status

### Requirement: Summary output
`--summary` SHALL print, to stdout, the resolved source and target revisions, the anchor and range, each planned step on one numbered line with its kind (PICK or SYNC) and short commit id, and the verification verdict. It SHALL NOT print covered-commit details or reasons.

#### Scenario: Condensed mixed plan
- **WHEN** the plan contains two PICKs and one SYNC
- **THEN** `--summary` SHALL list three numbered steps with kind and short commit id, and end with the verification verdict

#### Scenario: Summary covers a sync
- **WHEN** a SYNC covers two commits
- **THEN** its single line SHALL show the kind SYNC and the commit it syncs to, without listing each covered commit's reason

### Requirement: Commit range
`--range` (short form `-r`) SHALL take a value of the form `<a>..<b>` selecting the path-touching commits reachable from `b` but not from `a` (git semantics: `a` is exclusive). Either side may be empty; a value without `..` SHALL be rejected.

#### Scenario: Explicit range
- **WHEN** the user passes `--range release-26.05..master`
- **THEN** the range SHALL cover path-touching commits reachable from `master` but not from `release-26.05`

#### Scenario: Open-ended range
- **WHEN** the user passes `--range abc123..`
- **THEN** the range SHALL start after `abc123` and end at the range end's default

#### Scenario: Malformed range
- **WHEN** `--range` is given a value without `..`
- **THEN** quickpick SHALL report the malformed range and exit with a non-zero status

### Requirement: Bound syntax
Each `--range` side and `--onto` SHALL accept `branch|commit|branch@commit`. A bare `branch` resolves to its tip. In `branch@commit`, the commit SHALL be resolved and asserted reachable from `branch`. A value that resolves whole as a revision SHALL be used as that revision rather than split.

#### Scenario: Branch bound
- **WHEN** a bound names an existing branch
- **THEN** it SHALL resolve to the branch's tip commit

#### Scenario: Pinned bound on its line
- **WHEN** `--onto release-26.05@abc123` names a commit reachable from `release-26.05`
- **THEN** quickpick SHALL use `abc123` as the target

#### Scenario: Pinned bound off its line
- **WHEN** `--onto release-26.05@abc123` names a commit not reachable from `release-26.05`
- **THEN** quickpick SHALL report that the commit is not on `release-26.05` and exit with a non-zero status

#### Scenario: At-sign inside a ref name
- **WHEN** a bound's whole value resolves as a revision, such as a branch literally named `foo@bar`
- **THEN** it SHALL be used as that revision rather than split at the `@`

### Requirement: Show configuration
`--show-config` SHALL print each merged setting as `key = value` and exit without planning or requiring a target.

#### Scenario: Merged values shown
- **WHEN** `quickpick.toml` sets `vcs = "jujutsu"` and the user runs `qp --show-config`
- **THEN** the output SHALL include `vcs = jujutsu` alongside the other merged settings

## MODIFIED Requirements

### Requirement: Executable name
The quickpick package SHALL install its command-line executable under the name `qp`.

#### Scenario: Invoking the executable
- **WHEN** the user runs `qp --help` after installing the package
- **THEN** the help text SHALL describe quickpick and list its options and output modes

### Requirement: Path input
`qp` SHALL require exactly one positional argument: a repository-relative path to a file or directory. Only that path is in scope: the plan SHALL NOT change any file outside it.

#### Scenario: Directory path
- **WHEN** the user runs `qp --onto release-26.05 pkgs/by-name/op/openai`
- **THEN** quickpick SHALL plan changes for every file under `pkgs/by-name/op/openai` and for no other file

#### Scenario: Missing path
- **WHEN** the user runs `qp --onto release-26.05` without a path
- **THEN** quickpick SHALL print usage information and exit with a non-zero status

#### Scenario: Path absent from source at --to
- **WHEN** the path does not exist at the range end and never existed inside the range
- **THEN** quickpick SHALL report that the path is not found on the source and exit with a non-zero status

### Requirement: Repository selection
quickpick SHALL operate on the git repository given by the `repo` setting, defaulting to the current working directory. A jujutsu repository colocated with git SHALL be accepted. quickpick SHALL NOT modify the repository, its refs or its working tree.

#### Scenario: Not a repository
- **WHEN** the `repo` setting points to a directory that is not a git repository
- **THEN** quickpick SHALL report that the path is not a git repository and exit with a non-zero status

#### Scenario: Repository unchanged
- **WHEN** quickpick finishes in any output mode
- **THEN** the repository's refs, index and working tree SHALL be unchanged

### Requirement: Target revision is explicit
The `onto` setting SHALL be required and SHALL accept the bound syntax (a branch, commit, or `branch@commit` pinning a commit to a branch line). quickpick SHALL NOT infer a default target.

#### Scenario: Target not provided
- **WHEN** no `--onto` flag, `QUICKPICK__ONTO` variable or `onto` config entry is set
- **THEN** quickpick SHALL report that a target is required, explain how to set it, and exit with a non-zero status

#### Scenario: Target does not resolve
- **WHEN** `--onto upstream/release-99.99` does not resolve to a commit
- **THEN** quickpick SHALL report the unresolved revision and exit with a non-zero status

### Requirement: Source revision
The `source` setting SHALL name the default source line and SHALL default to exactly `master`, with no fallback to remote-tracking branches. It SHALL be used to resolve a bare-commit or empty range end and to bound the anchor search; a bound that names a branch supplies its own line.

#### Scenario: Default source
- **WHEN** no source is configured and a local `master` exists
- **THEN** quickpick SHALL use the local `master` as the source line

#### Scenario: Default source missing
- **WHEN** no source is configured, the range end is empty or a bare commit, and no ref named `master` resolves, even though `upstream/master` exists
- **THEN** quickpick SHALL report that `master` does not resolve and exit with a non-zero status

#### Scenario: Branch bound supplies the line
- **WHEN** `--range ..staging-next@abc123` names a commit reachable from `staging-next`
- **THEN** the range end SHALL be `abc123` on the `staging-next` line without consulting the `source` setting

### Requirement: Range end
The `b` bound of `--range` SHALL name the last source commit to copy. When `b` is empty, it SHALL default to the newest commit reachable from the source line that touches the path. A bare-commit `b` SHALL be reachable from the `source` line.

#### Scenario: Default --to
- **WHEN** the range end is empty
- **THEN** the plan SHALL bring the path to its state at the newest source commit touching it

#### Scenario: --to not on source
- **WHEN** a bare-commit range end is not reachable from the source line
- **THEN** quickpick SHALL report that the commit is not on the source and exit with a non-zero status

### Requirement: Range start and anchor detection
The anchor SHALL be the newest commit reachable from the range end that touches the path and whose path content equals the path on the target. An empty range start `a` SHALL default to the anchor, making `a..b` cover the commits after it. An explicit `a` SHALL be exclusive.

#### Scenario: Anchor found
- **WHEN** `a` is empty and source commit A's path content equals the target's
- **THEN** the range SHALL cover the path-touching commits after A

#### Scenario: No anchor and no --from
- **WHEN** `a` is empty and no source commit's path content equals the target's
- **THEN** quickpick SHALL report that no common state was found, ask for an explicit range start, and exit with a non-zero status

#### Scenario: Explicit --from disagrees with target
- **WHEN** `a` is given and the path's content at `a` differs from the target's
- **THEN** quickpick SHALL print a warning and continue planning the commits after `a`

#### Scenario: Target already in sync
- **WHEN** the path content on the target equals its content at the range end
- **THEN** every output mode SHALL report that nothing needs to be done, print no steps or script, and exit successfully

#### Scenario: --from not before --to
- **WHEN** `--range` names an `a` that is not an ancestor of `b`
- **THEN** quickpick SHALL still plan the commits reachable from `b` but not from `a`, rather than rejecting the range

### Requirement: Plan verification
quickpick SHALL verify that applying the planned steps to the target yields path content equal to the path at the range end. If verification fails, quickpick SHALL report the failure and exit with a non-zero status. Script mode SHALL NOT print a script that failed verification.

#### Scenario: Verified plan
- **WHEN** the simulated result matches the path at the range end
- **THEN** explain mode SHALL report successful verification and script mode SHALL print the script

#### Scenario: Verification fails
- **WHEN** the simulated result differs from the path at the range end
- **THEN** quickpick SHALL report the differing files and exit with a non-zero status without printing a script

### Requirement: Explain output
`--explain` SHALL print, to stdout, each planned step in order with its kind (PICK or SYNC), commit SHA, author date and summary, plus each SYNC's reasons. It SHALL also print the resolved source, target, range, anchor and verification result.

#### Scenario: Mixed plan
- **WHEN** the plan contains two PICKs and one SYNC
- **THEN** `--explain` SHALL list three steps in order, and the SYNC entry SHALL include the commits it covers and why each could not be picked

### Requirement: Script output
Script mode (the default) SHALL print a self-contained POSIX shell script that stops at the first failing command. It SHALL begin by creating the starting point at the target, then perform each step in plan order. quickpick SHALL NOT execute the script.

#### Scenario: Script is self-contained
- **WHEN** the user runs `qp --onto upstream/release-26.05 pkgs/by-name/op/openai`
- **THEN** the script's first step SHALL create the starting point at `upstream/release-26.05` before any pick or sync

#### Scenario: Script is not executed
- **WHEN** script mode completes
- **THEN** no commit, branch or working-copy change SHALL have been made in the repository

### Requirement: Error reporting
quickpick SHALL write warnings and errors to stderr and keep stdout for the plan or script. Every error SHALL produce a non-zero exit status and a message naming the offending input.

#### Scenario: Warnings do not pollute the script
- **WHEN** script mode emits a shared definition file warning
- **THEN** the warning SHALL appear on stderr and the script on stdout SHALL remain valid shell

## REMOVED Requirements

### Requirement: Subcommands
**Reason**: The tool has a single job — emitting a script — so mode selection moved to flags (`--explain`, `--summary`) rather than subcommands.
**Migration**: `qp propose ...` becomes `qp --explain ...`; `qp generate ...` becomes `qp ...`; `qp configure <cmd>` is replaced by hand-editing `quickpick.toml` and inspecting the result with `qp --show-config`.

### Requirement: Subcommand shorthands
**Reason**: With no subcommands there is nothing for `p`, `g`, `c` or `config` to alias.
**Migration**: `qp p` → `qp --explain`; `qp g` → `qp`; `qp c`/`qp config` → `qp --show-config` or editing `quickpick.toml` directly.

## RENAMED Requirements

- FROM: `### Requirement: Propose output`
- TO: `### Requirement: Explain output`

- FROM: `### Requirement: Generate output`
- TO: `### Requirement: Script output`
