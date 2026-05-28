use diesel::prelude::*;

use crate::db::rows::{NewWatchProgressRow, WatchProgressRow};
use crate::models::watch::WatchProgress;
use crate::schema::{media_items, watch_progress};

use super::DbError;

/// Repository for watch progress CRUD operations on SQLite (diesel).
#[allow(dead_code)]
pub struct WatchProgressRepo<'a> {
    conn: &'a mut SqliteConnection,
}

#[allow(dead_code)]
impl<'a> WatchProgressRepo<'a> {
    pub fn new(conn: &'a mut SqliteConnection) -> Self {
        Self { conn }
    }

    /// Insert or update watch progress for a media item.
    pub fn upsert(&mut self, progress: &WatchProgress) -> Result<(), DbError> {
        let new = NewWatchProgressRow::from(progress);
        diesel::insert_into(watch_progress::table)
            .values(&new)
            .on_conflict(watch_progress::media_item_id)
            .do_update()
            .set((
                watch_progress::position_seconds.eq(&new.position_seconds),
                watch_progress::duration_seconds.eq(&new.duration_seconds),
                watch_progress::watched.eq(&new.watched),
                watch_progress::last_watched_at.eq(&new.last_watched_at),
            ))
            .execute(self.conn)?;
        Ok(())
    }

    /// Find watch progress by media item ID.
    pub fn find_by_media_id(
        &mut self,
        media_item_id: &str,
    ) -> Result<Option<WatchProgress>, DbError> {
        let row = watch_progress::table
            .find(media_item_id)
            .select(WatchProgressRow::as_select())
            .first::<WatchProgressRow>(self.conn)
            .optional()?;
        Ok(row.map(WatchProgress::from))
    }

    /// List in-progress items (not watched, position > 0), ordered by
    /// last_watched_at DESC.
    pub fn list_in_progress(&mut self, limit: usize) -> Result<Vec<WatchProgress>, DbError> {
        let rows = watch_progress::table
            .filter(
                watch_progress::watched
                    .eq(0)
                    .and(watch_progress::position_seconds.gt(0.0)),
            )
            .order(watch_progress::last_watched_at.desc())
            .limit(limit as i64)
            .select(WatchProgressRow::as_select())
            .load::<WatchProgressRow>(self.conn)?;
        Ok(rows.into_iter().map(WatchProgress::from).collect())
    }

    /// List all watched items. Replaces the inline `watched = 1` scan that used
    /// to live in `app::db_helpers::load_watch_data`.
    pub fn list_watched(&mut self) -> Result<Vec<WatchProgress>, DbError> {
        let rows = watch_progress::table
            .filter(watch_progress::watched.eq(1))
            .select(WatchProgressRow::as_select())
            .load::<WatchProgressRow>(self.conn)?;
        Ok(rows.into_iter().map(WatchProgress::from).collect())
    }

    /// Mark a media item as watched.
    pub fn mark_watched(&mut self, media_item_id: &str, timestamp: &str) -> Result<(), DbError> {
        diesel::update(watch_progress::table.find(media_item_id))
            .set((
                watch_progress::watched.eq(1),
                watch_progress::last_watched_at.eq(timestamp),
            ))
            .execute(self.conn)?;
        Ok(())
    }

    /// Mark a media item as unwatched (reset progress).
    pub fn mark_unwatched(&mut self, media_item_id: &str) -> Result<(), DbError> {
        diesel::delete(watch_progress::table.find(media_item_id)).execute(self.conn)?;
        Ok(())
    }

    /// Delete watch progress for a specific media item.
    pub fn delete_by_media_id(&mut self, media_item_id: &str) -> Result<(), DbError> {
        diesel::delete(watch_progress::table.find(media_item_id)).execute(self.conn)?;
        Ok(())
    }

