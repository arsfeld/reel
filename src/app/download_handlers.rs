//! Download manager wiring: the bridge between the pure
//! [`DownloadQueue`](crate::services::download::queue) scheduler, the resumable
//! [`DownloadClient`](crate::services::download::transfer), the
//! [`DownloadsRepo`](crate::db::downloads_repo), and the Relm4 app shell.
//!
//! The queue stays pure (it returns [`DownloadEvent`]s as data); this module
//! performs the effects: it spawns a bounded number of transfer tasks, streams
//! their progress back to the GTK loop through a channel (mirroring the MPRIS
//! bridge), persists every state transition to the repo, and recovers in-flight
//! downloads on startup. Failure handling is split by cause via the pure
//! [`failure_policy`] so each `FailReason` maps to a distinct recovery posture.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use relm4::ComponentController;
use rusqlite::Connection;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::db::downloads_repo::DownloadsRepo;
use crate::models::download::{Download, DownloadState, FailReason};
use crate::models::media::MediaItem;
use crate::services::download::queue::{DownloadEvent, DownloadQueue, QueueItem};
use crate::services::download::transfer::{
    DownloadClient, TransferResult, confined_media_path, part_path,
};
use crate::services::download::{DownloadError, prune};
use crate::services::media_source::MediaSource;

use super::App;
use super::utils::iso_now;

/// Only forward a progress tick to the UI once this many bytes have accumulated
/// since the last one, so a fast transfer doesn't flood the GTK loop.
const PROGRESS_BYTES_DELTA: u64 = 1_048_576; // 1 MiB

/// A message streamed from a background transfer task back to the GTK loop.
#[derive(Debug)]
#[allow(dead_code)] // `total` feeds the U13 progress UI.
pub enum DownloadRunnerMsg {
    /// A transfer advanced; advisory byte counts for the UI.
    Progress {
        media_item_id: String,
        downloaded: u64,
        total: Option<u64>,
    },
    /// A transfer finished (success or error).
    Finished {
        media_item_id: String,
        result: Result<TransferResult, DownloadError>,
    },
}

pub use crate::components::downloads::DownloadItemAction;

/// How a failed transfer should affect the queue and UI, derived purely from
/// the failure cause. Each `FailReason` maps to a distinct recovery posture
/// (plan Key Technical Decision: "failure is split by cause, not a single
/// `Failed`").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FailurePolicy {
    /// Pause every active transfer (used for `DiskFull`: retrying just refails).
    pub pause_queue: bool,
    /// Raise the persistent disk-full banner.
    pub disk_full_banner: bool,
    /// Park the item and surface a single re-auth prompt (`AuthExpired`).
    pub prompt_reauth: bool,
    /// Re-queue the item for a bounded auto-retry (`Network`).
    pub auto_retry: bool,
}

/// Map a failure reason to its recovery posture. Pure and fully unit-testable.
pub fn failure_policy(reason: FailReason) -> FailurePolicy {
    match reason {
        FailReason::Network => FailurePolicy {
            auto_retry: true,
            ..FailurePolicy::default()
        },
        FailReason::DiskFull => FailurePolicy {
            pause_queue: true,
            disk_full_banner: true,
            ..FailurePolicy::default()
        },
        FailReason::AuthExpired => FailurePolicy {
            prompt_reauth: true,
            ..FailurePolicy::default()
        },
        // A changed remote file or a missing local file are terminal until the
        // user re-downloads — no automatic recovery.
        FailReason::SourceFileChanged | FailReason::FileMissing => FailurePolicy::default(),
    }
}

/// Build a self-contained download row from a library item, taking the part key
/// from the source's download descriptor. `None` when the item is not
/// downloadable (no file, or the source doesn't support downloads).
pub fn new_download_row(
    item: &MediaItem,
    part_key: &str,
    queue_order: i64,
    group_id: Option<String>,
) -> Download {
    Download {
        media_item_id: item.id.clone(),
        part_key: part_key.to_string(),
        source_type: item.source_type,
        source_id: item.source_id.clone(),
        state: DownloadState::Queued,
        fail_reason: None,
        byte_count: 0,
        total_size: None,
        validator: None,
        file_path: None,
        group_id,
        queue_order,
        enqueued_at: iso_now(),
        completed_at: None,
        media_type: item.media_type,
        title: item.title.clone(),
        year: item.year,
        parent_id: item.parent_id.clone(),
        season_number: item.season_number,
        episode_number: item.episode_number,
        // Prefer the item's own poster; fall back to the series poster for
        // episodes so a grouped row still has artwork offline.
        poster_path: item
            .poster_path
            .clone()
            .or_else(|| item.series_poster_path.clone()),
    }
}

