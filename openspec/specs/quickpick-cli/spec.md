# quickpick-cli Specification

## Purpose

Plans how to bring exactly one path in a nixpkgs checkout from a source revision to a target revision, using cherry-picks where possible and explained sync commits otherwise, and presents that plan as an explanation or as a git or jujutsu shell script without modifying the repository.

## Requirements

### Requirement: Executable name
The quickpick package SHALL install its command-line executable under the name `qp`.

#### Scenario: Invoking the executable
- **WHEN** the user runs `qp --help` after installing the package
- **THEN** the help text SHALL describe quickpick and list its subcommands

### Requirement: Subcommands
quickpick SHALL provide the subcommands `propose` (explain the plan), `generate` (print a shell script for the plan) and `configure` (manage the configuration file). `propose` and `generate` SHALL accept the same planning options and SHALL compute the same plan for the same inputs.

#### Scenario: Propose and generate agree
- **WHEN** the user runs `qp propose` and `qp generate` with identical arguments against the same repository state
- **THEN** both SHALL describe the same ordered steps covering the same source commits

#### Scenario: No subcommand
- **WHEN** the user runs `qp` with no subcommand
- **THEN** quickpick SHALL print usage information and exit with a non-zero status

### Requirement: Subcommand shorthands
Each subcommand SHALL also be accepted under a shorthand: `p` for `propose`, `g` for `generate`, and `c` for `configure`. `configure` SHALL also be accepted as `config`. Help output SHALL show the shorthands.

#### Scenario: Shorthand behaves like the full name
- **WHEN** the user runs `qp g --onto release-26.05 pkgs/by-name/op/openai`
- **THEN** the output SHALL be identical to `qp generate --onto release-26.05 pkgs/by-name/op/openai`

#### Scenario: Configure shorthand
- **WHEN** the user runs `qp c list`
- **THEN** quickpick SHALL behave as `qp configure list`

#### Scenario: Shorthands shown in help
- **WHEN** the user runs `qp --help`
- **THEN** the help text SHALL show `p`, `g` and `c` as aliases of their subcommands

### Requirement: Path input
`propose` and `generate` SHALL require exactly one positional argument: a repository-relative path to a file or directory. Only that path is in scope: the plan SHALL NOT change any file outside it.

#### Scenario: Directory path
- **WHEN** the user runs `qp propose --onto release-26.05 pkgs/by-name/op/openai`
- **THEN** quickpick SHALL plan changes for every file under `pkgs/by-name/op/openai` and for no other file

#### Scenario: Missing path
- **WHEN** the user runs `qp propose --onto release-26.05` without a path
- **THEN** quickpick SHALL print usage information and exit with a non-zero status

#### Scenario: Path absent from source at --to
- **WHEN** the path does not exist at the `--to` commit and never existed between `--from` and `--to`
- **THEN** quickpick SHALL report that the path is not found on the source and exit with a non-zero status

### Requirement: Setting precedence
quickpick SHALL resolve each setting (`repo`, `source`, `onto`, `vcs`, `branch`) from, in decreasing precedence: the CLI flag, the `QUICKPICK__<SETTING>` environment variable, the `quickpick.toml` configuration file, and the built-in default.

#### Scenario: Flag overrides environment
- **WHEN** `QUICKPICK__SOURCE=staging` is set and the user passes `--source master`
- **THEN** quickpick SHALL use `master` as the source

#### Scenario: Environment overrides config file
- **WHEN** `quickpick.toml` sets `source = "staging-next"` and `QUICKPICK__SOURCE=staging` is set, with no `--source` flag
- **THEN** quickpick SHALL use `staging` as the source

#### Scenario: Config file overrides default
- **WHEN** `quickpick.toml` sets `vcs = "jujutsu"` and neither a flag nor an environment variable sets `vcs`
- **THEN** quickpick SHALL use the jujutsu dialect

### Requirement: Repository selection
quickpick SHALL operate on the git repository given by the `repo` setting, defaulting to the current working directory. A jujutsu repository colocated with git SHALL be accepted. quickpick SHALL NOT modify the repository, its refs or its working tree.

#### Scenario: Not a repository
- **WHEN** the `repo` setting points to a directory that is not a git repository
- **THEN** quickpick SHALL report that the path is not a git repository and exit with a non-zero status

#### Scenario: Repository unchanged
- **WHEN** quickpick finishes `propose` or `generate`
- **THEN** the repository's refs, index and working tree SHALL be unchanged

### Requirement: Target revision is explicit
The `onto` setting SHALL be required and SHALL accept any revision expression git can resolve (a branch, remote-tracking branch, tag or commit). quickpick SHALL NOT infer a default target.

#### Scenario: Target not provided
- **WHEN** no `--onto` flag, `QUICKPICK__ONTO` variable or `onto` config entry is set
- **THEN** quickpick SHALL report that a target is required, explain how to set it, and exit with a non-zero status

