//! Error types for HTTP resource fetching

use std::fmt;

/// Errors that can occur during resource fetching
#[derive(Debug, Clone)]
pub enum FetchError {
    /// Network error (connection failed, timeout, etc.)
    Network(String),
    /// HTTP error (non-2xx status code)
    Http { status: u16, message: String },
    /// Failed to parse response (invalid JSON, etc.)
    Parse(String),
    /// Resource not found (404)
    NotFound(String),
    /// Request was cancelled
    Cancelled,
    /// Authentication required (401)
    Unauthorized,
    /// Access denied (403)
    Forbidden,
    /// The request is well-formed but this build/server pairing cannot
    /// honor it (e.g. an escape-mode config before the server's
    /// render_mode enum knows "escape"). Client-side refusal — nothing
    /// was sent.
    Unsupported(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::Network(msg) => write!(f, "Network error: {}", msg),
            FetchError::Http { status, message } => {
                write!(f, "HTTP error {}: {}", status, message)
            }
            FetchError::Parse(msg) => write!(f, "Parse error: {}", msg),
            FetchError::NotFound(url) => write!(f, "Not found: {}", url),
            FetchError::Cancelled => write!(f, "Request cancelled"),
            FetchError::Unauthorized => write!(f, "Authentication required"),
            FetchError::Forbidden => write!(f, "Access denied"),
            FetchError::Unsupported(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for FetchError {}

impl From<serde_json::Error> for FetchError {
    fn from(err: serde_json::Error) -> Self {
        FetchError::Parse(err.to_string())
    }
}
