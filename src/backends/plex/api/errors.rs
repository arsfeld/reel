use thiserror::Error;

/// Typed error enum for Plex API operations
///
/// This enum differentiates between different failure modes to enable
/// appropriate retry strategies and error handling.
#[derive(Error, Debug, Clone)]
pub enum PlexApiError {
    /// Authentication failed (401, 403)
    /// These are permanent errors and should not be retried
    #[error("Authentication failed: {message} (status: {status})")]
    Authentication { status: u16, message: String },

    /// Rate limiting error (429)
    /// Should be retried with exponential backoff
    #[error("Rate limited: {message} (retry after: {retry_after:?}s)")]
    RateLimit {
        message: String,
        retry_after: Option<u64>,
    },

    /// Server error (500+)
    /// Transient errors that should be retried
    #[error("Server error: {message} (status: {status})")]
    ServerError { status: u16, message: String },

    /// Client error (400-499, excluding auth and rate limit)
    /// Usually permanent errors that should not be retried
    #[error("Client error: {message} (status: {status})")]
    ClientError { status: u16, message: String },

    /// Network/connection errors (timeout, connection refused, etc.)
    /// Transient errors that should be retried
    #[error("Network error: {0}")]
    Network(String),

    /// JSON parsing errors
    /// Usually permanent errors
    #[error("Failed to parse response: {0}")]
    ParseError(String),

    /// Generic errors for cases not covered above
    #[error("API error: {0}")]
    Other(String),
}

impl PlexApiError {
    /// Check if this error is transient and should be retried
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            PlexApiError::Network(_)
                | PlexApiError::ServerError { .. }
                | PlexApiError::RateLimit { .. }
        )
    }

    /// Check if this is a rate limit error
    pub fn is_rate_limit(&self) -> bool {
        matches!(self, PlexApiError::RateLimit { .. })
    }

    /// Get the retry-after duration for rate limit errors
    pub fn retry_after(&self) -> Option<u64> {
        match self {
            PlexApiError::RateLimit { retry_after, .. } => *retry_after,
            _ => None,
        }
    }

    /// Create an error from a reqwest error
    pub fn from_reqwest(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            PlexApiError::Network(format!("Request timeout: {}", error))
        } else if error.is_connect() {
            PlexApiError::Network(format!("Connection failed: {}", error))
        } else if error.is_request() {
            PlexApiError::Network(format!("Request error: {}", error))
        } else {
            PlexApiError::Other(error.to_string())
        }
    }

    /// Create an error from an HTTP status code and response
    pub fn from_status(status: u16, body: String) -> Self {
        match status {
            401 | 403 => PlexApiError::Authentication {
                status,
                message: body,
            },
            429 => {
                // Try to extract retry-after header value from the message
                // This is a simple implementation; a real one would parse headers
                PlexApiError::RateLimit {
                    message: body,
                    retry_after: None,
                }
            }
            400..=499 => PlexApiError::ClientError {
                status,
                message: body,
            },
            500..=599 => PlexApiError::ServerError {
                status,
                message: body,
            },
            _ => PlexApiError::Other(format!("HTTP {}: {}", status, body)),
        }
    }
}
