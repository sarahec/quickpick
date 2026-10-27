//! Error type and exit-code mapping.

use std::fmt;

/// Errors reported by quickpick.
///
/// Exit codes follow design decision 8: `0` success, `1` planning or
/// runtime errors, `2` usage errors (clap emits `2` for its own parse
/// failures before `Error` is involved).
#[derive(Debug)]
pub enum Error {
    /// Configuration loading or validation failed.
    Config(String),
    /// The required `onto` setting was not provided by any layer.
    MissingOnto,
    /// A usage error detected after argument parsing.
    Usage(String),
    /// Planning or repository work failed.
    Planning(String),
}

impl Error {
    /// The process exit code for this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 2,
            Self::Config(_) | Self::MissingOnto | Self::Planning(_) => 1,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "configuration error: {msg}"),
            Self::MissingOnto => write!(
                f,
                "a target revision is required: pass --onto, set QUICKPICK__ONTO, \
                 or set the `onto` key in quickpick.toml"
            ),
            Self::Usage(msg) => write!(f, "usage error: {msg}"),
            Self::Planning(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_match_design() {
        assert_eq!(Error::Config("x".into()).exit_code(), 1);
        assert_eq!(Error::MissingOnto.exit_code(), 1);
        assert_eq!(Error::Planning("x".into()).exit_code(), 1);
        assert_eq!(Error::Usage("x".into()).exit_code(), 2);
    }

    #[test]
    fn missing_onto_names_every_source() {
        let msg = Error::MissingOnto.to_string();
        assert!(msg.contains("--onto"), "{msg}");
        assert!(msg.contains("QUICKPICK__ONTO"), "{msg}");
        assert!(msg.contains("quickpick.toml"), "{msg}");
    }
}