/// Resolved arguments for a single transfer task.
struct TransferParams {
    media_item_id: String,
    url: String,
    validator: Option<String>,
    media_path: PathBuf,
    expected_size: Option<u64>,
}

/// Owns the live download state: the pure queue, the in-flight task handles, the
/// shared transfer client, and the banner flags. Held by [`App`].
pub struct DownloadManager {
    pub queue: DownloadQueue,
    /// Abort handles for in-flight transfers, keyed by `media_item_id`.
    active: HashMap<String, JoinHandle<()>>,
    /// Shared transfer client (built once; `None` if construction failed).
    client: Option<Arc<DownloadClient>>,
    /// Sender cloned into each transfer task; the matching receiver is relayed
    /// to the GTK loop at app init.
    tx: mpsc::UnboundedSender<DownloadRunnerMsg>,
    /// Disk-full banner is showing (queue paused until space is freed).
    pub disk_full: bool,
    /// Over-budget warn-only banner is showing (only unwatched downloads remain
    /// over budget; nothing was auto-deleted).
    pub over_budget_warning: bool,
}

impl DownloadManager {
    /// Create a manager plus the receiver the app relays into `AppMsg`.
    pub fn new(concurrency: usize) -> (Self, mpsc::UnboundedReceiver<DownloadRunnerMsg>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let client = DownloadClient::new()
            .map(Arc::new)
            .map_err(|e| warn!("Failed to build download client: {e}"))
            .ok();
        (
            Self {
                queue: DownloadQueue::new(concurrency),
                active: HashMap::new(),
                client,
                tx,
                disk_full: false,
                over_budget_warning: false,
            },
            rx,
        )
    }

    /// Abort an in-flight transfer task, if any. The `.part` file is left on
    /// disk (kept for a later resume; removed by the caller on cancel/delete).
    fn abort(&mut self, media_item_id: &str) {
        if let Some(handle) = self.active.remove(media_item_id) {
            handle.abort();
        }
    }

    /// Spawn a transfer task that streams progress and a final result back
    /// through the channel.
    fn spawn(&mut self, params: TransferParams) {
        let Some(client) = self.client.clone() else {
            warn!("No download client; cannot start transfer");
            return;
        };
        let tx = self.tx.clone();
        let progress_tx = tx.clone();
        let id = params.media_item_id.clone();
        let id_for_progress = params.media_item_id.clone();
        let handle = relm4::spawn(async move {
            let mut last_sent = 0u64;
            let result = client
                .download(
                    &params.url,
                    params.validator.as_deref(),
                    &params.media_path,
                    params.expected_size,
                    |p| {
                        let done = p.total == Some(p.downloaded);
                        if done || p.downloaded.saturating_sub(last_sent) >= PROGRESS_BYTES_DELTA {
                            last_sent = p.downloaded;
                            let _ = progress_tx.send(DownloadRunnerMsg::Progress {
                                media_item_id: id_for_progress.clone(),
                                downloaded: p.downloaded,
                                total: p.total,
                            });
                        }
                    },
                )
                .await;
            let _ = tx.send(DownloadRunnerMsg::Finished {
                media_item_id: id,
                result,
            });
        });
        self.active.insert(params.media_item_id, handle);
    }
}

impl App {
    /// Show a transient toast. Used by the download handlers, which hold
    /// `&mut App` rather than a sender.
    pub(super) fn toast(&self, message: &str) {
        let toast = adw::Toast::new(message);
        toast.set_timeout(3);
        self.toast_overlay.add_toast(toast);
    }

    /// Whether a completed local download exists for an item (badge predicate,
    /// R14). Only a `Completed` download is offline-ready.
    pub(super) fn is_downloaded(&self, media_item_id: &str) -> bool {
        self.db_conn
            .as_ref()
            .and_then(|conn| DownloadsRepo::new(conn).find(media_item_id).ok().flatten())
            .map(|d| crate::services::download::sidecar::shows_downloaded_badge(d.state))
            .unwrap_or(false)
    }

