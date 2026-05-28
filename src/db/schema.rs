use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use tracing::warn;

use super::DbError;

const SCHEMA_VERSION: i32 = 4;

/// Schema for the offline-downloads tables (v4). Kept as a constant so the
/// fresh-database path (the main `init_db` batch) and the migration path
/// (`migrate_to_v4`) create byte-identical tables.
///
/// Note: `downloads` deliberately has NO foreign key to `media_items`.
/// Downloads are self-contained (they snapshot their own metadata) and must
/// survive independently of the library cache — including the foreign-schema
/// self-heal in `init_db`, which drops `media_items` and every other user
/// table. That self-heal wipes these download tables too, but NOT the media
/// files on disk; `DownloadsRepo::verify_downloads` re-adopts on-disk downloads
/// from their sidecars after such a rebuild.
const DOWNLOADS_SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS download_groups (
        id TEXT PRIMARY KEY,
        scope TEXT NOT NULL,
        parent_media_id TEXT NOT NULL,
        title TEXT NOT NULL,
        snapshot_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS downloads (
        media_item_id TEXT PRIMARY KEY,
        part_key TEXT NOT NULL,
        source_type TEXT NOT NULL,
        source_id TEXT NOT NULL,
        state TEXT NOT NULL,
        fail_reason TEXT,
        byte_count INTEGER NOT NULL DEFAULT 0,
        total_size INTEGER,
        validator TEXT,
        file_path TEXT,
        group_id TEXT,
        queue_order INTEGER NOT NULL DEFAULT 0,
        enqueued_at TEXT NOT NULL,
        completed_at TEXT,
        media_type TEXT NOT NULL,
        title TEXT NOT NULL,
        year INTEGER,
        parent_id TEXT,
        season_number INTEGER,
        episode_number INTEGER,
        poster_path TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_downloads_state
        ON downloads(state);
    CREATE INDEX IF NOT EXISTS idx_downloads_group
        ON downloads(group_id);
    CREATE INDEX IF NOT EXISTS idx_downloads_queue_order
        ON downloads(queue_order);

    CREATE TABLE IF NOT EXISTS pending_sync (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        media_item_id TEXT NOT NULL,
        rating_key TEXT NOT NULL,
        position_ms INTEGER NOT NULL,
        duration_ms INTEGER NOT NULL,
        kind TEXT NOT NULL,
        offline_recorded_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_pending_sync_recorded
        ON pending_sync(offline_recorded_at);
";

/// Initialize the database schema. Creates tables if they don't exist.
/// Idempotent: safe to call on every app startup.
pub fn init_db(conn: &Connection) -> Result<(), DbError> {
    // The data directory was historically shared with predecessor apps that
    // used the same file with an incompatible schema (INTEGER ids,
    // `position_ms`, extra tables). Their tables satisfy `CREATE TABLE IF NOT
    // EXISTS` but lack reel-ng's columns, so every query fails. Detect that
    // shape, snapshot it, and rebuild this database as reel-ng's own.
    if is_foreign_schema(conn)? {
        warn!("Database has an incompatible schema from another app; backing up and rebuilding");
        if let Err(e) = backup_foreign_db(conn) {
            warn!("Could not snapshot the incompatible database before rebuild: {e}");
        }
        drop_all_tables(conn)?;
    }

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

    conn.execute_batch(DOWNLOADS_SCHEMA)?;

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
    if current_version < 4 {
        migrate_to_v4(conn)?;
    }

    Ok(())
}

/// Migrate schema to v4: add the offline-downloads tables (`download_groups`,
/// `downloads`, `pending_sync`). Uses the shared `DOWNLOADS_SCHEMA` with
/// `CREATE TABLE IF NOT EXISTS`, so it is idempotent and produces the same
/// tables as a fresh `init_db`.
fn migrate_to_v4(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(DOWNLOADS_SCHEMA)?;
    conn.execute("UPDATE schema_version SET version = ?1", [4])?;
    Ok(())
}

/// Reel-ng's `media_items` table is keyed by a TEXT `id` and carries an
/// `external_id` column. A predecessor app's table exists (so `IF NOT EXISTS`
/// is a no-op) but has neither. Treat "table present without `external_id`" as
/// the signal that this database belongs to another app and must be rebuilt.
fn is_foreign_schema(conn: &Connection) -> Result<bool, DbError> {
    let table_present: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'media_items'",
        [],
        |row| row.get(0),
    )?;
    if table_present == 0 {
        return Ok(false);
    }
    let has_external_id: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('media_items') WHERE name = 'external_id'",
        [],
        |row| row.get(0),
    )?;
    Ok(has_external_id == 0)
}

/// Snapshot the current (incompatible) database next to itself before it is
/// rebuilt, so the predecessor data is recoverable. No-op for in-memory
/// databases. Best-effort: the caller logs and continues on failure.
fn backup_foreign_db(conn: &Connection) -> Result<(), DbError> {
    let main_file: String = conn.query_row(
        "SELECT file FROM pragma_database_list WHERE name = 'main'",
        [],
        |row| row.get(0),
    )?;
    if main_file.is_empty() {
        return Ok(()); // in-memory database
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup = format!("{main_file}.foreign-backup-{ts}");
    conn.execute("VACUUM INTO ?1", [backup])?;
    Ok(())
}

/// Drop every user table so the schema can be recreated from scratch. Internal
/// `sqlite_*` tables are left alone. Foreign keys are disabled for the duration
/// so cross-table references don't block the drops.
fn drop_all_tables(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let names: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for name in names {
        conn.execute_batch(&format!("DROP TABLE IF EXISTS \"{name}\";"))?;
    }
    Ok(())
}

/// Migrate schema from v2 to v3: add `video_resolution` and `hdr` columns to
/// `media_items`. Both were previously read into `MediaItem` from Plex but
/// never persisted, so the columns are nullable with no default backfill —
/// existing rows get the values on next Plex sync.
fn migrate_to_v3(conn: &Connection) -> Result<(), DbError> {
    // Only runs for genuine v2 databases (the version gate in init_db skips it
    // for fresh v3 schemas), so the columns never already exist here. A
    // duplicate-column error is tolerated defensively; any other error (locked
    // DB, disk full, I/O) propagates so we don't bump the version past a
    // failed migration.
    add_column_if_missing(conn, "video_resolution")?;
    add_column_if_missing(conn, "hdr")?;

    conn.execute("UPDATE schema_version SET version = ?1", [3])?;

    Ok(())
}

/// Add a nullable TEXT column to `media_items`, treating a duplicate-column
/// error as success and propagating everything else.
fn add_column_if_missing(conn: &Connection, column: &str) -> Result<(), DbError> {
    let sql = format!("ALTER TABLE media_items ADD COLUMN {column} TEXT");
    match conn.execute(&sql, []) {
        Ok(_) => Ok(()),
        Err(rusqlite::Error::SqliteFailure(_, Some(ref msg)))
            if msg.contains("duplicate column name") =>
        {
            Ok(())
        }
        Err(e) => Err(DbError::from(e)),
    }
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
            .query_row("SELECT title FROM media_items WHERE id = 'm1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(title, "Dune");

        // New columns exist and default to NULL.
        let hdr: Option<String> = conn
            .query_row("SELECT hdr FROM media_items WHERE id = 'm1'", [], |row| {
                row.get(0)
            })
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
    fn fresh_db_has_download_tables() {
        let conn = test_db();
        for table in ["downloads", "download_groups", "pending_sync"] {
            let count: i32 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "table {table} should exist and be empty");
        }
    }

    #[test]
    fn migrate_v3_to_v4_adds_download_tables() {
        // Build a v3 schema by hand (media_items + watch_progress, no downloads).
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE schema_version (version INTEGER NOT NULL);
            INSERT INTO schema_version (version) VALUES (3);

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
                video_resolution TEXT,
                hdr TEXT,
                added_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            ",
        )
        .unwrap();

        assert_eq!(schema_version(&conn).unwrap(), 3);
        init_db(&conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);

        // Download tables now exist.
        let downloads: i32 = conn
            .query_row("SELECT COUNT(*) FROM downloads", [], |row| row.get(0))
            .unwrap();
        assert_eq!(downloads, 0);
        let groups: i32 = conn
            .query_row("SELECT COUNT(*) FROM download_groups", [], |row| row.get(0))
            .unwrap();
        assert_eq!(groups, 0);
        let pending: i32 = conn
            .query_row("SELECT COUNT(*) FROM pending_sync", [], |row| row.get(0))
            .unwrap();
        assert_eq!(pending, 0);
    }

    #[test]
    fn migrate_to_v4_is_idempotent() {
        let conn = test_db(); // already v4 via init_db
        // Running init_db again must not error on the existing download tables.
        init_db(&conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn fresh_and_migrated_download_schema_match() {
        // Columns of `downloads` on a fresh DB.
        let fresh = test_db();
        let fresh_cols: Vec<String> = {
            let mut stmt = fresh
                .prepare("SELECT name FROM pragma_table_info('downloads') ORDER BY name")
                .unwrap();
            let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
            rows.collect::<Result<Vec<_>, _>>().unwrap()
        };

        // Columns of `downloads` after a v3->v4 migration.
        let migrated = Connection::open_in_memory().unwrap();
        migrated
            .execute_batch(
                "CREATE TABLE schema_version (version INTEGER NOT NULL);
                 INSERT INTO schema_version (version) VALUES (3);
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
                     video_resolution TEXT,
                     hdr TEXT,
                     added_at TEXT NOT NULL,
                     updated_at TEXT NOT NULL
                 );",
            )
            .unwrap();
        init_db(&migrated).unwrap();
        let migrated_cols: Vec<String> = {
            let mut stmt = migrated
                .prepare("SELECT name FROM pragma_table_info('downloads') ORDER BY name")
                .unwrap();
            let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
            rows.collect::<Result<Vec<_>, _>>().unwrap()
        };

        assert_eq!(fresh_cols, migrated_cols);
    }

    #[test]
    fn foreign_schema_is_rebuilt_as_reel_ng() {
        // A predecessor app's database: INTEGER-keyed media_items with no
        // external_id, a position_ms watch_progress table, an unknown table,
        // and a schema_version with multiple rows.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE schema_version (version INTEGER NOT NULL);
            INSERT INTO schema_version (version) VALUES (3);
            INSERT INTO schema_version (version) VALUES (4);
            INSERT INTO schema_version (version) VALUES (6);

            CREATE TABLE media_items (
                id INTEGER PRIMARY KEY,
                summary TEXT,
                library_section TEXT
            );
            INSERT INTO media_items (id, summary) VALUES (1, 'old data');

            CREATE TABLE watch_progress (
                media_item_id INTEGER PRIMARY KEY,
                position_ms INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE collections (id INTEGER PRIMARY KEY);
            ",
        )
        .unwrap();

        init_db(&conn).unwrap();

        // reel-ng's columns now exist.
        let has_external_id: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('media_items') WHERE name = 'external_id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(has_external_id, 1);
        let has_position_seconds: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('watch_progress') WHERE name = 'position_seconds'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(has_position_seconds, 1);

        // Foreign table and old rows are gone.
        let collections: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'collections'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(collections, 0);
        let media_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM media_items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(media_rows, 0);

        // schema_version is a single, correct row.
        let version_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version_rows, 1);
        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn fresh_database_is_not_treated_as_foreign() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(!is_foreign_schema(&conn).unwrap());
        init_db(&conn).unwrap();
        // A reel-ng database (with external_id) is never flagged as foreign.
        assert!(!is_foreign_schema(&conn).unwrap());
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
