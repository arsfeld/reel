use std::sync::Arc;

use diesel::SqliteConnection;
use relm4::ComponentSender;
use tracing::{debug, info, warn};

use crate::db::watch_progress_repo::WatchProgressRepo;
use crate::models::watch::WatchProgress;
use crate::services::media_source::MediaSource;
use crate::services::plex::source::PlexSource;
use crate::services::watch_state::WatchStateEvent;

use super::App;
use super::AppCmd;
use super::utils::iso_now;

pub fn dispatch_watch_events(
    db_conn: &mut Option<SqliteConnection>,
    events: Vec<WatchStateEvent>,
    source: &Option<Arc<PlexSource>>,
    sender: &ComponentSender<App>,
) {
    for event in events {
        match event {
            WatchStateEvent::PersistProgress {
                media_id,
                position,
                duration,
            } => {
                if let Some(conn) = db_conn.as_mut() {
                    let mut repo = WatchProgressRepo::new(conn);
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
                if let Some(conn) = db_conn.as_mut() {
                    let mut repo = WatchProgressRepo::new(conn);
                    let timestamp = iso_now();
                    if let Err(e) = repo.mark_watched(&media_id, &timestamp) {
                        warn!("Failed to mark as watched: {e}");
                    }
                }
                // Fire-and-forget Plex scrobble
                if !rating_key.is_empty()
                    && let Some(source) = source.clone()
                {
                    info!("Scrobble: rating_key={rating_key}");
                    sender.oneshot_command(async move {
                        if let Err(e) = source.scrobble(&rating_key).await {
                            warn!("Plex scrobble failed: {e}");
                        }
                        AppCmd::Noop
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
                    sender.oneshot_command(async move {
                        if let Err(e) = source
                            .report_progress(&rating_key, &state, time_ms, duration_ms)
                            .await
                        {
                            warn!("Plex timeline report failed: {e}");
                        }
                        AppCmd::Noop
                    });
                }
            }
        }
    }
}