#### Scenario: Target does not resolve
- **WHEN** `--onto upstream/release-99.99` does not resolve to a commit
- **THEN** quickpick SHALL report the unresolved revision and exit with a non-zero status

### Requirement: Source revision
The `source` setting SHALL accept any revision expression git can resolve and SHALL default to exactly `master`, with no fallback to remote-tracking branches.

#### Scenario: Default source
- **WHEN** no source is configured and a local `master` exists
- **THEN** quickpick SHALL use the local `master` as the source

#### Scenario: Default source missing
- **WHEN** no source is configured and no ref named `master` resolves, even though `upstream/master` exists
- **THEN** quickpick SHALL report that `master` does not resolve and exit with a non-zero status

### Requirement: Range end
`--to` SHALL name the last source commit to copy. When omitted, it SHALL default to the newest commit reachable from the source that touches the path. A given `--to` SHALL be reachable from the source.

#### Scenario: Default --to
- **WHEN** `--to` is omitted
- **THEN** the plan SHALL bring the path to its state at the newest source commit touching it

#### Scenario: --to not on source
- **WHEN** `--to` names a commit that is not reachable from the source
- **THEN** quickpick SHALL report that the commit is not on the source and exit with a non-zero status

### Requirement: Range start and anchor detection
The anchor SHALL be the newest commit reachable from `--to` that touches the path and whose path content equals the path on the target. `--from` is inclusive, defaults to the first path-touching commit after the anchor, and SHALL be an ancestor of `--to`.

#### Scenario: Anchor found
- **WHEN** `--from` is omitted and source commit A's path content equals the target's
- **THEN** the range SHALL start at the first path-touching commit after A

#### Scenario: No anchor and no --from
- **WHEN** `--from` is omitted and no source commit's path content equals the target's
- **THEN** quickpick SHALL report that no common state was found, ask for `--from`, and exit with a non-zero status

#### Scenario: Explicit --from disagrees with target
- **WHEN** `--from` is given and the path's content just before `--from` differs from the target's
- **THEN** quickpick SHALL print a warning and continue planning from `--from`

#### Scenario: Target already in sync
- **WHEN** the path content on the target equals its content at `--to`
- **THEN** `propose` SHALL report that nothing needs to be done, `generate` SHALL print no steps, and quickpick SHALL exit successfully

#### Scenario: --from not before --to
- **WHEN** `--from` is not an ancestor of `--to`
- **THEN** quickpick SHALL report the invalid range and exit with a non-zero status

### Requirement: Step classification
For each non-merge commit in the range that touches the path, oldest first, quickpick SHALL plan a PICK if the commit touches only the path and applies cleanly to the simulated state. Otherwise it SHALL plan a SYNC that sets the path to its content at that commit.

#### Scenario: Clean in-path commit
- **WHEN** a commit touches only files within the path and applies without conflict to the state left by the previous steps
- **THEN** quickpick SHALL plan a PICK for it

#### Scenario: Commit touching files outside the path
- **WHEN** a commit touches files within the path and also files outside it
- **THEN** quickpick SHALL plan a SYNC for it, recording the reason and the outside files it touched

#### Scenario: Conflicting commit
- **WHEN** a commit touches only the path but conflicts with the simulated state
- **THEN** quickpick SHALL plan a SYNC for it, recording the reason and the conflicting files

#### Scenario: Planning continues after a sync
- **WHEN** a SYNC is planned for a commit and later commits apply cleanly on top of the path's state at that commit
- **THEN** quickpick SHALL plan PICKs for those later commits

#### Scenario: Merge commits
- **WHEN** a merge commit in the range touches the path
- **THEN** quickpick SHALL NOT plan a step for the merge commit itself

### Requirement: Consecutive syncs are merged
Adjacent SYNC steps SHALL be combined into a single SYNC that sets the path to its content at the last covered commit and records every covered commit with its reason.

#### Scenario: Two adjacent syncs
- **WHEN** commits c3 and c4 are both classified as SYNC and are adjacent in the plan
- **THEN** the plan SHALL contain one SYNC to the path's state at c4 that lists both c3 and c4

### Requirement: Plan verification
quickpick SHALL verify that applying the planned steps to the target yields path content equal to the path at `--to`. If verification fails, quickpick SHALL report the failure and exit with a non-zero status. `generate` SHALL NOT print a script that failed verification.

#### Scenario: Verified plan
- **WHEN** the simulated result matches the path at `--to`
- **THEN** `propose` SHALL report successful verification and `generate` SHALL print the script

#### Scenario: Verification fails
- **WHEN** the simulated result differs from the path at `--to`
- **THEN** quickpick SHALL report the differing files and exit with a non-zero status without printing a script

### Requirement: Shared definition file warning
When a commit in the range also touches a shared package-definition file (`pkgs/top-level/all-packages.nix`, `pkgs/top-level/python-packages.nix`, or another file under `pkgs/top-level/`), quickpick SHALL warn that the target may also need that file's change, because quickpick does not copy it.

