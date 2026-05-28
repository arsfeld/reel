//! Dispatch of watch-state events to local DB + the source, plus the offline
//! progress capture / sync-back path (R20/R21).
//!
//! When a timeline/scrobble report fails because the source is unreachable, the
//! event is queued to `pending_sync` (carrying `offline_recorded_at`) instead of
//! being lost. On reconnect [`flush_pending_sync`] replays the queued reports to
//! the source. Resume preference is the one sanctioned inversion of "Plex is
//! authoritative" (see memory `feedback_plex_authoritative_watch_state`): an
//! unsynced offline position wins over the source's own (stale) offset — see
//! [`resume_position`].

use std::sync::Arc;

use relm4::ComponentSender;
use rusqlite::Connection;
use tracing::{debug, info, warn};

use crate::db::downloads_repo::DownloadsRepo;
use crate::db::watch_progress_repo::WatchProgressRepo;
use crate::models::download::{PendingSync, SyncKind};
use crate::models::watch::WatchProgress;
use crate::services::media_source::MediaSource;
use crate::services::plex::source::PlexSource;
use crate::services::watch_state::WatchStateEvent;

use super::App;
use super::AppCmd;
use super::utils::iso_now;

/// Choose the resume position when starting playback.
///
/// Offline-recorded progress for a downloaded item wins over the source's own
/// offset — the one sanctioned inversion of "Plex is authoritative", because
/// offline progress is genuinely newer than whatever the source last saw. If
/// the source already considers the item *watched*, that terminal state wins and
/// the offline mid-progress position is ignored (latest-state-wins). With no
/// offline progress, the source offset wins, then local tracked progress.
pub fn resume_position(
    source_offset: Option<f64>,
    offline_pending: Option<f64>,
    local_tracked: Option<f64>,
    remote_watched: bool,
) -> Option<f64> {
    if remote_watched {
        // Source says watched: don't resume from a stale offline mid-progress.
        return source_offset.or(local_tracked);
    }
    offline_pending.or(source_offset).or(local_tracked)
}

pub fn dispatch_watch_events(
    db_conn: &Option<Connection>,
    events: Vec<WatchStateEvent>,
    source: &Option<Arc<PlexSource>>,
    media_id: Option<&str>,
    sender: &ComponentSender<App>,
) {
    for event in events {
        match event {
            WatchStateEvent::PersistProgress {
                media_id,
                position,
                duration,
            } => {
                if let Some(conn) = db_conn {
                    let repo = WatchProgressRepo::new(conn);
                    let progress = WatchProgress {
                        media_item_id: media_id,
                        position_seconds: position,
                        duration_seconds: duration,
                        watched: false,
                        last_watched_at: iso_now(),
                    };
                    if let Err(e) = repo.upsert(&progress) {
                        warn!("Failed to persist watch progress: {e}");
                    }
                }
            }
            WatchStateEvent::Scrobble {
                media_id,
                rating_key,
            } => {
                // Mark as watched locally
                if let Some(conn) = db_conn {
                    let repo = WatchProgressRepo::new(conn);
                    let timestamp = iso_now();
                    if let Err(e) = repo.mark_watched(&media_id, &timestamp) {
                        warn!("Failed to mark as watched: {e}");
                    }
                }
                // Fire-and-forget Plex scrobble; queue offline on failure.
                if !rating_key.is_empty()
                    && let Some(source) = source.clone()
                {
                    info!("Scrobble: rating_key={rating_key}");
                    let media_id = media_id.clone();
                    sender.oneshot_command(async move {
                        match source.scrobble(&rating_key).await {
                            Ok(()) => AppCmd::Noop,
                            Err(e) => {
                                warn!("Plex scrobble failed (queuing offline): {e}");
                                AppCmd::QueueOfflineSync(PendingSync {
                                    id: None,
                                    media_item_id: media_id,
                                    rating_key,
                                    position_ms: 0,
                                    duration_ms: 0,
                                    kind: SyncKind::Scrobble,
                                    offline_recorded_at: iso_now(),
                                })
                            }
                        }
                    });
                }
            }
            WatchStateEvent::ReportTimeline {
                rating_key,
                state,
                time_ms,
                duration_ms,
            } => {
                if !rating_key.is_empty()
                    && let Some(source) = source.clone()
                {
                    debug!("Timeline: key={rating_key} state={state} time={time_ms}ms");
                    let media_id = media_id.map(str::to_string);
                    sender.oneshot_command(async move {
                        match source
                            .report_progress(&rating_key, &state, time_ms, duration_ms)
                            .await
                        {
                            Ok(()) => AppCmd::Noop,
                            Err(e) => {
                                warn!("Plex timeline report failed (queuing offline): {e}");
                                match media_id {
                                    Some(media_item_id) => AppCmd::QueueOfflineSync(PendingSync {
                                        id: None,
                                        media_item_id,
                                        rating_key,
                                        position_ms: time_ms,
                                        duration_ms,
                                        kind: SyncKind::Timeline,
                                        offline_recorded_at: iso_now(),
                                    }),
                                    None => AppCmd::Noop,
                                }
                            }
                        }
                    });
                }
            }
        }
    }
}

