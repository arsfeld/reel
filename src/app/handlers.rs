use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use adw::prelude::*;

use relm4::prelude::*;
use tracing::{debug, info};

use crate::components::home::HomeViewMsg;
use crate::components::library::LibraryViewMsg;
use crate::components::player::video_player::{VideoPlayerMsg, VideoPlayerOutput};
use crate::components::sidebar::SidebarMsg;
use crate::config;
use crate::db::downloads_repo::DownloadsRepo;
use crate::db::watch_progress_repo::WatchProgressRepo;
use crate::models::download::DownloadState;
use crate::models::media::{MediaItem, SourceType};
use crate::models::source::{Source, SourceConfig};
use crate::navigation::CurrentView;
use crate::player::PlayState;
use crate::services::artwork::ArtworkCache;
use crate::services::media_source::MediaSource;
use crate::services::mpris;
use crate::services::plex::api::PlexClient;
use crate::services::plex::source::PlexSource;
use crate::services::watch_state::PlaybackState;

use super::App;
use super::AppCmd;
use super::AppMsg;
use super::db_helpers::load_watch_data;
use super::player_ui::{enter_player_mode, leave_player_mode, player_title_for_item};
use super::watch_events::{dispatch_watch_events, resume_position};

/// Handle VideoOutput messages from the video player component.
#[allow(clippy::too_many_lines)]
pub fn handle_video_output(
    app: &mut App,
    output: VideoPlayerOutput,
    sender: &ComponentSender<App>,
    root: &adw::ApplicationWindow,
) {
    match output {
        VideoPlayerOutput::FileLoaded {
            path: _,
            duration_secs,
        } => {
            app.screensaver.inhibit(root);
            if let Some(ref item) = app.now_playing {
                let meta = mpris::metadata_from_media_item(item, duration_secs, None);
                let _ = app.mpris.metadata_tx.send(meta);
                let rating_key = if item.source_type == SourceType::Plex {
                    Some(item.external_id.as_str())
                } else {
                    None
                };
                app.watch_tracker
                    .start(&item.id, rating_key, duration_secs, Instant::now());
            }
        }
        VideoPlayerOutput::PositionChanged {
            position_secs,
            duration_secs,
        } => {
            app.last_position = position_secs;
            let _ = app
                .mpris
                .position_tx
                .send(mpris::seconds_to_micros(position_secs));
            if let Some(ref item) = app.now_playing
                && !app.watch_tracker.is_active()
            {
                let rating_key = if item.source_type == SourceType::Plex {
                    Some(item.external_id.as_str())
                } else {
                    None
                };
                app.watch_tracker
                    .start(&item.id, rating_key, duration_secs, Instant::now());
            }
            let events = app
                .watch_tracker
                .process_position(position_secs, Instant::now());
            dispatch_watch_events(
                &app.db,
                events,
                &app.active_source,
                app.now_playing.as_ref().map(|i| i.id.as_str()),
                sender,
            );
        }
        VideoPlayerOutput::StateChanged(state) => {
            let _ = app.mpris.status_tx.send(state);
            if app.current_view == CurrentView::Player {
                root.set_title(Some(crate::player::window_title_for_state(state)));
            }
            match state {
                PlayState::Playing => {
                    app.screensaver.inhibit(root);
                    let events = app.watch_tracker.process_state_change(
                        PlaybackState::Playing,
                        app.last_position,
                        Instant::now(),
                    );
                    dispatch_watch_events(
                        &app.db,
                        events,
                        &app.active_source,
                        app.now_playing.as_ref().map(|i| i.id.as_str()),
                        sender,
                    );
                }
                PlayState::Paused | PlayState::Stopped => {
                    app.screensaver.uninhibit(root);
                    let events = app.watch_tracker.process_state_change(
                        PlaybackState::Paused,
                        app.last_position,
                        Instant::now(),
                    );
                    dispatch_watch_events(
                        &app.db,
                        events,
                        &app.active_source,
                        app.now_playing.as_ref().map(|i| i.id.as_str()),
                        sender,
                    );
                }
            }
        }
        VideoPlayerOutput::EndOfFile => {
            app.screensaver.uninhibit(root);
            let _ = app.mpris.status_tx.send(PlayState::Stopped);
            let _ = app.mpris.metadata_tx.send(mpris::MprisMetadata::default());
            let _ = app.mpris.position_tx.send(0);
            let events = app.watch_tracker.stop(app.last_position);
            dispatch_watch_events(
                &app.db,
                events,
                &app.active_source,
                app.now_playing.as_ref().map(|i| i.id.as_str()),
                sender,
            );
            app.now_playing = None;
            let watch_data = load_watch_data(&app.db);
            app.library_view
                .emit(LibraryViewMsg::SetWatchData(watch_data));
            if app.current_view == CurrentView::Player {
                leave_player_mode(root, &mut app.player_chrome_revealer);
                app.stack.set_visible_child_name("shell");
                root.set_fullscreened(false);
            }
        }
        VideoPlayerOutput::VolumeChanged { volume, muted: _ } => {
            app.settings.playback.default_volume = volume;
            let _ = app.settings.save();
        }
        VideoPlayerOutput::SpeedChanged(_) => {}
        VideoPlayerOutput::ToggleFullscreen => {}
        VideoPlayerOutput::Error(msg) => {
            sender.input(AppMsg::ShowToast(msg));
        }
        VideoPlayerOutput::LoadSubtitleFile => {
            super::dialogs::show_subtitle_chooser(root, sender.input_sender().clone());
        }
        VideoPlayerOutput::Leave => {
            sender.input(AppMsg::GoBack);
        }
        VideoPlayerOutput::ControlsRevealedChanged(revealed) => {
            app.player_chrome_revealer.set_reveal_child(revealed);
        }
    }
}