#### Scenario: Commit touches python-packages.nix
- **WHEN** a commit in the range touches `pkgs/top-level/python-packages.nix` in addition to the path
- **THEN** quickpick SHALL print a warning naming the commit and the file

### Requirement: Propose output
`propose` SHALL print, to stdout, each planned step in order with its kind (PICK or SYNC), commit SHA, author date and summary, plus each SYNC's reasons. It SHALL also print the resolved source, target, range, anchor and verification result.

#### Scenario: Mixed plan
- **WHEN** the plan contains two PICKs and one SYNC
- **THEN** `propose` SHALL list three steps in order, and the SYNC entry SHALL include the commits it covers and why each could not be picked

### Requirement: Generate output
`generate` SHALL print a self-contained POSIX shell script that stops at the first failing command. It SHALL begin by creating the starting point at the target, then perform each step in plan order. quickpick SHALL NOT execute the script.

#### Scenario: Script is self-contained
- **WHEN** the user runs `qp generate --onto upstream/release-26.05 pkgs/by-name/op/openai`
- **THEN** the script's first step SHALL create the starting point at `upstream/release-26.05` before any pick or sync

#### Scenario: Script is not executed
- **WHEN** `generate` completes
- **THEN** no commit, branch or working-copy change SHALL have been made in the repository

### Requirement: VCS dialect
The `vcs` setting SHALL select the script dialect, either `git` (default) or `jujutsu`. Any other value SHALL be rejected with an error listing the allowed values.

#### Scenario: Git dialect
- **WHEN** `vcs` is `git`
- **THEN** the script SHALL start with `git switch -c <branch> <onto>`, use `git cherry-pick -x <sha>` for each PICK, and for each SYNC restore the path from the commit and create a commit with the sync message

#### Scenario: Jujutsu dialect
- **WHEN** `vcs` is `jujutsu`
- **THEN** the script SHALL start with `jj new <onto>`, use `jj duplicate` placed before the working-copy change for each PICK, use `jj restore --from <sha> <path>` for each SYNC, and set each resulting change's description

#### Scenario: Invalid dialect
- **WHEN** `vcs` is `svn`
- **THEN** quickpick SHALL report the invalid value, list `git` and `jujutsu`, and exit with a non-zero status

### Requirement: Branch name for the git dialect
In the git dialect, the script SHALL create the branch named by the `branch` setting. It SHALL default to `quickpick/<last component of the path>`. The jujutsu dialect SHALL ignore this setting.

#### Scenario: Default branch name
- **WHEN** `vcs` is `git`, no branch is configured and the path is `pkgs/by-name/op/openai`
- **THEN** the script SHALL create the branch `quickpick/openai`

### Requirement: Cherry-pick message convention
Every PICK SHALL produce a change whose description is the original commit message, followed by a blank line and `(cherry picked from commit <full sha>)`, in both dialects.

#### Scenario: Jujutsu pick message
- **WHEN** `vcs` is `jujutsu` and the plan picks commit `79af03b4e99b57f02aacd6c5b762ec02a76193f6` with summary `openai: 1.4.0 -> 1.4.1`
- **THEN** the resulting change's description SHALL be the original message followed by a blank line and `(cherry picked from commit 79af03b4e99b57f02aacd6c5b762ec02a76193f6)`

### Requirement: Sync commit message
Every SYNC SHALL produce a change whose description starts with `<last path component>: sync to <short sha> (<summary of last covered commit>)`. The body SHALL state that it was not cherry-picked, name the path and the full source SHA, and list each covered commit with SHA, summary and reason.

#### Scenario: Sync covering an outside-touching commit
- **WHEN** a SYNC covers commit `4d5e6f7` (`openai: 1.4.0 -> 1.4.1`), which also touched `pkgs/top-level/python-packages.nix`
- **THEN** the message summary SHALL be `openai: sync to 4d5e6f7 (openai: 1.4.0 -> 1.4.1)` and the body SHALL list `4d5e6f7` with the reason that it touches `pkgs/top-level/python-packages.nix`

### Requirement: Path deletion during a sync
When the path does not exist at a SYNC's commit, the SYNC SHALL remove the path on the target instead of restoring it.

#### Scenario: Package removed on source
- **WHEN** a SYNC targets a commit at which the path has been deleted
- **THEN** the generated step SHALL remove the path (for git, with `git rm -r`) and record the removal in the sync message

### Requirement: Error reporting
quickpick SHALL write warnings and errors to stderr and keep stdout for the plan or script. Every error SHALL produce a non-zero exit status and a message naming the offending input.

#### Scenario: Warnings do not pollute the script
- **WHEN** `generate` emits a shared definition file warning
- **THEN** the warning SHALL appear on stderr and the script on stdout SHALL remain valid shell
