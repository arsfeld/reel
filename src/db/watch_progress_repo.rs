use rusqlite::{Connection, params};

use crate::models::watch::WatchProgress;

use super::DbError;

/// Repository for watch progress CRUD operations on SQLite.
#[allow(dead_code)]
pub struct WatchProgressRepo<'a> {
    conn: &'a Connection,
}

#[allow(dead_code)]
impl<'a> WatchProgressRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Insert or update watch progress for a media item.
    pub fn upsert(&self, progress: &WatchProgress) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO watch_progress (
                media_item_id, position_seconds, duration_seconds, watched, last_watched_at
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(media_item_id) DO UPDATE SET
                position_seconds = excluded.position_seconds,
                duration_seconds = excluded.duration_seconds,
                watched = excluded.watched,
                last_watched_at = excluded.last_watched_at",
            params![
                progress.media_item_id,
                progress.position_seconds,
                progress.duration_seconds,
                progress.watched as i32,
                progress.last_watched_at,
            ],
        )?;
        Ok(())
    }

    /// Find watch progress by media item ID.
    pub fn find_by_media_id(&self, media_item_id: &str) -> Result<Option<WatchProgress>, DbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM watch_progress WHERE media_item_id = ?1")?;

        let mut rows = stmt.query(params![media_item_id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_watch_progress(row)?)),
            None => Ok(None),
        }
    }

    /// List in-progress items (not watched, position > 0), ordered by last_watched_at DESC.
    pub fn list_in_progress(&self, limit: usize) -> Result<Vec<WatchProgress>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM watch_progress
             WHERE watched = 0 AND position_seconds > 0
             ORDER BY last_watched_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            row_to_watch_progress(row).map_err(|e| match e {
                DbError::Sqlite(e) => e,
                other => rusqlite::Error::ToSqlConversionFailure(Box::new(other)),
            })
        })?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    }

    /// Mark a media item as watched.
    pub fn mark_watched(&self, media_item_id: &str, timestamp: &str) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE watch_progress SET watched = 1, last_watched_at = ?1 WHERE media_item_id = ?2",
            params![timestamp, media_item_id],
        )?;
        Ok(())
    }

    /// Mark a media item as unwatched (reset progress).
    pub fn mark_unwatched(&self, media_item_id: &str) -> Result<(), DbError> {
        self.conn.execute(
            "DELETE FROM watch_progress WHERE media_item_id = ?1",
            params![media_item_id],
        )?;
        Ok(())
    }

    /// Delete watch progress for a specific media item.
    pub fn delete_by_media_id(&self, media_item_id: &str) -> Result<(), DbError> {
        self.conn.execute(
            "DELETE FROM watch_progress WHERE media_item_id = ?1",
            params![media_item_id],
        )?;
        Ok(())
    }

    /// Remove orphan watch_progress rows where media_item_id no longer exists in media_items.
    pub fn cleanup_orphans(&self) -> Result<usize, DbError> {
        let count = self.conn.execute(
            "DELETE FROM watch_progress WHERE media_item_id NOT IN (SELECT id FROM media_items)",
            [],
        )?;
        Ok(count)
    }
}