/// Decide the local file path to play, if a complete on-disk copy exists.
///
/// Returns `Some(path)` only for a `Completed` download whose file is present on
/// disk; an incomplete/`Paused`/`Failed` download, a missing file, or no
/// download row all return `None`, so Play falls back to streaming (R13). Pure
/// so the redirect decision is testable without GTK, the DB, or a real file
/// outside the existence flag the caller supplies.
pub fn local_playback_path(
    state: Option<DownloadState>,
    file_path: Option<&str>,
    file_exists: bool,
) -> Option<String> {
    match (state, file_path) {
        (Some(DownloadState::Completed), Some(path)) if file_exists => Some(path.to_string()),
        _ => None,
    }
}

/// Resolve the effective playback URL for an item: a `file://` URL to the local
/// downloaded copy when one is complete and present, else the streamed source
/// URL unchanged. The single Play choke point every path converges on (R12).
fn local_redirect(app: &App, item: Option<&MediaItem>, stream_url: &str) -> String {
    if let Some(item) = item
        && let Some(db) = app.db.as_ref()
        && let Ok(Some(d)) = db.with(|conn| DownloadsRepo::new(conn).find(&item.id))
    {
        let exists = d
            .file_path
            .as_deref()
            .map(|p| Path::new(p).exists())
            .unwrap_or(false);
        if let Some(local) = local_playback_path(Some(d.state), d.file_path.as_deref(), exists) {
            info!("Playing local downloaded copy for {}", item.id);
            return format!("file://{local}");
        }
    }
    stream_url.to_string()
}

/// Handle PlayMedia: set up the player for a new media URL.
pub fn handle_play_media(
    app: &mut App,
    url: String,
    media_item: Option<MediaItem>,
    sender: &ComponentSender<App>,
    root: &adw::ApplicationWindow,
) {
    info!("Playing media: {}...", &url[..url.len().min(80)]);
    // Resume where playback left off. For a downloaded item watched offline, an
    // unsynced offline position wins over the source's (stale) offset — the one
    // sanctioned inversion of "Plex is authoritative". Otherwise the source
    // offset wins, then locally tracked progress. A source-`watched` item drops
    // the offline mid-progress (latest-state-wins).
    app.pending_resume = None;
    if let Some(ref item) = media_item {
        let source_offset = item.resume_position_secs();
        let (offline_pending, local_tracked) = match app.db.as_ref() {
            Some(db) => db.with(|conn| {
                let offline = DownloadsRepo::new(conn)
                    .latest_pending_sync_for(&item.id)
                    .ok()
                    .flatten()
                    .map(|p| p.position_ms as f64 / 1000.0);
                let local = WatchProgressRepo::new(conn)
                    .find_by_media_id(&item.id)
                    .ok()
                    .flatten()
                    .filter(|progress| progress.should_show_resume())
                    .map(|progress| progress.resume_position());
                (offline, local)
            }),
            None => (None, None),
        };
        app.pending_resume =
            resume_position(source_offset, offline_pending, local_tracked, item.watched);
    }
    // Redirect to a complete local copy when one exists; otherwise stream from
    // the source (R12/R13). This single choke point covers every Play path.
    let play_url = local_redirect(app, media_item.as_ref(), &url);
    app.now_playing = media_item.clone();
    app.last_position = 0.0;
    app.current_view = CurrentView::Player;
    let title = player_title_for_item(media_item.as_ref(), &url);
    enter_player_mode(
        root,
        &mut app.player_chrome_revealer,
        &app.player_window_title,
        &title,
    );
    app.video_player.emit(VideoPlayerMsg::SetTitle(Some(title)));
    app.stack.set_visible_child_name("player");
    app.video_player.emit(VideoPlayerMsg::SetAutoplay(true));
    app.video_player.emit(VideoPlayerMsg::SetUrl {
        url: Some(play_url),
        resume_secs: app.pending_resume.take(),
    });

    // Fetch skip-intro / skip-credits markers from Plex.
    if let Some(ref item) = media_item
        && item.source_type == SourceType::Plex
        && let Some(source) = app.active_source.clone()
    {
        let rating_key = item.external_id.clone();
        let duration_secs = item.runtime_minutes.map(|m| m as f64 * 60.0).unwrap_or(0.0);
        sender.oneshot_command(async move {
            match source.skip_markers(&rating_key, duration_secs).await {
                Ok(markers) => AppCmd::SkipMarkersLoaded(markers),
                Err(e) => {
                    debug!("Skip markers not available for {rating_key}: {e}");
                    AppCmd::Noop
                }
            }
        });
    }
}