    /// Remove orphan watch_progress rows whose media_item_id no longer exists
    /// in media_items.
    pub fn cleanup_orphans(&mut self) -> Result<usize, DbError> {
        let count = diesel::delete(watch_progress::table.filter(
            watch_progress::media_item_id.ne_all(media_items::table.select(media_items::id)),
        ))
        .execute(self.conn)?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init::init_db;
    use crate::db::media_repo::MediaRepo;
    use crate::models::media::{MediaItem, MediaType, SourceType};

    fn test_db() -> SqliteConnection {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        init_db(&mut conn).unwrap();
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
            series_poster_path: None,
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
            watched: false,
            library_section_id: None,
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

    fn setup_with_movie(conn: &mut SqliteConnection, id: &str) {
        MediaRepo::new(conn)
            .upsert(&test_movie(id, "Test Movie"))
            .unwrap();
    }

    #[test]
    fn upsert_and_find_roundtrip() {
        let mut conn = test_db();
        setup_with_movie(&mut conn, "m1");
        let mut repo = WatchProgressRepo::new(&mut conn);

        repo.upsert(&test_progress("m1", 300.0, 7200.0)).unwrap();

        let found = repo.find_by_media_id("m1").unwrap().unwrap();
        assert_eq!(found.media_item_id, "m1");
        assert!((found.position_seconds - 300.0).abs() < f64::EPSILON);
        assert!((found.duration_seconds - 7200.0).abs() < f64::EPSILON);
        assert!(!found.watched);
    }

    #[test]
    fn upsert_updates_existing() {
        let mut conn = test_db();
        setup_with_movie(&mut conn, "m1");
        let mut repo = WatchProgressRepo::new(&mut conn);

        repo.upsert(&test_progress("m1", 100.0, 7200.0)).unwrap();
        repo.upsert(&test_progress("m1", 500.0, 7200.0)).unwrap();

        let found = repo.find_by_media_id("m1").unwrap().unwrap();
        assert!((found.position_seconds - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn find_returns_none_for_missing() {
        let mut conn = test_db();
        let mut repo = WatchProgressRepo::new(&mut conn);
        assert!(repo.find_by_media_id("nonexistent").unwrap().is_none());
    }

    #[test]
    fn list_in_progress_excludes_watched() {
        let mut conn = test_db();
        setup_with_movie(&mut conn, "m1");
        setup_with_movie(&mut conn, "m2");
        let mut repo = WatchProgressRepo::new(&mut conn);

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
        let mut conn = test_db();
        setup_with_movie(&mut conn, "m1");
        let mut repo = WatchProgressRepo::new(&mut conn);

        repo.upsert(&test_progress("m1", 0.0, 7200.0)).unwrap();
        assert!(repo.list_in_progress(20).unwrap().is_empty());
    }

    #[test]
    fn list_in_progress_ordered_by_last_watched_desc() {
        let mut conn = test_db();
        setup_with_movie(&mut conn, "m1");
        setup_with_movie(&mut conn, "m2");
        let mut repo = WatchProgressRepo::new(&mut conn);

        let mut p1 = test_progress("m1", 100.0, 7200.0);
        p1.last_watched_at = "2026-03-14T10:00:00Z".to_string();
        repo.upsert(&p1).unwrap();
        let mut p2 = test_progress("m2", 200.0, 7200.0);
        p2.last_watched_at = "2026-03-14T12:00:00Z".to_string();
        repo.upsert(&p2).unwrap();

        let in_progress = repo.list_in_progress(20).unwrap();
        assert_eq!(in_progress[0].media_item_id, "m2");
        assert_eq!(in_progress[1].media_item_id, "m1");
    }

    #[test]
    fn list_in_progress_respects_limit() {
        let mut conn = test_db();
        for i in 0..5 {
            setup_with_movie(&mut conn, &format!("m{i}"));
        }
        let mut repo = WatchProgressRepo::new(&mut conn);
        for i in 0..5 {
            repo.upsert(&test_progress(&format!("m{i}"), 100.0, 7200.0))
                .unwrap();
        }
        assert_eq!(repo.list_in_progress(3).unwrap().len(), 3);
    }

    #[test]
    fn list_watched_returns_only_watched() {
        let mut conn = test_db();
        setup_with_movie(&mut conn, "m1");
        setup_with_movie(&mut conn, "m2");
        let mut repo = WatchProgressRepo::new(&mut conn);

        repo.upsert(&test_progress("m1", 300.0, 7200.0)).unwrap(); // in progress
        let mut watched = test_progress("m2", 7200.0, 7200.0);
        watched.watched = true;
        repo.upsert(&watched).unwrap();

        let done = repo.list_watched().unwrap();
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].media_item_id, "m2");
        assert!(done[0].watched);
    }

    #[test]
    fn mark_watched_sets_flag() {
        let mut conn = test_db();
        setup_with_movie(&mut conn, "m1");
        let mut repo = WatchProgressRepo::new(&mut conn);

        repo.upsert(&test_progress("m1", 6500.0, 7200.0)).unwrap();
        repo.mark_watched("m1", "2026-03-14T15:00:00Z").unwrap();

        let found = repo.find_by_media_id("m1").unwrap().unwrap();
        assert!(found.watched);
        assert_eq!(found.last_watched_at, "2026-03-14T15:00:00Z");
    }

    #[test]
    fn mark_unwatched_deletes_progress() {
        let mut conn = test_db();
        setup_with_movie(&mut conn, "m1");
        let mut repo = WatchProgressRepo::new(&mut conn);

        repo.upsert(&test_progress("m1", 300.0, 7200.0)).unwrap();
        repo.mark_unwatched("m1").unwrap();
        assert!(repo.find_by_media_id("m1").unwrap().is_none());
    }

    #[test]
    fn delete_by_media_id_removes_entry() {
        let mut conn = test_db();
        setup_with_movie(&mut conn, "m1");
        let mut repo = WatchProgressRepo::new(&mut conn);

        repo.upsert(&test_progress("m1", 300.0, 7200.0)).unwrap();
        repo.delete_by_media_id("m1").unwrap();
        assert!(repo.find_by_media_id("m1").unwrap().is_none());
    }

    #[test]
    fn cleanup_orphans_removes_entries_without_media_items() {
        use diesel::connection::SimpleConnection;
        let mut conn = test_db();
        setup_with_movie(&mut conn, "m1");
        setup_with_movie(&mut conn, "m_orphan");

        {
            let mut repo = WatchProgressRepo::new(&mut conn);
            repo.upsert(&test_progress("m1", 300.0, 7200.0)).unwrap();
            repo.upsert(&test_progress("m_orphan", 100.0, 7200.0))
                .unwrap();
        }

        // diesel enables `foreign_keys = ON` by default, so deleting a media
        // item normally cascade-deletes its watch_progress row. Disable that
        // here to leave a genuine orphan for cleanup_orphans to remove.
        conn.batch_execute("PRAGMA foreign_keys = OFF;").unwrap();
        MediaRepo::new(&mut conn)
            .delete_by_source(&SourceType::Plex, "http://localhost:32400")
            .unwrap();
        // Re-insert m1 so only m_orphan is an orphan.
        setup_with_movie(&mut conn, "m1");

        let mut repo = WatchProgressRepo::new(&mut conn);
        let deleted = repo.cleanup_orphans().unwrap();
        assert_eq!(deleted, 1);
        assert!(repo.find_by_media_id("m1").unwrap().is_some());
        assert!(repo.find_by_media_id("m_orphan").unwrap().is_none());
    }

    #[test]
    fn watched_boolean_roundtrip() {
        let mut conn = test_db();
        setup_with_movie(&mut conn, "m1");
        let mut repo = WatchProgressRepo::new(&mut conn);

        let mut progress = test_progress("m1", 6500.0, 7200.0);
        progress.watched = true;
        repo.upsert(&progress).unwrap();

        let found = repo.find_by_media_id("m1").unwrap().unwrap();
        assert!(found.watched);
    }
}
