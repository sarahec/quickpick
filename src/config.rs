//! Layered configuration via clapfig.
//!
//! Settings resolve in the order CLI flag > `QUICKPICK__<KEY>` environment
//! variable > `quickpick.toml` (platform config directory) > built-in
//! default.

use std::path::PathBuf;

use clapfig::{Clapfig, Schema, TypedBuilder};
use serde::{Deserialize, Serialize};

use crate::cli::Cli;
use crate::error::Error;

/// Script dialect for `generate`.
#[derive(Schema, Serialize, Deserialize, clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Vcs {
    /// Plain git: `git switch -c`, `git cherry-pick -x`, `git rm`/`checkout`/commit.
    Git,
    /// Jujutsu colocated with git: `jj new`, `jj duplicate`, `jj restore`.
    Jujutsu,
}

/// The `quickpick.toml` settings schema.
#[derive(Schema, Serialize, Deserialize, Debug)]
pub struct Settings {
    /// Repository to operate on (default: current working directory).
    #[clapfig(value)]
    pub repo: Option<PathBuf>,
    /// Source revision the path is taken from.
    #[clapfig(default = "master")]
    pub source: String,
    /// Target revision the path is brought onto (required).
    pub onto: Option<String>,
    /// Script dialect.
    #[clapfig(default = "git")]
    pub vcs: Vcs,
    /// Branch name for the git dialect (default `quickpick/<last path component>`).
    pub branch: Option<String>,
}

/// A clapfig builder preconfigured for quickpick.
pub fn builder() -> TypedBuilder<Settings> {
    Clapfig::typed::<Settings>()
        .app_name("quickpick")
        .env_prefix("QUICKPICK")
}

/// Load settings, applying the CLI flags in `args` as the top layer.
///
/// Fails with [`Error::MissingOnto`] when no layer provides a target.
pub fn load(args: &Cli) -> Result<Settings, Error> {
    load_from(builder(), args)
}

/// Like [`load`], but on a caller-configured builder (used by tests to
/// isolate file and environment layers).
fn load_from(b: TypedBuilder<Settings>, args: &Cli) -> Result<Settings, Error> {
    let settings = b
        .cli_overrides_from(args)
        .load()
        .map_err(|e| Error::Config(e.to_string()))?;
    if settings.onto.is_none() {
        return Err(Error::MissingOnto);
    }
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serializes tests that mutate the process-global environment.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard(&'static str);

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            unsafe { std::env::set_var(key, value) };
            Self(key)
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            unsafe { std::env::remove_var(self.0) };
        }
    }

    fn base_args() -> Cli {
        Cli {
            path: Some("pkgs/x".into()),
            explain: false,
            summary: false,
            range: None,
            onto: None,
            source: None,
            vcs: None,
            branch: None,
            repo: None,
            show_config: false,
        }
    }

    #[test]
    fn defaults_apply_with_no_layers() {
        let settings = builder().no_env().load().unwrap();
        assert_eq!(settings.source, "master");
        assert_eq!(settings.vcs, Vcs::Git);
        assert!(settings.onto.is_none());
        assert!(settings.repo.is_none());
        assert!(settings.branch.is_none());
    }

    #[test]
    fn file_overrides_default_and_flag_overrides_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("quickpick.toml"),
            "vcs = \"jujutsu\"\nsource = \"staging-next\"\n",
        )
        .unwrap();
        let b = || {
            builder()
                .no_env()
                .search_paths(vec![clapfig::SearchPath::Path(dir.path().to_path_buf())])
        };
        let from_file = b().load().unwrap();
        assert_eq!(from_file.vcs, Vcs::Jujutsu);
        assert_eq!(from_file.source, "staging-next");

        let mut args = base_args();
        args.vcs = Some(Vcs::Git);
        args.source = Some("main".into());
        let from_flag = b().cli_overrides_from(&args).load().unwrap();
        assert_eq!(from_flag.vcs, Vcs::Git);
        assert_eq!(from_flag.source, "main");
    }

    #[test]
    fn env_overrides_file_and_flag_overrides_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::set("QUICKPICK__SOURCE", "staging");
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("quickpick.toml"),
            "source = \"staging-next\"\n",
        )
        .unwrap();
        let b =
            || builder().search_paths(vec![clapfig::SearchPath::Path(dir.path().to_path_buf())]);
        assert_eq!(b().load().unwrap().source, "staging");

        let mut args = base_args();
        args.source = Some("master".into());
        assert_eq!(
            b().cli_overrides_from(&args).load().unwrap().source,
            "master"
        );
    }

    #[test]
    fn unset_flags_do_not_clobber_lower_layers() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("quickpick.toml"),
            "onto = \"release-26.05\"\nsource = \"staging-next\"\nvcs = \"jujutsu\"\n",
        )
        .unwrap();
        let settings = load_from(
            builder()
                .no_env()
                .search_paths(vec![clapfig::SearchPath::Path(dir.path().to_path_buf())]),
            &base_args(),
        )
        .unwrap();
        assert_eq!(settings.onto.as_deref(), Some("release-26.05"));
        assert_eq!(settings.source, "staging-next");
        assert_eq!(settings.vcs, Vcs::Jujutsu);
    }

    #[test]
    fn invalid_vcs_lists_allowed_values() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("quickpick.toml"), "vcs = \"svn\"\n").unwrap();
        let err = builder()
            .no_env()
            .search_paths(vec![clapfig::SearchPath::Path(dir.path().to_path_buf())])
            .load()
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("git"), "{msg}");
        assert!(msg.contains("jujutsu"), "{msg}");
    }

    #[test]
    fn load_rejects_missing_onto() {
        let err = load_from(builder().no_env(), &base_args()).unwrap_err();
        assert!(matches!(err, Error::MissingOnto));
    }

    #[test]
    fn load_accepts_onto_from_flag() {
        let mut args = base_args();
        args.onto = Some("release-26.05".into());
        let settings = load_from(builder().no_env(), &args).unwrap();
        assert_eq!(settings.onto.as_deref(), Some("release-26.05"));
    }
}
