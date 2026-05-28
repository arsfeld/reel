//! Repository for offline-download persistence: the `downloads`,
//! `download_groups`, and `pending_sync` tables (schema v4).
//!
//! Downloads are self-contained and have no foreign key to `media_items`
//! (see `schema::DOWNLOADS_SCHEMA`), so this repo never joins the library
//! table — every field needed to render or resume a download lives on the row.

use rusqlite::{Connection, Row, params};

use crate::models::download::{
    Download, DownloadGroup, DownloadState, FailReason, GroupScope, PendingSync, SyncKind,
};
use crate::models::media::{MediaType, SourceType};

use super::DbError;

/// Repository for download, group, and pending-sync CRUD on SQLite.
#[allow(dead_code)]
pub struct DownloadsRepo<'a> {
    conn: &'a Connection,
}

#[allow(dead_code)]
impl<'a> DownloadsRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    // --- downloads ---

    /// Insert or update a download, keyed by `media_item_id`.
    pub fn upsert(&self, d: &Download) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO downloads (
                media_item_id, part_key, source_type, source_id, state, fail_reason,
                byte_count, total_size, validator, file_path, group_id, queue_order,
                enqueued_at, completed_at, media_type, title, year, parent_id,
                season_number, episode_number, poster_path
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                ?16, ?17, ?18, ?19, ?20, ?21
            )
            ON CONFLICT(media_item_id) DO UPDATE SET
                part_key = excluded.part_key,
                source_type = excluded.source_type,
                source_id = excluded.source_id,
                state = excluded.state,
                fail_reason = excluded.fail_reason,
                byte_count = excluded.byte_count,
                total_size = excluded.total_size,
                validator = excluded.validator,
                file_path = excluded.file_path,
                group_id = excluded.group_id,
                queue_order = excluded.queue_order,
                enqueued_at = excluded.enqueued_at,
                completed_at = excluded.completed_at,
                media_type = excluded.media_type,
                title = excluded.title,
                year = excluded.year,
                parent_id = excluded.parent_id,
                season_number = excluded.season_number,
                episode_number = excluded.episode_number,
                poster_path = excluded.poster_path",
            params![
                d.media_item_id,
                d.part_key,
                d.source_type.as_str(),
                d.source_id,
                d.state.as_str(),
                d.fail_reason.map(|r| r.as_str()),
                d.byte_count,
                d.total_size,
                d.validator,
                d.file_path,
                d.group_id,
                d.queue_order,
                d.enqueued_at,
                d.completed_at,
                d.media_type.as_str(),
                d.title,
                d.year,
                d.parent_id,
                d.season_number,
                d.episode_number,
                d.poster_path,
            ],
        )?;
        Ok(())
    }

    /// Find a download by its media item id.
    pub fn find(&self, media_item_id: &str) -> Result<Option<Download>, DbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM downloads WHERE media_item_id = ?1")?;
        let mut rows = stmt.query(params![media_item_id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_download(row)?)),
            None => Ok(None),
        }
    }

    /// All downloads, ordered by queue position then enqueue time.
    pub fn list_all(&self) -> Result<Vec<Download>, DbError> {
        self.query_downloads(
            "SELECT * FROM downloads ORDER BY queue_order ASC, enqueued_at ASC",
            params![],
        )
    }

    /// Downloads in a given state.
    pub fn list_by_state(&self, state: DownloadState) -> Result<Vec<Download>, DbError> {
        self.query_downloads(
            "SELECT * FROM downloads WHERE state = ?1 ORDER BY queue_order ASC, enqueued_at ASC",
            params![state.as_str()],
        )
    }

    /// Members of a download group (only rows explicitly tagged with `group_id`;
    /// a separately-enqueued episode of the same show is excluded).
    pub fn list_by_group(&self, group_id: &str) -> Result<Vec<Download>, DbError> {
        self.query_downloads(
            "SELECT * FROM downloads WHERE group_id = ?1 ORDER BY season_number ASC, episode_number ASC",
            params![group_id],
        )
    }

    /// Completed downloads ordered oldest-completion first (prune order input).
    pub fn list_completed_oldest_first(&self) -> Result<Vec<Download>, DbError> {
        self.query_downloads(
            "SELECT * FROM downloads WHERE state = 'completed'
             ORDER BY completed_at ASC, media_item_id ASC",
            params![],
        )
    }

    /// Total bytes occupied by completed downloads (uses `total_size`, falling
    /// back to the advisory `byte_count` when the total is unknown).
    pub fn total_completed_bytes(&self) -> Result<i64, DbError> {
        let total: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(COALESCE(total_size, byte_count)), 0)
             FROM downloads WHERE state = 'completed'",
            [],
            |row| row.get(0),
        )?;
        Ok(total)
    }

    /// Update just the state (and optional fail reason) of a download.
    pub fn update_state(
        &self,
        media_item_id: &str,
        state: DownloadState,
        fail_reason: Option<FailReason>,
    ) -> Result<(), DbError> {
        self.conn.execute(
            "UPDATE downloads SET state = ?1, fail_reason = ?2 WHERE media_item_id = ?3",
            params![
                state.as_str(),
                fail_reason.map(|r| r.as_str()),
                media_item_id
            ],
        )?;
        Ok(())
    }

    /// Delete a download row (the file on disk is removed by the caller).
    pub fn delete(&self, media_item_id: &str) -> Result<(), DbError> {
        self.conn.execute(
            "DELETE FROM downloads WHERE media_item_id = ?1",
            params![media_item_id],
        )?;
        Ok(())
    }

    fn query_downloads(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> Result<Vec<Download>, DbError> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params, |row| {
            row_to_download(row).map_err(|e| match e {
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

    // --- groups ---

    /// Insert or update a download group.
    pub fn upsert_group(&self, g: &DownloadGroup) -> Result<(), DbError> {
        self.conn.execute(
            "INSERT INTO download_groups (id, scope, parent_media_id, title, snapshot_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                scope = excluded.scope,
                parent_media_id = excluded.parent_media_id,
                title = excluded.title,
                snapshot_at = excluded.snapshot_at",
            params![
                g.id,
                g.scope.as_str(),
                g.parent_media_id,
                g.title,
                g.snapshot_at
            ],
        )?;
        Ok(())
    }

    /// All download groups, ordered by title for a stable Downloads view.
    pub fn list_groups(&self) -> Result<Vec<DownloadGroup>, DbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM download_groups ORDER BY title ASC, id ASC")?;
        let rows = stmt.query_map([], |row| {
            row_to_group(row).map_err(|e| match e {
                DbError::Sqlite(e) => e,
                other => rusqlite::Error::ToSqlConversionFailure(Box::new(other)),
            })
        })?;
        let mut groups = Vec::new();
        for row in rows {
            groups.push(row?);
        }
        Ok(groups)
    }

    /// Find a group by id.
    pub fn find_group(&self, id: &str) -> Result<Option<DownloadGroup>, DbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM download_groups WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_group(row)?)),
            None => Ok(None),
        }
    }

    /// Delete a group (members' `group_id` is left intact; the caller decides
    /// member fate so a group delete and a member delete stay distinct).
    pub fn delete_group(&self, id: &str) -> Result<(), DbError> {
        self.conn
            .execute("DELETE FROM download_groups WHERE id = ?1", params![id])?;
        Ok(())
    }

    // --- pending sync (offline progress) ---

    /// Queue a progress report recorded while offline. Returns the new rowid.
    pub fn insert_pending_sync(&self, p: &PendingSync) -> Result<i64, DbError> {
        self.conn.execute(
            "INSERT INTO pending_sync (
                media_item_id, rating_key, position_ms, duration_ms, kind, offline_recorded_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                p.media_item_id,
                p.rating_key,
                p.position_ms,
                p.duration_ms,
                p.kind.as_str(),
                p.offline_recorded_at,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// All queued offline reports, oldest first (flush order).
    pub fn list_pending_sync(&self) -> Result<Vec<PendingSync>, DbError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM pending_sync ORDER BY offline_recorded_at ASC, id ASC")?;
        let rows = stmt.query_map([], |row| {
            row_to_pending_sync(row).map_err(|e| match e {
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

    /// Most recent pending report for a media item, if any (used to prefer an
    /// unsynced offline position over a stale source offset on resume).
    pub fn latest_pending_sync_for(
        &self,
        media_item_id: &str,
    ) -> Result<Option<PendingSync>, DbError> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM pending_sync WHERE media_item_id = ?1
             ORDER BY offline_recorded_at DESC, id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![media_item_id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_pending_sync(row)?)),
            None => Ok(None),
        }
    }

    /// Delete a flushed pending-sync row by id.
    pub fn delete_pending_sync(&self, id: i64) -> Result<(), DbError> {
        self.conn
            .execute("DELETE FROM pending_sync WHERE id = ?1", params![id])?;
        Ok(())
    }
}

fn row_to_download(row: &Row) -> Result<Download, DbError> {
    let source_type_str: String = row.get("source_type")?;
    let state_str: String = row.get("state")?;
    let media_type_str: String = row.get("media_type")?;
    let fail_reason_str: Option<String> = row.get("fail_reason")?;
    Ok(Download {
        media_item_id: row.get("media_item_id")?,
        part_key: row.get("part_key")?,
        source_type: SourceType::from_str(&source_type_str).unwrap_or(SourceType::Plex),
        source_id: row.get("source_id")?,
        state: DownloadState::from_db_str(&state_str).unwrap_or(DownloadState::Failed),
        fail_reason: fail_reason_str.as_deref().and_then(FailReason::from_db_str),
        byte_count: row.get("byte_count")?,
        total_size: row.get("total_size")?,
        validator: row.get("validator")?,
        file_path: row.get("file_path")?,
        group_id: row.get("group_id")?,
        queue_order: row.get("queue_order")?,
        enqueued_at: row.get("enqueued_at")?,
        completed_at: row.get("completed_at")?,
        media_type: MediaType::from_str(&media_type_str).unwrap_or(MediaType::Movie),
        title: row.get("title")?,
        year: row.get("year")?,
        parent_id: row.get("parent_id")?,
        season_number: row.get("season_number")?,
        episode_number: row.get("episode_number")?,
        poster_path: row.get("poster_path")?,
    })
}

fn row_to_group(row: &Row) -> Result<DownloadGroup, DbError> {
    let scope_str: String = row.get("scope")?;
    Ok(DownloadGroup {
        id: row.get("id")?,
        scope: GroupScope::from_db_str(&scope_str).unwrap_or(GroupScope::Show),
        parent_media_id: row.get("parent_media_id")?,
        title: row.get("title")?,
        snapshot_at: row.get("snapshot_at")?,
    })
}

fn row_to_pending_sync(row: &Row) -> Result<PendingSync, DbError> {
    let kind_str: String = row.get("kind")?;
    Ok(PendingSync {
        id: row.get("id")?,
        media_item_id: row.get("media_item_id")?,
        rating_key: row.get("rating_key")?,
        position_ms: row.get("position_ms")?,
        duration_ms: row.get("duration_ms")?,
        kind: SyncKind::from_db_str(&kind_str).unwrap_or(SyncKind::Timeline),
        offline_recorded_at: row.get("offline_recorded_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        conn
    }

    fn sample_download(id: &str) -> Download {
        Download {
            media_item_id: id.to_string(),
            part_key: "/library/parts/456/file.mkv".to_string(),
            source_type: SourceType::Plex,
            source_id: "http://localhost:32400".to_string(),
            state: DownloadState::Queued,
            fail_reason: None,
            byte_count: 0,
            total_size: Some(1_000_000),
            validator: None,
            file_path: None,
            group_id: None,
            queue_order: 0,
            enqueued_at: "2026-05-28T10:00:00Z".to_string(),
            completed_at: None,
            media_type: MediaType::Movie,
            title: "Dune".to_string(),
            year: Some(2021),
            parent_id: None,
            season_number: None,
            episode_number: None,
            poster_path: Some("/library/metadata/1/thumb/1".to_string()),
        }
    }

    fn completed(id: &str, size: i64, completed_at: &str) -> Download {
        Download {
            state: DownloadState::Completed,
            total_size: Some(size),
            completed_at: Some(completed_at.to_string()),
            file_path: Some(format!("/downloads/{id}.mkv")),
            ..sample_download(id)
        }
    }

    #[test]
    fn upsert_and_find_roundtrip() {
        let conn = test_db();
        let repo = DownloadsRepo::new(&conn);
        let d = sample_download("plex:src:1");
        repo.upsert(&d).unwrap();
        let found = repo.find("plex:src:1").unwrap().unwrap();
        assert_eq!(found, d);
    }

    #[test]
    fn upsert_is_idempotent_and_updates() {
        let conn = test_db();
        let repo = DownloadsRepo::new(&conn);
        repo.upsert(&sample_download("m1")).unwrap();
        let mut updated = sample_download("m1");
        updated.state = DownloadState::Downloading;
        updated.byte_count = 512;
        repo.upsert(&updated).unwrap();

        let all = repo.list_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].state, DownloadState::Downloading);
        assert_eq!(all[0].byte_count, 512);
    }

    #[test]
    fn find_returns_none_for_missing() {
        let conn = test_db();
        let repo = DownloadsRepo::new(&conn);
        assert!(repo.find("nope").unwrap().is_none());
    }

    #[test]
    fn list_by_state_filters() {
        let conn = test_db();
        let repo = DownloadsRepo::new(&conn);
        repo.upsert(&sample_download("q1")).unwrap();
        repo.upsert(&completed("c1", 100, "2026-05-28T11:00:00Z"))
            .unwrap();

        let queued = repo.list_by_state(DownloadState::Queued).unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].media_item_id, "q1");
    }

    #[test]
    fn update_state_sets_fail_reason() {
        let conn = test_db();
        let repo = DownloadsRepo::new(&conn);
        repo.upsert(&sample_download("m1")).unwrap();
        repo.update_state("m1", DownloadState::Failed, Some(FailReason::DiskFull))
            .unwrap();
        let found = repo.find("m1").unwrap().unwrap();
        assert_eq!(found.state, DownloadState::Failed);
        assert_eq!(found.fail_reason, Some(FailReason::DiskFull));
    }

    #[test]
    fn group_membership_excludes_separately_enqueued_episode() {
        let conn = test_db();
        let repo = DownloadsRepo::new(&conn);
        repo.upsert_group(&DownloadGroup {
            id: "g1".to_string(),
            scope: GroupScope::Show,
            parent_media_id: "show1".to_string(),
            title: "The Show".to_string(),
            snapshot_at: "2026-05-28T10:00:00Z".to_string(),
        })
        .unwrap();

        // Two episodes enqueued as part of the group.
        let mut ep1 = sample_download("ep1");
        ep1.group_id = Some("g1".to_string());
        ep1.media_type = MediaType::Episode;
        ep1.season_number = Some(1);
        ep1.episode_number = Some(1);
        repo.upsert(&ep1).unwrap();

        let mut ep2 = sample_download("ep2");
        ep2.group_id = Some("g1".to_string());
        ep2.media_type = MediaType::Episode;
        ep2.season_number = Some(1);
        ep2.episode_number = Some(2);
        repo.upsert(&ep2).unwrap();

        // A third episode of the same show enqueued on its own (no group_id).
        let mut solo = sample_download("ep3");
        solo.media_type = MediaType::Episode;
        solo.season_number = Some(1);
        solo.episode_number = Some(3);
        repo.upsert(&solo).unwrap();

        let members = repo.list_by_group("g1").unwrap();
        assert_eq!(members.len(), 2);
        assert!(members.iter().all(|d| d.group_id.as_deref() == Some("g1")));
        assert!(!members.iter().any(|d| d.media_item_id == "ep3"));
    }

    #[test]
    fn group_upsert_find_roundtrip() {
        let conn = test_db();
        let repo = DownloadsRepo::new(&conn);
        let g = DownloadGroup {
            id: "g1".to_string(),
            scope: GroupScope::Season,
            parent_media_id: "season1".to_string(),
            title: "Season 1".to_string(),
            snapshot_at: "2026-05-28T10:00:00Z".to_string(),
        };
        repo.upsert_group(&g).unwrap();
        assert_eq!(repo.find_group("g1").unwrap().unwrap(), g);
    }

    #[test]
    fn total_completed_bytes_sums_only_completed() {
        let conn = test_db();
        let repo = DownloadsRepo::new(&conn);
        repo.upsert(&completed("c1", 100, "2026-05-28T11:00:00Z"))
            .unwrap();
        repo.upsert(&completed("c2", 250, "2026-05-28T12:00:00Z"))
            .unwrap();
        repo.upsert(&sample_download("q1")).unwrap(); // queued, not counted
        assert_eq!(repo.total_completed_bytes().unwrap(), 350);
    }

    #[test]
    fn list_completed_oldest_first_orders_by_completion() {
        let conn = test_db();
        let repo = DownloadsRepo::new(&conn);
        repo.upsert(&completed("late", 1, "2026-05-28T20:00:00Z"))
            .unwrap();
        repo.upsert(&completed("early", 1, "2026-05-28T08:00:00Z"))
            .unwrap();
        let ordered = repo.list_completed_oldest_first().unwrap();
        assert_eq!(ordered[0].media_item_id, "early");
        assert_eq!(ordered[1].media_item_id, "late");
    }

    #[test]
    fn pending_sync_insert_and_list_oldest_first() {
        let conn = test_db();
        let repo = DownloadsRepo::new(&conn);
        let id1 = repo
            .insert_pending_sync(&PendingSync {
                id: None,
                media_item_id: "m1".to_string(),
                rating_key: "123".to_string(),
                position_ms: 1000,
                duration_ms: 5000,
                kind: SyncKind::Timeline,
                offline_recorded_at: "2026-05-28T10:00:00Z".to_string(),
            })
            .unwrap();
        repo.insert_pending_sync(&PendingSync {
            id: None,
            media_item_id: "m2".to_string(),
            rating_key: "456".to_string(),
            position_ms: 2000,
            duration_ms: 5000,
            kind: SyncKind::Scrobble,
            offline_recorded_at: "2026-05-28T09:00:00Z".to_string(),
        })
        .unwrap();

        let pending = repo.list_pending_sync().unwrap();
        assert_eq!(pending.len(), 2);
        // m2 recorded earlier -> first.
        assert_eq!(pending[0].media_item_id, "m2");
        assert_eq!(pending[1].media_item_id, "m1");

        repo.delete_pending_sync(id1).unwrap();
        assert_eq!(repo.list_pending_sync().unwrap().len(), 1);
    }

    #[test]
    fn latest_pending_sync_for_returns_most_recent() {
        let conn = test_db();
        let repo = DownloadsRepo::new(&conn);
        repo.insert_pending_sync(&PendingSync {
            id: None,
            media_item_id: "m1".to_string(),
            rating_key: "123".to_string(),
            position_ms: 1000,
            duration_ms: 5000,
            kind: SyncKind::Timeline,
            offline_recorded_at: "2026-05-28T09:00:00Z".to_string(),
        })
        .unwrap();
        repo.insert_pending_sync(&PendingSync {
            id: None,
            media_item_id: "m1".to_string(),
            rating_key: "123".to_string(),
            position_ms: 4000,
            duration_ms: 5000,
            kind: SyncKind::Timeline,
            offline_recorded_at: "2026-05-28T10:00:00Z".to_string(),
        })
        .unwrap();

        let latest = repo.latest_pending_sync_for("m1").unwrap().unwrap();
        assert_eq!(latest.position_ms, 4000);
        assert!(repo.latest_pending_sync_for("other").unwrap().is_none());
    }
}
