//! Layered configuration via clapfig.

/// Script dialect for `generate`.
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vcs {
    /// Plain git: `git switch -c`, `git cherry-pick -x`, `git rm`/`checkout`/commit.
    Git,
    /// Jujutsu colocated with git: `jj new`, `jj duplicate`, `jj restore`.
    Jujutsu,
}
