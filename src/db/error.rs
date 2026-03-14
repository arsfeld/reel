#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Data error: {0}")]
    Data(String),
}
