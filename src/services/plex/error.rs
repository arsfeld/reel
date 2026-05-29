#[derive(Debug, thiserror::Error)]
pub enum PlexError {
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
