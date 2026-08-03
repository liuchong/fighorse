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

impl Error {
    /// Return the Figma HTTP status for structured diagnostics.
    pub fn figma_status(&self) -> Option<u16> {
        match self {
            Error::Figma { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// Return only Figma's documented human-readable error fields.
    ///
    /// Arbitrary response bodies are deliberately excluded because they can
    /// contain private upstream details that should not be copied into shared
    /// diagnostics.
    pub fn figma_message(&self) -> Option<&str> {
        match self {
            Error::Figma { body, .. } => body
                .get("message")
                .or_else(|| body.get("err"))
                .and_then(|value| value.as_str())
                .filter(|message| !message.trim().is_empty()),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Http(e) => write!(f, "{e}"),
            Error::Figma {
                status,
                status_text,
                ..
            } => {
                write!(f, "Figma API error: {status} {status_text}")?;
                if let Some(message) = self.figma_message() {
                    write!(f, ": {message}")?;
                }
                Ok(())
            }
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
            // Report the configured timeout rather than a placeholder. Call
            // sites in `http` already convert timeouts explicitly; this keeps
            // any other path (e.g. streaming a response body) accurate too.
            let ms = std::env::var("FIGHORSE_HTTP_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.trim().parse::<u64>().ok())
                .filter(|n| *n > 0)
                .unwrap_or(120_000);
            Error::Timeout(ms)
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
