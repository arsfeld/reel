//! Repository for offline-download persistence: the `downloads`,
//! `download_groups`, and `pending_sync` tables (diesel).
//!
//! Downloads are self-contained and have no foreign key to `media_items`, so
//! this repo never joins the library table — every field needed to render or
//! resume a download lives on the row. The foreign-schema self-heal in
//! `db::init` drops these tables (not the on-disk files); `sidecar::verify_downloads`
//! re-adopts from disk after such a wipe.

use diesel::prelude::*;

use crate::models::download::{
    Download, DownloadGroup, DownloadState, FailReason, GroupScope, PendingSync, SyncKind,
};
use crate::models::media::{MediaType, SourceType};
use crate::schema::{download_groups, downloads, pending_sync};

use super::DbError;

// ---------------------------------------------------------------------------
// Row structs + conversions
// ---------------------------------------------------------------------------

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = downloads)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct DownloadRow {
    media_item_id: String,
    part_key: String,
    source_type: String,
    source_id: String,
    state: String,
    fail_reason: Option<String>,
    byte_count: i64,
    total_size: Option<i64>,
    validator: Option<String>,
    file_path: Option<String>,
    group_id: Option<String>,
    queue_order: i64,
    enqueued_at: String,
    completed_at: Option<String>,
    media_type: String,
    title: String,
    year: Option<i32>,
    parent_id: Option<String>,
    season_number: Option<i32>,
    episode_number: Option<i32>,
    poster_path: Option<String>,
}

#[derive(Debug, Insertable, AsChangeset)]
#[diesel(table_name = downloads, treat_none_as_null = true)]
struct NewDownloadRow {
    media_item_id: String,
    part_key: String,
    source_type: String,
    source_id: String,
    state: String,
    fail_reason: Option<String>,
    byte_count: i64,
    total_size: Option<i64>,
    validator: Option<String>,
    file_path: Option<String>,
    group_id: Option<String>,
    queue_order: i64,
    enqueued_at: String,
    completed_at: Option<String>,
    media_type: String,
    title: String,
    year: Option<i32>,
    parent_id: Option<String>,
    season_number: Option<i32>,
    episode_number: Option<i32>,
    poster_path: Option<String>,
}

impl From<DownloadRow> for Download {
    fn from(r: DownloadRow) -> Self {
        Download {
            media_item_id: r.media_item_id,
            part_key: r.part_key,
            source_type: SourceType::from_str(&r.source_type).unwrap_or(SourceType::Plex),
            source_id: r.source_id,
            state: DownloadState::from_db_str(&r.state).unwrap_or(DownloadState::Failed),
            fail_reason: r.fail_reason.as_deref().and_then(FailReason::from_db_str),
            byte_count: r.byte_count,
            total_size: r.total_size,
            validator: r.validator,
            file_path: r.file_path,
            group_id: r.group_id,
            queue_order: r.queue_order,
            enqueued_at: r.enqueued_at,
            completed_at: r.completed_at,
            media_type: MediaType::from_str(&r.media_type).unwrap_or(MediaType::Movie),
            title: r.title,
            year: r.year,
            parent_id: r.parent_id,
            season_number: r.season_number,
            episode_number: r.episode_number,
            poster_path: r.poster_path,
        }
    }
}

impl From<&Download> for NewDownloadRow {
    fn from(d: &Download) -> Self {
        NewDownloadRow {
            media_item_id: d.media_item_id.clone(),
            part_key: d.part_key.clone(),
            source_type: d.source_type.as_str().to_string(),
            source_id: d.source_id.clone(),
            state: d.state.as_str().to_string(),
            fail_reason: d.fail_reason.map(|r| r.as_str().to_string()),
            byte_count: d.byte_count,
            total_size: d.total_size,
            validator: d.validator.clone(),
            file_path: d.file_path.clone(),
            group_id: d.group_id.clone(),
            queue_order: d.queue_order,
            enqueued_at: d.enqueued_at.clone(),
            completed_at: d.completed_at.clone(),
            media_type: d.media_type.as_str().to_string(),
            title: d.title.clone(),
            year: d.year,
            parent_id: d.parent_id.clone(),
            season_number: d.season_number,
            episode_number: d.episode_number,
            poster_path: d.poster_path.clone(),
        }
    }
}

#[derive(Debug, Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = download_groups, treat_none_as_null = true)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct GroupRow {
    id: String,
    scope: String,
    parent_media_id: String,
    title: String,
    snapshot_at: String,
}