    /// Push the current download state and banner flags to the Downloads view.
    pub(super) fn refresh_downloads_view(&self) {
        use crate::components::downloads::DownloadsViewMsg;
        let (downloads, groups) = match self.db_conn.as_ref() {
            Some(conn) => {
                let repo = DownloadsRepo::new(conn);
                (
                    repo.list_all().unwrap_or_default(),
                    repo.list_groups().unwrap_or_default(),
                )
            }
            None => (Vec::new(), Vec::new()),
        };
        self.downloads_view.emit(DownloadsViewMsg::SetDownloads {
            downloads,
            groups,
            over_budget_warning: self.downloads.over_budget_warning,
            disk_full: self.downloads.disk_full,
        });
    }
}

// --- handler free functions (operate on &mut App, keep update() thin) ---

/// Borrow the repo from the app's connection, if the DB is available.
fn with_repo<T>(app: &App, f: impl FnOnce(&DownloadsRepo) -> T) -> Option<T> {
    let conn = app.db_conn.as_ref()?;
    Some(f(&DownloadsRepo::new(conn)))
}

/// Persist the queue's current state for `media_item_id` (state + optional fail
/// reason). A no-op if the DB is unavailable.
fn persist_state(
    conn: &Option<Connection>,
    media_item_id: &str,
    state: DownloadState,
    fail_reason: Option<FailReason>,
) {
    if let Some(conn) = conn {
        let repo = DownloadsRepo::new(conn);
        if let Err(e) = repo.update_state(media_item_id, state, fail_reason) {
            warn!("Failed to persist download state for {media_item_id}: {e}");
        }
    }
}

/// Resolve the transfer arguments for a queued item from its repo row.
fn resolve_transfer(app: &App, media_item_id: &str, part_key: &str) -> Option<TransferParams> {
    let source = app.active_source.clone()?;
    let folder = app.settings.downloads.effective_folder();
    let media_path = confined_media_path(&folder, media_item_id, part_key)
        .map_err(|e| warn!("Refusing unsafe download path: {e}"))
        .ok()?;
    let row = match with_repo(app, |repo| repo.find(media_item_id)) {
        Some(Ok(Some(row))) => row,
        _ => return None,
    };
    Some(TransferParams {
        media_item_id: media_item_id.to_string(),
        url: source.playback_url(part_key),
        validator: row.validator,
        media_path,
        expected_size: row.total_size.map(|s| s as u64),
    })
}

/// Perform the effects of a batch of queue events: start transfers (persisting
/// `Downloading` and spawning a task) and cancel in-flight ones.
fn apply_events(app: &mut App, events: Vec<DownloadEvent>) {
    for event in events {
        match event {
            DownloadEvent::StartTransfer {
                media_item_id,
                part_key,
            } => {
                persist_state(
                    &app.db_conn,
                    &media_item_id,
                    DownloadState::Downloading,
                    None,
                );
                if let Some(params) = resolve_transfer(app, &media_item_id, &part_key) {
                    info!("Starting download transfer: {media_item_id}");
                    app.downloads.spawn(params);
                } else {
                    // Couldn't resolve (no source / bad path): mark failed so the
                    // slot frees and the user can retry.
                    let reschedule = app.downloads.queue.mark_failed(&media_item_id);
                    persist_state(
                        &app.db_conn,
                        &media_item_id,
                        DownloadState::Failed,
                        Some(FailReason::Network),
                    );
                    apply_events(app, reschedule);
                }
            }
            DownloadEvent::CancelTransfer { media_item_id } => {
                app.downloads.abort(&media_item_id);
            }
        }
    }
}

/// Enqueue a single library item (movie or episode) for download.
pub fn enqueue_download(app: &mut App, item: &MediaItem) {
    enqueue_item(app, item, None);
}