fn row_to_watch_progress(row: &rusqlite::Row) -> Result<WatchProgress, DbError> {
    let watched_int: i32 = row.get("watched")?;
    Ok(WatchProgress {
        media_item_id: row.get("media_item_id")?,
        position_seconds: row.get("position_seconds")?,
        duration_seconds: row.get("duration_seconds")?,
        watched: watched_int != 0,
        last_watched_at: row.get("last_watched_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::db::media_repo::MediaRepo;
    use crate::models::media::{MediaItem, MediaType, SourceType};

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        conn
    }

    fn test_movie(id: &str, title: &str) -> MediaItem {
        MediaItem {
            id: id.to_string(),
            source_type: SourceType::Plex,
            source_id: "http://localhost:32400".to_string(),
            external_id: id.to_string(),
            media_type: MediaType::Movie,
            title: title.to_string(),
            year: Some(2021),
            overview: None,
            content_rating: None,
            rating: None,
            runtime_minutes: Some(155),
            poster_path: None,
            backdrop_path: None,
            genres: vec![],
            parent_id: None,
            season_number: None,
            episode_number: None,
            air_date: None,
            file_path: Some("/library/parts/456/file.mkv".to_string()),
            video_resolution: None,
            hdr: None,
            added_at: "2024-01-15".to_string(),
            updated_at: "2024-01-15".to_string(),
            playback_position_ms: None,
        }
    }

    fn test_progress(media_item_id: &str, position: f64, duration: f64) -> WatchProgress {
        WatchProgress {
            media_item_id: media_item_id.to_string(),
            position_seconds: position,
            duration_seconds: duration,
            watched: false,
            last_watched_at: "2026-03-14T12:00:00Z".to_string(),
        }
    }

    fn setup_with_movie(conn: &Connection, id: &str) {
        let media_repo = MediaRepo::new(conn);
        media_repo.upsert(&test_movie(id, "Test Movie")).unwrap();
    }

    #[test]
    fn upsert_and_find_roundtrip() {
        let conn = test_db();
        setup_with_movie(&conn, "m1");
        let repo = WatchProgressRepo::new(&conn);

        let progress = test_progress("m1", 300.0, 7200.0);
        repo.upsert(&progress).unwrap();

        let found = repo.find_by_media_id("m1").unwrap().unwrap();
        assert_eq!(found.media_item_id, "m1");
        assert!((found.position_seconds - 300.0).abs() < f64::EPSILON);
        assert!((found.duration_seconds - 7200.0).abs() < f64::EPSILON);
        assert!(!found.watched);
    }

    #[test]
    fn upsert_updates_existing() {
        let conn = test_db();
        setup_with_movie(&conn, "m1");
        let repo = WatchProgressRepo::new(&conn);

        repo.upsert(&test_progress("m1", 100.0, 7200.0)).unwrap();
        repo.upsert(&test_progress("m1", 500.0, 7200.0)).unwrap();

        let found = repo.find_by_media_id("m1").unwrap().unwrap();
        assert!((found.position_seconds - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn find_returns_none_for_missing() {
        let conn = test_db();
        let repo = WatchProgressRepo::new(&conn);
        assert!(repo.find_by_media_id("nonexistent").unwrap().is_none());
    }

    #[test]
    fn list_in_progress_excludes_watched() {
        let conn = test_db();
        setup_with_movie(&conn, "m1");
        setup_with_movie(&conn, "m2");
        let repo = WatchProgressRepo::new(&conn);

        repo.upsert(&test_progress("m1", 300.0, 7200.0)).unwrap();
        let mut watched = test_progress("m2", 6500.0, 7200.0);
        watched.watched = true;
        repo.upsert(&watched).unwrap();

        let in_progress = repo.list_in_progress(20).unwrap();
        assert_eq!(in_progress.len(), 1);
        assert_eq!(in_progress[0].media_item_id, "m1");
    }

    #[test]
    fn list_in_progress_excludes_zero_position() {
        let conn = test_db();
        setup_with_movie(&conn, "m1");
        let repo = WatchProgressRepo::new(&conn);

        repo.upsert(&test_progress("m1", 0.0, 7200.0)).unwrap();

        let in_progress = repo.list_in_progress(20).unwrap();
        assert!(in_progress.is_empty());
    }

    #[test]
    fn list_in_progress_ordered_by_last_watched_desc() {
        let conn = test_db();
        setup_with_movie(&conn, "m1");
        setup_with_movie(&conn, "m2");
        let repo = WatchProgressRepo::new(&conn);

        let mut p1 = test_progress("m1", 100.0, 7200.0);
        p1.last_watched_at = "2026-03-14T10:00:00Z".to_string();
        repo.upsert(&p1).unwrap();

        let mut p2 = test_progress("m2", 200.0, 7200.0);
        p2.last_watched_at = "2026-03-14T12:00:00Z".to_string();
        repo.upsert(&p2).unwrap();

        let in_progress = repo.list_in_progress(20).unwrap();
        assert_eq!(in_progress[0].media_item_id, "m2"); // more recent first
        assert_eq!(in_progress[1].media_item_id, "m1");
    }

    #[test]
    fn list_in_progress_respects_limit() {
        let conn = test_db();
        for i in 0..5 {
            let id = format!("m{i}");
            setup_with_movie(&conn, &id);
        }
        let repo = WatchProgressRepo::new(&conn);

        for i in 0..5 {
            repo.upsert(&test_progress(&format!("m{i}"), 100.0, 7200.0))
                .unwrap();
        }

        let in_progress = repo.list_in_progress(3).unwrap();
        assert_eq!(in_progress.len(), 3);
    }

    #[test]
    fn mark_watched_sets_flag() {
        let conn = test_db();
        setup_with_movie(&conn, "m1");
        let repo = WatchProgressRepo::new(&conn);

        repo.upsert(&test_progress("m1", 6500.0, 7200.0)).unwrap();
        repo.mark_watched("m1", "2026-03-14T15:00:00Z").unwrap();

        let found = repo.find_by_media_id("m1").unwrap().unwrap();
        assert!(found.watched);
        assert_eq!(found.last_watched_at, "2026-03-14T15:00:00Z");
    }

    #[test]
    fn mark_unwatched_deletes_progress() {
        let conn = test_db();
        setup_with_movie(&conn, "m1");
        let repo = WatchProgressRepo::new(&conn);

        repo.upsert(&test_progress("m1", 300.0, 7200.0)).unwrap();
        repo.mark_unwatched("m1").unwrap();

        assert!(repo.find_by_media_id("m1").unwrap().is_none());
    }

    #[test]
    fn delete_by_media_id_removes_entry() {
        let conn = test_db();
        setup_with_movie(&conn, "m1");
        let repo = WatchProgressRepo::new(&conn);

        repo.upsert(&test_progress("m1", 300.0, 7200.0)).unwrap();
        repo.delete_by_media_id("m1").unwrap();

        assert!(repo.find_by_media_id("m1").unwrap().is_none());
    }

    #[test]
    fn cleanup_orphans_removes_entries_without_media_items() {
        let conn = test_db();
        let media_repo = MediaRepo::new(&conn);
        setup_with_movie(&conn, "m1");
        setup_with_movie(&conn, "m_orphan");
        let repo = WatchProgressRepo::new(&conn);

        // Insert progress for both
        repo.upsert(&test_progress("m1", 300.0, 7200.0)).unwrap();
        repo.upsert(&test_progress("m_orphan", 100.0, 7200.0))
            .unwrap();

        // Delete the media item to create an orphan watch_progress row
        // Disable FK enforcement so the cascade doesn't auto-delete
        conn.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
        media_repo
            .delete_by_source(&SourceType::Plex, "http://localhost:32400")
            .unwrap();

        // Re-insert m1 so it's not an orphan (only m_orphan should be)
        setup_with_movie(&conn, "m1");

        let deleted = repo.cleanup_orphans().unwrap();
        assert_eq!(deleted, 1);

        // m1 should still exist
        assert!(repo.find_by_media_id("m1").unwrap().is_some());
        // orphan should be gone
        assert!(repo.find_by_media_id("m_orphan").unwrap().is_none());
    }

    #[test]
    fn watched_boolean_roundtrip() {
        let conn = test_db();
        setup_with_movie(&conn, "m1");
        let repo = WatchProgressRepo::new(&conn);

        let mut progress = test_progress("m1", 6500.0, 7200.0);
        progress.watched = true;
        repo.upsert(&progress).unwrap();

        let found = repo.find_by_media_id("m1").unwrap().unwrap();
        assert!(found.watched);
    }
}
