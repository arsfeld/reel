use diesel::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use super::DbError;

/// Migrations baked into the binary at compile time. The app self-migrates at
/// startup with no `diesel_cli` dependency at runtime.
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// Run any migrations not yet applied to this database, in order. Idempotent:
/// already-applied migrations (tracked in `__diesel_schema_migrations`) are
/// skipped. Safe to call on every startup.
#[allow(dead_code)]
pub fn run_migrations(conn: &mut SqliteConnection) -> Result<(), DbError> {
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| DbError::Migration(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::connection::SimpleConnection;
    use diesel::prelude::*;
    use diesel::sql_types::{BigInt, Text};

    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = BigInt)]
        n: i64,
    }

    fn table_count(conn: &mut SqliteConnection, name: &str) -> i64 {
        diesel::sql_query(
            "SELECT COUNT(*) AS n FROM sqlite_master WHERE type = 'table' AND name = ?",
        )
        .bind::<Text, _>(name)
        .get_result::<Count>(conn)
        .unwrap()
        .n
    }

    fn applied_migration_count(conn: &mut SqliteConnection) -> i64 {
        diesel::sql_query("SELECT COUNT(*) AS n FROM __diesel_schema_migrations")
            .get_result::<Count>(conn)
            .unwrap()
            .n
    }

    #[test]
    fn fresh_database_gets_all_tables() {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        run_migrations(&mut conn).unwrap();

        assert_eq!(table_count(&mut conn, "media_items"), 1);
        assert_eq!(table_count(&mut conn, "sources"), 1);
        assert_eq!(table_count(&mut conn, "watch_progress"), 1);
    }

    #[test]
    fn running_twice_is_idempotent() {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        run_migrations(&mut conn).unwrap();
        // Second run must be a clean no-op (no pending migrations remain).
        assert!(!conn.has_pending_migration(MIGRATIONS).unwrap());
        run_migrations(&mut conn).unwrap();
    }

    #[test]
    fn all_migrations_recorded() {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        run_migrations(&mut conn).unwrap();
        // baseline + downloads.
        assert_eq!(applied_migration_count(&mut conn), 2);
    }

    #[test]
    fn baseline_is_noop_against_existing_v3_schema() {
        // Simulate an existing reel-ng database: the v3 schema already present
        // (legacy integer schema_version table included), no diesel tracking.
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        conn.batch_execute(
            "
            CREATE TABLE schema_version (version INTEGER NOT NULL);
            INSERT INTO schema_version (version) VALUES (3);

            CREATE TABLE sources (
                id TEXT PRIMARY KEY NOT NULL, source_type TEXT NOT NULL,
                name TEXT NOT NULL, config TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1, last_synced_at TEXT
            );
            CREATE TABLE media_items (
                id TEXT PRIMARY KEY NOT NULL, source_type TEXT NOT NULL,
                source_id TEXT NOT NULL, external_id TEXT NOT NULL,
                media_type TEXT NOT NULL, title TEXT NOT NULL, year INTEGER,
                overview TEXT, content_rating TEXT, rating REAL,
                runtime_minutes INTEGER, poster_path TEXT, backdrop_path TEXT,
                genres TEXT NOT NULL DEFAULT '[]', parent_id TEXT,
                season_number INTEGER, episode_number INTEGER, air_date TEXT,
                file_path TEXT, video_resolution TEXT, hdr TEXT,
                added_at TEXT NOT NULL, updated_at TEXT NOT NULL
            );
            CREATE TABLE watch_progress (
                media_item_id TEXT PRIMARY KEY NOT NULL,
                position_seconds REAL NOT NULL DEFAULT 0.0,
                duration_seconds REAL NOT NULL DEFAULT 0.0,
                watched INTEGER NOT NULL DEFAULT 0,
                last_watched_at TEXT NOT NULL,
                FOREIGN KEY (media_item_id) REFERENCES media_items(id) ON DELETE CASCADE
            );

            INSERT INTO media_items (id, source_type, source_id, external_id,
                media_type, title, genres, added_at, updated_at)
            VALUES ('m1', 'plex', 'http://s', 'k1', 'movie', 'Dune', '[]', '0', '0');
            ",
        )
        .unwrap();

        // Baseline runs, records itself as applied, leaves the existing row.
        // (baseline + downloads = 2 migrations recorded.)
        run_migrations(&mut conn).unwrap();
        assert_eq!(applied_migration_count(&mut conn), 2);

        let rows = diesel::sql_query("SELECT COUNT(*) AS n FROM media_items")
            .get_result::<Count>(&mut conn)
            .unwrap();
        assert_eq!(rows.n, 1, "pre-existing row must survive the baseline");

        // The legacy version table was retired.
        assert_eq!(table_count(&mut conn, "schema_version"), 0);
    }
}