/// Enqueue an item, optionally as a member of a group. Upserts the self-contained
/// row, then drives the pure queue and performs the resulting effects.
pub fn enqueue_item(app: &mut App, item: &MediaItem, group_id: Option<String>) {
    let Some(source) = app.active_source.clone() else {
        app.toast("Connect a source before downloading");
        return;
    };
    let descriptor = match source.download_descriptor(item) {
        Ok(d) => d,
        Err(e) => {
            warn!("Item not downloadable: {e}");
            app.toast("This item can't be downloaded");
            return;
        }
    };

    // Snapshot the row (idempotent: an existing non-removed row is left as-is by
    // the queue, but we still upsert so a re-trigger refreshes metadata).
    let order = app.downloads.queue.items().len() as i64;
    let already = matches!(
        with_repo(app, |repo| repo.find(&item.id)),
        Some(Ok(Some(_)))
    );
    if !already {
        let row = new_download_row(item, &descriptor.part_key, order, group_id);
        if let Err(e) = with_repo(app, |repo| repo.upsert(&row)).transpose() {
            warn!("Failed to persist download row: {e}");
        }
    }

    let events = app.downloads.queue.enqueue(&item.id, &descriptor.part_key);
    apply_events(app, events);
    app.refresh_downloads_view();
}

/// Enqueue a show/season download: snapshot the currently-downloadable episodes
/// (R5), record the group, and enqueue the missing ones as group members. A
/// re-trigger tops up only episodes not already present (R6) — `enqueue_item`'s
/// idempotency and the pure queue handle the skip.
pub fn enqueue_group(
    app: &mut App,
    parent: &MediaItem,
    episodes: &[MediaItem],
    scope: crate::models::download::GroupScope,
) {
    use crate::services::download::snapshot;
    let set: Vec<MediaItem> = snapshot::snapshot_set(episodes)
        .into_iter()
        .cloned()
        .collect();
    if set.is_empty() {
        app.toast("No downloadable episodes");
        return;
    }
    let group_id = format!("group:{}", parent.id);
    let group = crate::models::download::DownloadGroup {
        id: group_id.clone(),
        scope,
        parent_media_id: parent.id.clone(),
        title: parent.title.clone(),
        snapshot_at: iso_now(),
    };
    if let Some(conn) = app.db_conn.as_ref()
        && let Err(e) = DownloadsRepo::new(conn).upsert_group(&group)
    {
        warn!("Failed to persist download group: {e}");
    }
    for ep in &set {
        enqueue_item(app, ep, Some(group_id.clone()));
    }
    app.toast(&format!("Downloading {}", parent.title));
}

/// Apply a per-item action from the UI (pause/resume/cancel/delete/retry).
pub fn item_action(app: &mut App, media_item_id: &str, action: DownloadItemAction) {
    let events = match action {
        DownloadItemAction::Pause => {
            let e = app.downloads.queue.pause(media_item_id);
            persist_state(&app.db_conn, media_item_id, DownloadState::Paused, None);
            e
        }
        DownloadItemAction::Resume | DownloadItemAction::Retry => {
            let e = app.downloads.queue.resume(media_item_id);
            // Resume/retry returns the item to Queued; clear any prior reason.
            persist_state(&app.db_conn, media_item_id, DownloadState::Queued, None);
            e
        }
        DownloadItemAction::Cancel | DownloadItemAction::Delete => {
            let e = app.downloads.queue.cancel(media_item_id);
            delete_download_files(app, media_item_id);
            with_repo(app, |repo| {
                if let Err(err) = repo.delete(media_item_id) {
                    warn!("Failed to delete download row {media_item_id}: {err}");
                }
            });
            e
        }
    };
    apply_events(app, events);
    app.refresh_downloads_view();
}

/// Reorder a queued item (queued items only — see [`DownloadQueue::reorder`]).
pub fn reorder_download(app: &mut App, media_item_id: &str, new_index: usize) {
    app.downloads.queue.reorder(media_item_id, new_index);
    // Persist the new ordering so it survives a restart.
    persist_queue_order(app);
    app.refresh_downloads_view();
}

/// Write the queue's current ordering back to each row's `queue_order`.
fn persist_queue_order(app: &App) {
    let ids: Vec<String> = app
        .downloads
        .queue
        .items()
        .iter()
        .map(|i| i.media_item_id.clone())
        .collect();
    if let Some(conn) = app.db_conn.as_ref() {
        let repo = DownloadsRepo::new(conn);
        for (order, id) in ids.iter().enumerate() {
            if let Ok(Some(mut row)) = repo.find(id) {
                row.queue_order = order as i64;
                let _ = repo.upsert(&row);
            }
        }
    }
}

