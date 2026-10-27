//! Error type and exit-code mapping.

use std::fmt;

/// Errors reported by quickpick.
#[derive(Debug)]
pub enum Error {
    /// Configuration loading or validation failed.
    Config(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "configuration error: {msg}"),
        }
    }
}

impl std::error::Error for Error {}
