#[derive(Debug, thiserror::Error)]
pub enum JellyfinError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Deserialization error: {0}")]
    Deserialize(#[from] serde_json::Error),

    #[error("Unauthorized: invalid or expired token")]
    Unauthorized,

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Server error: {status} - {message}")]
    Server { status: u16, message: String },
}

/// Errors from the transcode-decision / session-lifecycle path. Kept separate
/// from [`JellyfinError`] (mirroring `plex::TranscodeError`) so the
/// `From<…> for SourceError` impl can classify retryable/fallback errors
/// (`Timeout`/`Request` → `Connection`) apart from loud ones
/// (`Server`/`Parse`/`NoDecision` → `Other`).
#[derive(Debug, thiserror::Error)]
pub enum JellyfinTranscodeError {
    #[error("PlaybackInfo request timed out")]
    Timeout,
    #[error("PlaybackInfo request failed: {0}")]
    Request(String),
    #[error("PlaybackInfo server error: {status}")]
    Server { status: u16 },
    #[error("failed to parse PlaybackInfo response: {0}")]
    Parse(String),
    #[error("server returned no playable decision")]
    NoDecision,
}