/// Remove a download's media, `.part`, and sidecar files from disk.
fn delete_download_files(app: &App, media_item_id: &str) {
    let row = match with_repo(app, |repo| repo.find(media_item_id)) {
        Some(Ok(Some(row))) => row,
        _ => return,
    };
    if let Some(ref fp) = row.file_path {
        let media_path = PathBuf::from(fp);
        let _ = std::fs::remove_file(&media_path);
        let _ = std::fs::remove_file(crate::services::download::sidecar::sidecar_path(
            &media_path,
        ));
        let _ = std::fs::remove_file(part_path(&media_path));
    } else {
        // Never completed: the .part lives at the confined path.
        if let Ok(media_path) = confined_media_path(
            &app.settings.downloads.effective_folder(),
            media_item_id,
            &row.part_key,
        ) {
            let _ = std::fs::remove_file(part_path(&media_path));
        }
    }
}

/// Handle a runner message streamed from a transfer task.
pub fn handle_runner_msg(app: &mut App, msg: DownloadRunnerMsg) {
    match msg {
        DownloadRunnerMsg::Progress {
            media_item_id,
            downloaded,
            ..
        } => {
            // Advisory byte count for the UI; the authoritative offset is the
            // on-disk `.part` length, so we only update the counter.
            if let Some(conn) = app.db_conn.as_ref()
                && let Ok(Some(mut row)) = DownloadsRepo::new(conn).find(&media_item_id)
            {
                row.byte_count = downloaded as i64;
                let _ = DownloadsRepo::new(conn).upsert(&row);
            }
            app.refresh_downloads_view();
        }
        DownloadRunnerMsg::Finished {
            media_item_id,
            result,
        } => handle_transfer_finished(app, &media_item_id, result),
    }
}

/// Persist a finished transfer's outcome, write its sidecar on success, drive
/// the queue forward, and apply the failure policy on error.
fn handle_transfer_finished(
    app: &mut App,
    media_item_id: &str,
    result: Result<TransferResult, DownloadError>,
) {
    app.downloads.active.remove(media_item_id);
    match result {
        Ok(transfer) => {
            finalize_completed(app, media_item_id, &transfer);
            let events = app.downloads.queue.mark_completed(media_item_id);
            apply_events(app, events);
            // A completion may push usage over budget — prune watched items.
            enforce_budget(app, media_item_id);
        }
        Err(err) => {
            let reason = err.fail_reason();
            warn!("Download {media_item_id} failed: {err} ({reason:?})");
            let policy = failure_policy(reason);
            let events = app.downloads.queue.mark_failed(media_item_id);
            persist_state(
                &app.db_conn,
                media_item_id,
                DownloadState::Failed,
                Some(reason),
            );

            if policy.pause_queue {
                pause_all_active(app);
            }
            if policy.disk_full_banner {
                app.downloads.disk_full = true;
            }
            // Auto-retry / re-auth are scheduled by re-enqueue or a re-auth
            // prompt at the UI layer; the v1 policy keeps these simple.
            apply_events(app, events);
        }
    }
    app.refresh_downloads_view();
}

/// Mark a download complete: record total size + validator + final path, set
/// `completed_at`, and write the self-contained sidecar.
fn finalize_completed(app: &App, media_item_id: &str, transfer: &TransferResult) {
    let Some(conn) = app.db_conn.as_ref() else {
        return;
    };
    let repo = DownloadsRepo::new(conn);
    let Ok(Some(mut row)) = repo.find(media_item_id) else {
        return;
    };
    row.state = DownloadState::Completed;
    row.fail_reason = None;
    row.total_size = Some(transfer.total_size as i64);
    row.byte_count = transfer.total_size as i64;
    row.validator = transfer.validator.clone();
    row.file_path = Some(transfer.final_path.to_string_lossy().into_owned());
    row.completed_at = Some(iso_now());
    if let Err(e) = repo.upsert(&row) {
        warn!("Failed to persist completed download {media_item_id}: {e}");
        return;
    }
    if let Err(e) = crate::services::download::sidecar::write_sidecar(&transfer.final_path, &row) {
        warn!("Failed to write sidecar for {media_item_id}: {e}");
    }
}

