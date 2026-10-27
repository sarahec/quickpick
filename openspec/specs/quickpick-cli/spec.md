# quickpick-cli Specification

## Purpose

Plans how to bring exactly one path in a nixpkgs checkout from a source revision to a target revision, using cherry-picks where possible and explained sync commits otherwise, and presents that plan as an explanation or as a git or jujutsu shell script without modifying the repository.

## Requirements

### Requirement: Executable name
The quickpick package SHALL install its command-line executable under the name `qp`.

#### Scenario: Invoking the executable
- **WHEN** the user runs `qp --help` after installing the package
- **THEN** the help text SHALL describe quickpick and list its options and output modes

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

### Requirement: Show configuration
`--show-config` SHALL print each merged setting as `key = value` and exit without planning or requiring a target.

#### Scenario: Merged values shown
- **WHEN** `quickpick.toml` sets `vcs = "jujutsu"` and the user runs `qp --show-config`
- **THEN** the output SHALL include `vcs = jujutsu` alongside the other merged settings

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
quickpick SHALL verify that applying the planned steps to the target yields path content equal to the path at the range end. If verification fails, quickpick SHALL report the failure and exit with a non-zero status. Script mode SHALL NOT print a script that failed verification.

#### Scenario: Verified plan
- **WHEN** the simulated result matches the path at the range end
- **THEN** explain mode SHALL report successful verification and script mode SHALL print the script

#### Scenario: Verification fails
- **WHEN** the simulated result differs from the path at the range end
- **THEN** quickpick SHALL report the differing files and exit with a non-zero status without printing a script

### Requirement: Shared definition file warning
When a commit in the range also touches a shared package-definition file (`pkgs/top-level/all-packages.nix`, `pkgs/top-level/python-packages.nix`, or another file under `pkgs/top-level/`), quickpick SHALL warn that the target may also need that file's change, because quickpick does not copy it.

#### Scenario: Commit touches python-packages.nix
- **WHEN** a commit in the range touches `pkgs/top-level/python-packages.nix` in addition to the path
- **THEN** quickpick SHALL print a warning naming the commit and the file

### Requirement: Explain output
`--explain` SHALL print, to stdout, each planned step in order with its kind (PICK or SYNC), commit SHA, author date and summary, plus each SYNC's reasons. It SHALL also print the resolved source, target, range, anchor and verification result.

#### Scenario: Mixed plan
- **WHEN** the plan contains two PICKs and one SYNC
- **THEN** `--explain` SHALL list three steps in order, and the SYNC entry SHALL include the commits it covers and why each could not be picked

### Requirement: Summary output
`--summary` SHALL print, to stdout, the resolved source and target revisions, the anchor and range, each planned step on one numbered line with its kind (PICK or SYNC) and short commit id, and the verification verdict. It SHALL NOT print covered-commit details or reasons.

#### Scenario: Condensed mixed plan
- **WHEN** the plan contains two PICKs and one SYNC
- **THEN** `--summary` SHALL list three numbered steps with kind and short commit id, and end with the verification verdict

#### Scenario: Summary covers a sync
- **WHEN** a SYNC covers two commits
- **THEN** its single line SHALL show the kind SYNC and the commit it syncs to, without listing each covered commit's reason

### Requirement: Script output
Script mode (the default) SHALL print a self-contained POSIX shell script that stops at the first failing command. It SHALL begin by creating the starting point at the target, then perform each step in plan order. quickpick SHALL NOT execute the script.

#### Scenario: Script is self-contained
- **WHEN** the user runs `qp --onto upstream/release-26.05 pkgs/by-name/op/openai`
- **THEN** the script's first step SHALL create the starting point at `upstream/release-26.05` before any pick or sync

#### Scenario: Script is not executed
- **WHEN** script mode completes
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
- **WHEN** script mode emits a shared definition file warning
- **THEN** the warning SHALL appear on stderr and the script on stdout SHALL remain valid shell