/// Handle ConnectionSaved: persist the source and wire it to all views.
pub fn handle_connection_saved(
    app: &mut App,
    url: String,
    token: String,
    name: String,
    sender: &ComponentSender<App>,
) {
    info!("Plex connection saved: {} ({})", name, url);
    app.connection_dialog = None;

    // Save to database
    if let Some(db) = &app.db {
        let source = Source {
            id: Source::make_id(&url),
            source_type: SourceType::Plex,
            name: name.clone(),
            config: SourceConfig {
                url: url.clone(),
                token: token.clone(),
            },
            enabled: true,
            last_synced_at: None,
        };

        db.with(|conn| {
            let mut repo = crate::db::source_repo::SourceRepo::new(conn);
            let _ = repo.delete(&source.id);
            if let Err(e) = repo.insert(&source) {
                tracing::warn!("Failed to save source: {e}");
            }
        });
    }

    let client = PlexClient::new(&url, &token);
    let source = Arc::new(PlexSource::new(client, name.clone()));
    let artwork_cache = Arc::new(ArtworkCache::new(config::artwork_dir()));

    app.active_source = Some(source.clone());
    app.source_url = Some(url.clone());

    // Feed the sidebar tree: source identity, current visibility, and (async)
    // the source's libraries.
    app.sidebar.emit(SidebarMsg::SetSource {
        name: name.clone(),
        source_type: "plex".to_string(),
        source_id: url.clone(),
    });
    app.sidebar.emit(SidebarMsg::SetVisibility(
        app.settings.library_visibility.hidden.clone(),
    ));
    {
        // The PlexClient absorbs cold-start connection retries, so a single
        // fetch here is enough once the connection is established.
        let src = source.clone();
        sender.oneshot_command(async move {
            AppCmd::LibrariesLoaded(src.libraries().await.unwrap_or_default())
        });
    }

    app.home_view.emit(HomeViewMsg::SetSource(
        source.clone(),
        artwork_cache.clone(),
    ));
    app.library_view.emit(LibraryViewMsg::SetSource(
        source.clone(),
        artwork_cache.clone(),
    ));
    app.library_view.emit(LibraryViewMsg::SetSavedUiState(
        app.settings.library.clone(),
    ));
    app.movie_detail.emit(
        crate::components::detail::movie_detail::MovieDetailMsg::SetSource(
            source.clone(),
            artwork_cache.clone(),
        ),
    );
    app.downloads_view
        .emit(crate::components::downloads::DownloadsViewMsg::SetSource(
            source.clone(),
            artwork_cache.clone(),
        ));
    app.show_detail.emit(
        crate::components::detail::show_detail::ShowDetailMsg::SetSource(source, artwork_cache),
    );

    // Send watch data to library view
    let watch_data = load_watch_data(&app.db);
    app.library_view
        .emit(LibraryViewMsg::SetWatchData(watch_data));
    // A library loads when picked from the sidebar; the default view is Home.

    sender.input(AppMsg::ShowToast(format!("Connected to {name}")));

    // Flush any offline-recorded progress back to the source on (re)connect.
    super::watch_events::flush_pending_sync(app, sender);

    // The source is live — start any queued/recovered downloads.
    super::download_handlers::start_pending(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_with_existing_file_plays_local() {
        // Covers the centralized redirect decision: complete copy present.
        assert_eq!(
            local_playback_path(Some(DownloadState::Completed), Some("/dl/m1.mkv"), true),
            Some("/dl/m1.mkv".to_string())
        );
    }

    #[test]
    fn completed_but_file_missing_streams() {
        // Covers AE3: local file deleted externally -> fall back to streaming.
        assert_eq!(
            local_playback_path(Some(DownloadState::Completed), Some("/dl/m1.mkv"), false),
            None
        );
    }

    #[test]
    fn incomplete_or_paused_streams() {
        for state in [
            DownloadState::Queued,
            DownloadState::Downloading,
            DownloadState::Paused,
            DownloadState::Failed,
        ] {
            assert_eq!(
                local_playback_path(Some(state), Some("/dl/m1.mkv"), true),
                None,
                "state {state:?} must stream, not play a partial file"
            );
        }
    }

    #[test]
    fn no_download_row_streams() {
        assert_eq!(local_playback_path(None, None, false), None);
    }
}
