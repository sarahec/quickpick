//! Error type and exit-code mapping.

use std::fmt;

/// Errors reported by quickpick.
#[derive(Debug)]
pub enum Error {
    /// Configuration loading or validation failed.
    Config(String),
    /// The required `onto` setting was not provided by any layer.
    MissingOnto,
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
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_onto_names_every_source() {
        let msg = Error::MissingOnto.to_string();
        assert!(msg.contains("--onto"), "{msg}");
        assert!(msg.contains("QUICKPICK__ONTO"), "{msg}");
        assert!(msg.contains("quickpick.toml"), "{msg}");
    }
}