impl From<GroupRow> for DownloadGroup {
    fn from(r: GroupRow) -> Self {
        DownloadGroup {
            id: r.id,
            scope: GroupScope::from_db_str(&r.scope).unwrap_or(GroupScope::Show),
            parent_media_id: r.parent_media_id,
            title: r.title,
            snapshot_at: r.snapshot_at,
        }
    }
}

impl From<&DownloadGroup> for GroupRow {
    fn from(g: &DownloadGroup) -> Self {
        GroupRow {
            id: g.id.clone(),
            scope: g.scope.as_str().to_string(),
            parent_media_id: g.parent_media_id.clone(),
            title: g.title.clone(),
            snapshot_at: g.snapshot_at.clone(),
        }
    }
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = pending_sync)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct PendingSyncRow {
    id: i64,
    media_item_id: String,
    rating_key: String,
    position_ms: i64,
    duration_ms: i64,
    kind: String,
    offline_recorded_at: String,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = pending_sync)]
struct NewPendingSyncRow {
    media_item_id: String,
    rating_key: String,
    position_ms: i64,
    duration_ms: i64,
    kind: String,
    offline_recorded_at: String,
}

impl From<PendingSyncRow> for PendingSync {
    fn from(r: PendingSyncRow) -> Self {
        PendingSync {
            id: Some(r.id),
            media_item_id: r.media_item_id,
            rating_key: r.rating_key,
            position_ms: r.position_ms,
            duration_ms: r.duration_ms,
            kind: SyncKind::from_db_str(&r.kind).unwrap_or(SyncKind::Timeline),
            offline_recorded_at: r.offline_recorded_at,
        }
    }
}