/// Persist a progress report that failed to reach the source while offline.
/// Keyed for replay on reconnect by [`flush_pending_sync`].
pub fn queue_offline_sync(db_conn: &Option<Connection>, pending: &PendingSync) {
    if let Some(conn) = db_conn {
        let repo = DownloadsRepo::new(conn);
        if let Err(e) = repo.insert_pending_sync(pending) {
            warn!("Failed to queue offline progress: {e}");
        } else {
            info!(
                "Queued offline progress for {} ({:?})",
                pending.media_item_id, pending.kind
            );
        }
    }
}

/// Replay queued offline progress to the source on reconnect, oldest first.
///
/// Runs before any inbound library browse can surface a stale source offset, so
/// the user's offline viewing position is preserved. Successfully-flushed rows
/// are deleted via [`AppCmd::FlushedPending`].
pub fn flush_pending_sync(app: &App, sender: &ComponentSender<App>) {
    let Some(conn) = app.db_conn.as_ref() else {
        return;
    };
    let pending = match DownloadsRepo::new(conn).list_pending_sync() {
        Ok(p) if !p.is_empty() => p,
        Ok(_) => return,
        Err(e) => {
            warn!("Failed to read pending offline progress: {e}");
            return;
        }
    };
    let Some(source) = app.active_source.clone() else {
        return;
    };
    info!(
        "Flushing {} queued offline progress report(s)",
        pending.len()
    );
    sender.oneshot_command(async move {
        let mut flushed = Vec::new();
        for p in pending {
            let Some(id) = p.id else { continue };
            let result = match p.kind {
                SyncKind::Scrobble => source.scrobble(&p.rating_key).await,
                SyncKind::Timeline => {
                    source
                        .report_progress(&p.rating_key, "stopped", p.position_ms, p.duration_ms)
                        .await
                }
            };
            match result {
                Ok(()) => flushed.push(id),
                // Still unreachable: keep the row for a later attempt.
                Err(e) => warn!("Offline flush failed for {}: {e}", p.media_item_id),
            }
        }
        AppCmd::FlushedPending(flushed)
    });
}

/// Delete pending-sync rows that were successfully flushed to the source.
pub fn delete_flushed_pending(db_conn: &Option<Connection>, ids: &[i64]) {
    if let Some(conn) = db_conn {
        let repo = DownloadsRepo::new(conn);
        for id in ids {
            if let Err(e) = repo.delete_pending_sync(*id) {
                warn!("Failed to delete flushed pending row {id}: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_pending_wins_over_source_offset() {
        // A downloaded item watched offline resumes from the local position.
        assert_eq!(
            resume_position(Some(100.0), Some(1800.0), None, false),
            Some(1800.0)
        );
    }

    #[test]
    fn source_offset_wins_when_no_pending() {
        // Online behavior unchanged: no pending -> prefer the source offset.
        assert_eq!(
            resume_position(Some(100.0), None, Some(50.0), false),
            Some(100.0)
        );
    }

    #[test]
    fn local_tracked_used_when_no_source_or_pending() {
        assert_eq!(resume_position(None, None, Some(42.0), false), Some(42.0));
    }

    #[test]
    fn remote_watched_drops_offline_pending() {
        // Conflict: Plex shows watched, local pending shows mid-progress ->
        // Plex wins, the offline position is ignored.
        assert_eq!(resume_position(None, Some(1800.0), None, true), None);
        // A watched item with a source offset still ignores the offline pending.
        assert_eq!(
            resume_position(Some(0.0), Some(1800.0), None, true),
            Some(0.0)
        );
    }

    #[test]
    fn nothing_to_resume() {
        assert_eq!(resume_position(None, None, None, false), None);
    }
}