/// Pause every active transfer (disk-full recovery posture).
fn pause_all_active(app: &mut App) {
    let active: Vec<String> = app.downloads.active.keys().cloned().collect();
    for id in active {
        let events = app.downloads.queue.pause(&id);
        persist_state(&app.db_conn, &id, DownloadState::Paused, None);
        // pause() emits CancelTransfer for the aborted task; do not reschedule
        // (the queue is paused), so only honor the cancels.
        for event in events {
            if let DownloadEvent::CancelTransfer { media_item_id } = event {
                app.downloads.abort(&media_item_id);
            }
        }
    }
}

/// After a completion, prune the oldest *watched* downloads if over budget.
/// Unwatched downloads are never auto-deleted; if only unwatched remain over
/// budget, raise the warn-only banner instead.
fn enforce_budget(app: &mut App, now_playing_id: &str) {
    let Some(budget) = app.settings.downloads.max_bytes else {
        return;
    };
    let candidates = collect_prune_candidates(app);
    let total: u64 = candidates.iter().map(|c| c.size_bytes).sum();
    let outcome =
        prune::select_prune_candidates(&candidates, total, Some(budget), Some(now_playing_id));
    for id in &outcome.to_prune {
        delete_download_files(app, id);
        persist_state(&app.db_conn, id, DownloadState::Pruned, None);
        let events = app.downloads.queue.cancel(id); // remove from scheduler
        for event in events {
            if let DownloadEvent::CancelTransfer { media_item_id } = event {
                app.downloads.abort(&media_item_id);
            }
        }
        info!("Pruned watched download {id} to stay under budget");
    }
    app.downloads.over_budget_warning = outcome.warn;
}

/// Build prune candidates by joining completed downloads with their watch state.
fn collect_prune_candidates(app: &App) -> Vec<prune::PruneCandidate> {
    let Some(conn) = app.db_conn.as_ref() else {
        return Vec::new();
    };
    let repo = DownloadsRepo::new(conn);
    let Ok(completed) = repo.list_completed_oldest_first() else {
        return Vec::new();
    };
    let watch_repo = crate::db::watch_progress_repo::WatchProgressRepo::new(conn);
    completed
        .into_iter()
        .map(|d| {
            let progress = watch_repo.find_by_media_id(&d.media_item_id).ok().flatten();
            let (watched, last_watched_at) = progress
                .map(|p| (p.watched, Some(p.last_watched_at)))
                .unwrap_or((false, None));
            prune::PruneCandidate {
                media_item_id: d.media_item_id,
                size_bytes: d.total_size.or(Some(d.byte_count)).unwrap_or(0).max(0) as u64,
                watched,
                last_watched_at,
                completed_at: d.completed_at,
            }
        })
        .collect()
}

/// Rebuild the pure queue from persisted rows and recover any download left
/// `Downloading` by a crash (reset to `Queued`). Also reconciles on-disk files
/// against the DB (re-adopting post-wipe downloads). Called once at app init.
///
/// Does **not** start transfers: the source connects asynchronously after init,
/// so scheduling is deferred to [`start_pending`], called once the source is
/// validated (and after every enqueue).
pub fn recover_on_startup(app: &mut App) {
    // 1. Reconcile DB rows against disk (flag missing files, re-adopt sidecars).
    let folder = app.settings.downloads.effective_folder();
    if let Some(conn) = app.db_conn.as_ref() {
        let repo = DownloadsRepo::new(conn);
        match crate::services::download::sidecar::verify_downloads(&repo, &folder) {
            Ok(report) => {
                if !report.readopted.is_empty() || !report.failed_missing.is_empty() {
                    info!(
                        "Download reconcile: {} re-adopted, {} missing, {} unmanaged",
                        report.readopted.len(),
                        report.failed_missing.len(),
                        report.unmanaged.len()
                    );
                }
            }
            Err(e) => warn!("Download reconcile failed: {e}"),
        }
    }

    // 2. Rebuild the queue from persisted rows.
    let rows = with_repo(app, |repo| repo.list_all())
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let items: Vec<QueueItem> = rows
        .iter()
        .filter(|d| {
            !matches!(
                d.state,
                DownloadState::Removed | DownloadState::Pruned | DownloadState::Completed
            )
        })
        .map(|d| QueueItem {
            media_item_id: d.media_item_id.clone(),
            part_key: d.part_key.clone(),
            state: d.state,
        })
        .collect();
    let concurrency = app.settings.downloads.effective_concurrency();
    app.downloads.queue = DownloadQueue::from_items(items, concurrency);

    // 3. A crashed Downloading row has no running task — reset to Queued for
    //    re-validation rather than resuming blind, then schedule.
    app.downloads.queue.recover_on_startup();
    for id in app
        .downloads
        .queue
        .items()
        .iter()
        .filter(|i| i.state == DownloadState::Queued)
        .map(|i| i.media_item_id.clone())
        .collect::<Vec<_>>()
    {
        persist_state(&app.db_conn, &id, DownloadState::Queued, None);
    }
    app.refresh_downloads_view();
}

