//! Unified error type for fighorse.

use std::fmt;

/// Errors surfaced across the fighorse crate.
#[derive(Debug)]
pub enum Error {
    /// An HTTP/transport level failure talking to Figma.
    Http(reqwest::Error),
    /// A non-2xx response from the Figma REST API.
    Figma {
        status: u16,
        status_text: String,
        body: serde_json::Value,
    },
    /// Request exceeded the configured timeout.
    Timeout(u64),
    /// JSON (de)serialization failure.
    Json(serde_json::Error),
    /// Local filesystem failure.
    Io(std::io::Error),
    /// User/usage error (bad arguments, missing token, etc).
    Usage(String),
    /// Any other error with a message.
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Http(e) => write!(f, "{e}"),
            Error::Figma {
                status,
                status_text,
                ..
            } => write!(f, "Figma API error: {status} {status_text}"),
            Error::Timeout(ms) => write!(f, "Figma API request timed out after {ms}ms"),
            Error::Json(e) => write!(f, "{e}"),
            Error::Io(e) => write!(f, "{e}"),
            Error::Usage(m) => write!(f, "{m}"),
            Error::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            Error::Timeout(0)
        } else {
            Error::Http(e)
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;
