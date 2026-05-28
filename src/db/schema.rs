use rusqlite::Connection;

use super::DbError;

const SCHEMA_VERSION: i32 = 3;

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
            video_resolution TEXT,
            hdr TEXT,
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

        CREATE TABLE IF NOT EXISTS watch_progress (
            media_item_id TEXT PRIMARY KEY,
            position_seconds REAL NOT NULL DEFAULT 0.0,
            duration_seconds REAL NOT NULL DEFAULT 0.0,
            watched INTEGER NOT NULL DEFAULT 0,
            last_watched_at TEXT NOT NULL,
            FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_watch_progress_last_watched
            ON watch_progress(last_watched_at DESC);
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

    // Run migrations
    let current_version = schema_version(conn)?;
    if current_version < 2 {
        migrate_to_v2(conn)?;
    }
    if current_version < 3 {
        migrate_to_v3(conn)?;
    }

    Ok(())
}

/// Migrate schema from v2 to v3: add `video_resolution` and `hdr` columns to
/// `media_items`. Both were previously read into `MediaItem` from Plex but
/// never persisted, so the columns are nullable with no default backfill —
/// existing rows get the values on next Plex sync.
fn migrate_to_v3(conn: &Connection) -> Result<(), DbError> {
    // ADD COLUMN is tolerant if the column already exists in a freshly-created
    // schema (the v3 CREATE TABLE above includes both columns). Catch and
    // ignore the duplicate-column error in that case.
    let _ = conn.execute(
        "ALTER TABLE media_items ADD COLUMN video_resolution TEXT",
        [],
    );
    let _ = conn.execute("ALTER TABLE media_items ADD COLUMN hdr TEXT", []);

    conn.execute("UPDATE schema_version SET version = ?1", [3])?;

    Ok(())
}

/// Migrate schema from v1 to v2: add watch_progress table.
fn migrate_to_v2(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS watch_progress (
            media_item_id TEXT PRIMARY KEY,
            position_seconds REAL NOT NULL DEFAULT 0.0,
            duration_seconds REAL NOT NULL DEFAULT 0.0,
            watched INTEGER NOT NULL DEFAULT 0,
            last_watched_at TEXT NOT NULL,
            FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_watch_progress_last_watched
            ON watch_progress(last_watched_at DESC);
        ",
    )?;

    conn.execute("UPDATE schema_version SET version = ?1", [2])?;

    Ok(())
}

/// Get the current schema version.
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

    #[test]
    fn watch_progress_table_exists() {
        let conn = test_db();
        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM watch_progress", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn migrate_v1_to_v2_adds_watch_progress() {
        // Simulate a v1 database by creating tables without watch_progress
        let conn = Connection::open_in_memory().unwrap();

        // Create v1 schema manually (without watch_progress)
        conn.execute_batch(
            "
            CREATE TABLE schema_version (version INTEGER NOT NULL);
            INSERT INTO schema_version (version) VALUES (1);

            CREATE TABLE media_items (
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

            CREATE TABLE sources (
                id TEXT PRIMARY KEY,
                source_type TEXT NOT NULL,
                name TEXT NOT NULL,
                config TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                last_synced_at TEXT
            );
            ",
        )
        .unwrap();

        assert_eq!(schema_version(&conn).unwrap(), 1);

        // Run init_db which should migrate v1 → v2 → v3 in sequence
        init_db(&conn).unwrap();

        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);

        // Verify watch_progress table exists (the v2 migration's contribution)
        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM watch_progress", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn migrate_v2_to_v3_adds_hdr_and_video_resolution_columns() {
        // Build a v2 schema by hand (no video_resolution, no hdr).
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE schema_version (version INTEGER NOT NULL);
            INSERT INTO schema_version (version) VALUES (2);

            CREATE TABLE media_items (
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

            CREATE TABLE sources (
                id TEXT PRIMARY KEY,
                source_type TEXT NOT NULL,
                name TEXT NOT NULL,
                config TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                last_synced_at TEXT
            );

            CREATE TABLE watch_progress (
                media_item_id TEXT PRIMARY KEY,
                position_seconds REAL NOT NULL DEFAULT 0.0,
                duration_seconds REAL NOT NULL DEFAULT 0.0,
                watched INTEGER NOT NULL DEFAULT 0,
                last_watched_at TEXT NOT NULL,
                FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE CASCADE
            );
            ",
        )
        .unwrap();

        assert_eq!(schema_version(&conn).unwrap(), 2);

        // Insert a row to confirm the migration is non-destructive.
        conn.execute(
            "INSERT INTO media_items (
                id, source_type, source_id, external_id, media_type, title,
                added_at, updated_at
             ) VALUES ('m1', 'plex', 'http://test', 'k1', 'movie', 'Dune', '0', '0')",
            [],
        )
        .unwrap();

        init_db(&conn).unwrap();

        assert_eq!(schema_version(&conn).unwrap(), 3);

        // Pre-existing row survives.
        let title: String = conn
            .query_row(
                "SELECT title FROM media_items WHERE id = 'm1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(title, "Dune");

        // New columns exist and default to NULL.
        let hdr: Option<String> = conn
            .query_row(
                "SELECT hdr FROM media_items WHERE id = 'm1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(hdr, None);
        let vr: Option<String> = conn
            .query_row(
                "SELECT video_resolution FROM media_items WHERE id = 'm1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(vr, None);
    }

    #[test]
    fn watch_progress_foreign_key_references_media_items() {
        let conn = test_db();
        // Enable foreign key enforcement
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();

        // Try to insert watch_progress for non-existent media item
        let result = conn.execute(
            "INSERT INTO watch_progress (media_item_id, position_seconds, duration_seconds, watched, last_watched_at)
             VALUES ('nonexistent', 100.0, 7200.0, 0, '2026-03-14T12:00:00Z')",
            [],
        );
        assert!(result.is_err());
    }
}
