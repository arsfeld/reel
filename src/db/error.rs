#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum DbError {
    #[error("Diesel error: {0}")]
    Diesel(#[from] diesel::result::Error),

    #[error("Connection error: {0}")]
    Connection(#[from] diesel::ConnectionError),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Data error: {0}")]
    Data(String),
}
