use rusqlite::Connection;

use super::DbError;

const SCHEMA_VERSION: i32 = 1;

/// Initialize the database schema. Creates tables if they don't exist.
/// Idempotent: safe to call on every app startup.
pub fn init_db(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sources (
            id TEXT PRIMARY KEY,
            source_type TEXT NOT NULL,
            name TEXT NOT NULL,
            config TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            last_synced_at TEXT
        );

        CREATE TABLE IF NOT EXISTS media_items (
            id TEXT PRIMARY KEY,
            source_type TEXT NOT NULL,
            source_id TEXT NOT NULL,
            external_id TEXT NOT NULL,
            media_type TEXT NOT NULL,
            title TEXT NOT NULL,
            year INTEGER,
            overview TEXT,
            content_rating TEXT,
            rating REAL,
            runtime_minutes INTEGER,
            poster_path TEXT,
            backdrop_path TEXT,
            genres TEXT NOT NULL DEFAULT '[]',
            parent_id TEXT,
            season_number INTEGER,
            episode_number INTEGER,
            air_date TEXT,
            file_path TEXT,
            added_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_media_items_media_type
            ON media_items(media_type);
        CREATE INDEX IF NOT EXISTS idx_media_items_parent_id
            ON media_items(parent_id);
        CREATE INDEX IF NOT EXISTS idx_media_items_source
            ON media_items(source_type, source_id);
        CREATE INDEX IF NOT EXISTS idx_media_items_added_at
            ON media_items(added_at DESC);
        ",
    )?;

    // Set schema version if not already set
    let count: i32 = conn.query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))?;
    if count == 0 {
        conn.execute(
            "INSERT INTO schema_version (version) VALUES (?1)",
            [SCHEMA_VERSION],
        )?;
    }

    Ok(())
}

/// Get the current schema version.
#[allow(dead_code)]
pub fn schema_version(conn: &Connection) -> Result<i32, DbError> {
    let version = conn.query_row("SELECT version FROM schema_version", [], |row| row.get(0))?;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        conn
    }

    #[test]
    fn init_db_succeeds() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(init_db(&conn).is_ok());
    }

    #[test]
    fn init_db_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        init_db(&conn).unwrap(); // second call should not fail
    }

    #[test]
    fn schema_version_is_set() {
        let conn = test_db();
        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn schema_version_not_duplicated_on_reinit() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        init_db(&conn).unwrap();
        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn media_items_table_exists() {
        let conn = test_db();
        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM media_items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn sources_table_exists() {
        let conn = test_db();
        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM sources", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
