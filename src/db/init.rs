//! Database initialization on a diesel `SqliteConnection`: detect and rebuild a
//! foreign predecessor-app schema, then run embedded migrations.
//!
//! Replaces the legacy rusqlite `crate::db::schema::init_db`. The foreign-schema
//! guard preserves the data-protection behavior: a predecessor app's database is
//! snapshotted before it is dropped and rebuilt.

use std::time::{SystemTime, UNIX_EPOCH};

use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Text};
use tracing::warn;

use super::DbError;

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    n: i64,
}

#[derive(QueryableByName)]
struct TextRow {
    #[diesel(sql_type = Text)]
    value: String,
}

/// Initialize the database: rebuild it if it belongs to a predecessor app,
/// then apply any pending migrations. Idempotent — safe on every startup.
#[allow(dead_code)]
pub fn init_db(conn: &mut SqliteConnection) -> Result<(), DbError> {
    // The data directory was historically shared with predecessor apps that
    // used the same file with an incompatible schema (INTEGER ids, extra
    // tables). Their `media_items` lacks reel-ng's columns, so every query
    // fails. Detect that shape, snapshot it, and rebuild as reel-ng's own.
    if is_foreign_schema(conn)? {
        warn!("Database has an incompatible schema from another app; backing up and rebuilding");
        if let Err(e) = backup_foreign_db(conn) {
            warn!("Could not snapshot the incompatible database before rebuild: {e}");
        }
        drop_all_tables(conn)?;
    }

    super::migrations::run_migrations(conn)
}

/// Reel-ng's `media_items` is keyed by a TEXT `id` and carries an `external_id`
/// column. A predecessor app's table exists but has neither. Treat "table
/// present without `external_id`" as the signal that the database belongs to
/// another app and must be rebuilt.
#[allow(dead_code)]
fn is_foreign_schema(conn: &mut SqliteConnection) -> Result<bool, DbError> {
    let table_present = diesel::sql_query(
        "SELECT COUNT(*) AS n FROM sqlite_master WHERE type = 'table' AND name = 'media_items'",
    )
    .get_result::<CountRow>(conn)?
    .n;
    if table_present == 0 {
        return Ok(false);
    }
    let has_external_id = diesel::sql_query(
        "SELECT COUNT(*) AS n FROM pragma_table_info('media_items') WHERE name = 'external_id'",
    )
    .get_result::<CountRow>(conn)?
    .n;
    Ok(has_external_id == 0)
}

/// Snapshot the current (incompatible) database next to itself before it is
/// rebuilt, so predecessor data is recoverable. No-op for in-memory databases.
/// Best-effort: the caller logs and continues on failure.
#[allow(dead_code)]
fn backup_foreign_db(conn: &mut SqliteConnection) -> Result<(), DbError> {
    let main_file = diesel::sql_query("SELECT file AS value FROM pragma_database_list WHERE name = 'main'")
        .get_result::<TextRow>(conn)?
        .value;
    if main_file.is_empty() {
        return Ok(()); // in-memory database
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup = format!("{main_file}.foreign-backup-{ts}");
    diesel::sql_query("VACUUM INTO ?")
        .bind::<Text, _>(backup)
        .execute(conn)?;
    Ok(())
}

/// Drop every user table so the schema can be recreated from scratch. Internal
/// `sqlite_*` tables are left alone. Foreign keys are disabled for the duration
/// so cross-table references don't block the drops.
#[allow(dead_code)]
fn drop_all_tables(conn: &mut SqliteConnection) -> Result<(), DbError> {
    conn.batch_execute("PRAGMA foreign_keys = OFF;")?;
    let names: Vec<String> = diesel::sql_query(
        "SELECT name AS value FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
    )
    .load::<TextRow>(conn)?
    .into_iter()
    .map(|r| r.value)
    .collect();
    for name in names {
        conn.batch_execute(&format!("DROP TABLE IF EXISTS \"{name}\";"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_present(conn: &mut SqliteConnection, name: &str) -> bool {
        diesel::sql_query(
            "SELECT COUNT(*) AS n FROM sqlite_master WHERE type = 'table' AND name = ?",
        )
        .bind::<Text, _>(name)
        .get_result::<CountRow>(conn)
        .unwrap()
        .n
            == 1
    }

    fn column_present(conn: &mut SqliteConnection, table: &str, column: &str) -> bool {
        // pragma_table_info doesn't accept a bound table name; the table name is
        // a trusted literal here.
        diesel::sql_query(format!(
            "SELECT COUNT(*) AS n FROM pragma_table_info('{table}') WHERE name = ?"
        ))
        .bind::<Text, _>(column)
        .get_result::<CountRow>(conn)
        .unwrap()
        .n
            == 1
    }

    fn row_count(conn: &mut SqliteConnection, table: &str) -> i64 {
        diesel::sql_query(format!("SELECT COUNT(*) AS n FROM {table}"))
            .get_result::<CountRow>(conn)
            .unwrap()
            .n
    }

    #[test]
    fn fresh_database_is_not_foreign_and_initializes() {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        assert!(!is_foreign_schema(&mut conn).unwrap());
        init_db(&mut conn).unwrap();
        // A reel-ng database (with external_id) is never flagged foreign.
        assert!(!is_foreign_schema(&mut conn).unwrap());
        assert!(column_present(&mut conn, "media_items", "external_id"));
    }

    #[test]
    fn init_db_is_idempotent() {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        init_db(&mut conn).unwrap();
        init_db(&mut conn).unwrap();
        assert_eq!(row_count(&mut conn, "__diesel_schema_migrations"), 1);
    }

    #[test]
    fn foreign_schema_is_rebuilt_as_reel_ng() {
        // A predecessor app's database: INTEGER-keyed media_items with no
        // external_id, an unknown table, and a multi-row schema_version.
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        conn.batch_execute(
            "
            CREATE TABLE schema_version (version INTEGER NOT NULL);
            INSERT INTO schema_version (version) VALUES (3);
            INSERT INTO schema_version (version) VALUES (6);

            CREATE TABLE media_items (
                id INTEGER PRIMARY KEY,
                summary TEXT,
                library_section TEXT
            );
            INSERT INTO media_items (id, summary) VALUES (1, 'old data');

            CREATE TABLE collections (id INTEGER PRIMARY KEY);
            ",
        )
        .unwrap();

        assert!(is_foreign_schema(&mut conn).unwrap());
        init_db(&mut conn).unwrap();

        // reel-ng's columns now exist; the foreign table and old rows are gone.
        assert!(column_present(&mut conn, "media_items", "external_id"));
        assert!(!table_present(&mut conn, "collections"));
        assert_eq!(row_count(&mut conn, "media_items"), 0);
        // Migrations recorded once; foreign data did not carry over.
        assert_eq!(row_count(&mut conn, "__diesel_schema_migrations"), 1);
    }

    #[test]
    fn watch_progress_fk_cascade_enforced_when_pragma_on() {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        init_db(&mut conn).unwrap();
        conn.batch_execute("PRAGMA foreign_keys = ON;").unwrap();

        // Inserting watch_progress for a non-existent media item must fail.
        let result = diesel::sql_query(
            "INSERT INTO watch_progress (media_item_id, position_seconds, duration_seconds, watched, last_watched_at)
             VALUES ('nope', 100.0, 7200.0, 0, '2026-03-14T12:00:00Z')",
        )
        .execute(&mut conn);
        assert!(result.is_err());
    }
}