impl From<&PendingSync> for NewPendingSyncRow {
    fn from(p: &PendingSync) -> Self {
        NewPendingSyncRow {
            media_item_id: p.media_item_id.clone(),
            rating_key: p.rating_key.clone(),
            position_ms: p.position_ms,
            duration_ms: p.duration_ms,
            kind: p.kind.as_str().to_string(),
            offline_recorded_at: p.offline_recorded_at.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Repository
// ---------------------------------------------------------------------------

/// Repository for download, group, and pending-sync CRUD on SQLite (diesel).
#[allow(dead_code)]
pub struct DownloadsRepo<'a> {
    conn: &'a mut SqliteConnection,
}

#[allow(dead_code)]
impl<'a> DownloadsRepo<'a> {
    pub fn new(conn: &'a mut SqliteConnection) -> Self {
        Self { conn }
    }

    // --- downloads ---

    /// Insert or update a download, keyed by `media_item_id`.
    pub fn upsert(&mut self, d: &Download) -> Result<(), DbError> {
        let new = NewDownloadRow::from(d);
        diesel::insert_into(downloads::table)
            .values(&new)
            .on_conflict(downloads::media_item_id)
            .do_update()
            .set(&new)
            .execute(self.conn)?;
        Ok(())
    }

    /// Find a download by its media item id.
    pub fn find(&mut self, media_item_id: &str) -> Result<Option<Download>, DbError> {
        let row = downloads::table
            .find(media_item_id)
            .select(DownloadRow::as_select())
            .first::<DownloadRow>(self.conn)
            .optional()?;
        Ok(row.map(Download::from))
    }

    /// All downloads, ordered by queue position then enqueue time.
    pub fn list_all(&mut self) -> Result<Vec<Download>, DbError> {
        let rows = downloads::table
            .order((downloads::queue_order.asc(), downloads::enqueued_at.asc()))
            .select(DownloadRow::as_select())
            .load::<DownloadRow>(self.conn)?;
        Ok(rows.into_iter().map(Download::from).collect())
    }

    /// Downloads in a given state.
    pub fn list_by_state(&mut self, state: DownloadState) -> Result<Vec<Download>, DbError> {
        let rows = downloads::table
            .filter(downloads::state.eq(state.as_str()))
            .order((downloads::queue_order.asc(), downloads::enqueued_at.asc()))
            .select(DownloadRow::as_select())
            .load::<DownloadRow>(self.conn)?;
        Ok(rows.into_iter().map(Download::from).collect())
    }

    /// Members of a download group (only rows explicitly tagged with `group_id`;
    /// a separately-enqueued episode of the same show is excluded).
    pub fn list_by_group(&mut self, group_id: &str) -> Result<Vec<Download>, DbError> {
        let rows = downloads::table
            .filter(downloads::group_id.eq(group_id))
            .order((
                downloads::season_number.asc(),
                downloads::episode_number.asc(),
            ))
            .select(DownloadRow::as_select())
            .load::<DownloadRow>(self.conn)?;
        Ok(rows.into_iter().map(Download::from).collect())
    }

    /// Completed downloads ordered oldest-completion first (prune order input).
    pub fn list_completed_oldest_first(&mut self) -> Result<Vec<Download>, DbError> {
        let rows = downloads::table
            .filter(downloads::state.eq(DownloadState::Completed.as_str()))
            .order((
                downloads::completed_at.asc(),
                downloads::media_item_id.asc(),
            ))
            .select(DownloadRow::as_select())
            .load::<DownloadRow>(self.conn)?;
        Ok(rows.into_iter().map(Download::from).collect())
    }

    /// Total bytes occupied by completed downloads (uses `total_size`, falling
    /// back to the advisory `byte_count` when the total is unknown).
    pub fn total_completed_bytes(&mut self) -> Result<i64, DbError> {
        let rows = downloads::table
            .filter(downloads::state.eq(DownloadState::Completed.as_str()))
            .select((downloads::total_size, downloads::byte_count))
            .load::<(Option<i64>, i64)>(self.conn)?;
        Ok(rows
            .into_iter()
            .map(|(total, bytes)| total.unwrap_or(bytes))
            .sum())
    }

    /// Update just the state (and optional fail reason) of a download.
    pub fn update_state(
        &mut self,
        media_item_id: &str,
        state: DownloadState,
        fail_reason: Option<FailReason>,
    ) -> Result<(), DbError> {
        diesel::update(downloads::table.find(media_item_id))
            .set((
                downloads::state.eq(state.as_str()),
                downloads::fail_reason.eq(fail_reason.map(|r| r.as_str().to_string())),
            ))
            .execute(self.conn)?;
        Ok(())
    }

    /// Delete a download row (the file on disk is removed by the caller).
    pub fn delete(&mut self, media_item_id: &str) -> Result<(), DbError> {
        diesel::delete(downloads::table.find(media_item_id)).execute(self.conn)?;
        Ok(())
    }

    // --- groups ---

    /// Insert or update a download group.
    pub fn upsert_group(&mut self, g: &DownloadGroup) -> Result<(), DbError> {
        let row = GroupRow::from(g);
        diesel::insert_into(download_groups::table)
            .values(&row)
            .on_conflict(download_groups::id)
            .do_update()
            .set(&row)
            .execute(self.conn)?;
        Ok(())
    }

    /// All download groups, ordered by title for a stable Downloads view.
    pub fn list_groups(&mut self) -> Result<Vec<DownloadGroup>, DbError> {
        let rows = download_groups::table
            .order((download_groups::title.asc(), download_groups::id.asc()))
            .select(GroupRow::as_select())
            .load::<GroupRow>(self.conn)?;
        Ok(rows.into_iter().map(DownloadGroup::from).collect())
    }

    /// Find a group by id.
    pub fn find_group(&mut self, id: &str) -> Result<Option<DownloadGroup>, DbError> {
        let row = download_groups::table
            .find(id)
            .select(GroupRow::as_select())
            .first::<GroupRow>(self.conn)
            .optional()?;
        Ok(row.map(DownloadGroup::from))
    }

    /// Delete a group (members' `group_id` is left intact; the caller decides
    /// member fate so a group delete and a member delete stay distinct).
    pub fn delete_group(&mut self, id: &str) -> Result<(), DbError> {
        diesel::delete(download_groups::table.find(id)).execute(self.conn)?;
        Ok(())
    }

    // --- pending sync (offline progress) ---

    /// Queue a progress report recorded while offline. Returns the new rowid.
    pub fn insert_pending_sync(&mut self, p: &PendingSync) -> Result<i64, DbError> {
        let new = NewPendingSyncRow::from(p);
        let id = diesel::insert_into(pending_sync::table)
            .values(&new)
            .returning(pending_sync::id)
            .get_result::<i64>(self.conn)?;
        Ok(id)
    }

    /// All queued offline reports, oldest first (flush order).
    pub fn list_pending_sync(&mut self) -> Result<Vec<PendingSync>, DbError> {
        let rows = pending_sync::table
            .order((
                pending_sync::offline_recorded_at.asc(),
                pending_sync::id.asc(),
            ))
            .select(PendingSyncRow::as_select())
            .load::<PendingSyncRow>(self.conn)?;
        Ok(rows.into_iter().map(PendingSync::from).collect())
    }

    /// Most recent pending report for a media item, if any (used to prefer an
    /// unsynced offline position over a stale source offset on resume).
    pub fn latest_pending_sync_for(
        &mut self,
        media_item_id: &str,
    ) -> Result<Option<PendingSync>, DbError> {
        let row = pending_sync::table
            .filter(pending_sync::media_item_id.eq(media_item_id))
            .order((
                pending_sync::offline_recorded_at.desc(),
                pending_sync::id.desc(),
            ))
            .select(PendingSyncRow::as_select())
            .first::<PendingSyncRow>(self.conn)
            .optional()?;
        Ok(row.map(PendingSync::from))
    }

    /// Delete a flushed pending-sync row by id.
    pub fn delete_pending_sync(&mut self, id: i64) -> Result<(), DbError> {
        diesel::delete(pending_sync::table.find(id)).execute(self.conn)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init::init_db;

    fn test_db() -> SqliteConnection {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        init_db(&mut conn).unwrap();
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
        let mut conn = test_db();
        let mut repo = DownloadsRepo::new(&mut conn);
        let d = sample_download("plex:src:1");
        repo.upsert(&d).unwrap();
        let found = repo.find("plex:src:1").unwrap().unwrap();
        assert_eq!(found, d);
    }

    #[test]
    fn upsert_is_idempotent_and_updates() {
        let mut conn = test_db();
        let mut repo = DownloadsRepo::new(&mut conn);
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
        let mut conn = test_db();
        let mut repo = DownloadsRepo::new(&mut conn);
        assert!(repo.find("nope").unwrap().is_none());
    }

    #[test]
    fn list_by_state_filters() {
        let mut conn = test_db();
        let mut repo = DownloadsRepo::new(&mut conn);
        repo.upsert(&sample_download("q1")).unwrap();
        repo.upsert(&completed("c1", 100, "2026-05-28T11:00:00Z"))
            .unwrap();

        let queued = repo.list_by_state(DownloadState::Queued).unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].media_item_id, "q1");
    }

    #[test]
    fn update_state_sets_fail_reason() {
        let mut conn = test_db();
        let mut repo = DownloadsRepo::new(&mut conn);
        repo.upsert(&sample_download("m1")).unwrap();
        repo.update_state("m1", DownloadState::Failed, Some(FailReason::DiskFull))
            .unwrap();
        let found = repo.find("m1").unwrap().unwrap();
        assert_eq!(found.state, DownloadState::Failed);
        assert_eq!(found.fail_reason, Some(FailReason::DiskFull));
    }

    #[test]
    fn group_membership_excludes_separately_enqueued_episode() {
        let mut conn = test_db();
        let mut repo = DownloadsRepo::new(&mut conn);
        repo.upsert_group(&DownloadGroup {
            id: "g1".to_string(),
            scope: GroupScope::Show,
            parent_media_id: "show1".to_string(),
            title: "The Show".to_string(),
            snapshot_at: "2026-05-28T10:00:00Z".to_string(),
        })
        .unwrap();

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
        let mut conn = test_db();
        let mut repo = DownloadsRepo::new(&mut conn);
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
        let mut conn = test_db();
        let mut repo = DownloadsRepo::new(&mut conn);
        repo.upsert(&completed("c1", 100, "2026-05-28T11:00:00Z"))
            .unwrap();
        repo.upsert(&completed("c2", 250, "2026-05-28T12:00:00Z"))
            .unwrap();
        repo.upsert(&sample_download("q1")).unwrap();
        assert_eq!(repo.total_completed_bytes().unwrap(), 350);
    }

    #[test]
    fn list_completed_oldest_first_orders_by_completion() {
        let mut conn = test_db();
        let mut repo = DownloadsRepo::new(&mut conn);
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
        let mut conn = test_db();
        let mut repo = DownloadsRepo::new(&mut conn);
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
        assert_eq!(pending[0].media_item_id, "m2");
        assert_eq!(pending[1].media_item_id, "m1");

        repo.delete_pending_sync(id1).unwrap();
        assert_eq!(repo.list_pending_sync().unwrap().len(), 1);
    }

    #[test]
    fn latest_pending_sync_for_returns_most_recent() {
        let mut conn = test_db();
        let mut repo = DownloadsRepo::new(&mut conn);
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