/// Schedule any queued downloads now that a source is available. Called when the
/// source is validated at startup and after a reconnect. No-op without a source
/// (you can't download while offline) or while the queue is disk-full paused.
pub fn start_pending(app: &mut App) {
    if app.active_source.is_none() || app.downloads.disk_full {
        return;
    }
    let events = app.downloads.queue.schedule();
    apply_events(app, events);
    app.refresh_downloads_view();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_failure_auto_retries() {
        let p = failure_policy(FailReason::Network);
        assert!(p.auto_retry);
        assert!(!p.pause_queue);
        assert!(!p.disk_full_banner);
        assert!(!p.prompt_reauth);
    }

    #[test]
    fn disk_full_pauses_queue_and_shows_banner() {
        let p = failure_policy(FailReason::DiskFull);
        assert!(p.pause_queue);
        assert!(p.disk_full_banner);
        assert!(!p.auto_retry);
    }

    #[test]
    fn auth_expired_prompts_reauth_without_pausing() {
        let p = failure_policy(FailReason::AuthExpired);
        assert!(p.prompt_reauth);
        assert!(!p.pause_queue);
        assert!(!p.auto_retry);
    }

    #[test]
    fn changed_or_missing_file_has_no_auto_recovery() {
        for r in [FailReason::SourceFileChanged, FailReason::FileMissing] {
            let p = failure_policy(r);
            assert_eq!(p, FailurePolicy::default());
        }
    }

    fn episode_item() -> MediaItem {
        use crate::models::media::{MediaType, SourceType};
        MediaItem {
            id: "plex:srv:42".to_string(),
            source_type: SourceType::Plex,
            source_id: "http://localhost:32400".to_string(),
            external_id: "42".to_string(),
            media_type: MediaType::Episode,
            title: "Pilot".to_string(),
            year: None,
            overview: None,
            content_rating: None,
            rating: None,
            runtime_minutes: Some(45),
            poster_path: None,
            series_poster_path: Some("/library/metadata/9/thumb".to_string()),
            backdrop_path: None,
            genres: vec![],
            parent_id: Some("show1".to_string()),
            season_number: Some(1),
            episode_number: Some(1),
            air_date: None,
            file_path: Some("/library/parts/1/file.mkv".to_string()),
            video_resolution: None,
            hdr: None,
            added_at: "2026-01-01".to_string(),
            updated_at: "2026-01-01".to_string(),
            playback_position_ms: None,
            watched: false,
            library_section_id: None,
        }
    }

    #[test]
    fn new_download_row_snapshots_metadata_and_falls_back_to_series_poster() {
        let item = episode_item();
        let row = new_download_row(&item, "/library/parts/1/file.mkv", 3, Some("g1".into()));
        assert_eq!(row.media_item_id, "plex:srv:42");
        assert_eq!(row.state, DownloadState::Queued);
        assert_eq!(row.queue_order, 3);
        assert_eq!(row.group_id.as_deref(), Some("g1"));
        assert_eq!(row.part_key, "/library/parts/1/file.mkv");
        // Episode has no own poster -> falls back to the series poster.
        assert_eq!(
            row.poster_path.as_deref(),
            Some("/library/metadata/9/thumb")
        );
    }
}
