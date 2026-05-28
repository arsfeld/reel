use std::time::Instant;

use adw::prelude::*;

use relm4::prelude::*;
use tracing::{debug, info};

use crate::components::library::LibraryViewMsg;
use crate::components::player::video_player::{VideoPlayerMsg, VideoPlayerOutput};
use crate::db::watch_progress_repo::WatchProgressRepo;
use crate::models::media::{MediaItem, SourceType};
use crate::models::source::{Source, SourceConfig};
use crate::navigation::CurrentView;
use crate::player::PlayState;
use crate::services::mpris;
use crate::services::watch_state::PlaybackState;

use super::App;
use super::AppCmd;
use super::AppMsg;
use super::db_helpers::load_watch_data;
use super::player_ui::{enter_player_mode, leave_player_mode, player_title_for_item};
use super::watch_events::dispatch_watch_events;

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
                let rating_key = if item.source_type.reports_watch_state() {
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
                let rating_key = if item.source_type.reports_watch_state() {
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
            {
                let src = app.now_playing_source();
                dispatch_watch_events(&app.db_conn, events, &src, sender);
            }
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
                    {
                        let src = app.now_playing_source();
                        dispatch_watch_events(&app.db_conn, events, &src, sender);
                    }
                }
                PlayState::Paused | PlayState::Stopped => {
                    app.screensaver.uninhibit(root);
                    let events = app.watch_tracker.process_state_change(
                        PlaybackState::Paused,
                        app.last_position,
                        Instant::now(),
                    );
                    {
                        let src = app.now_playing_source();
                        dispatch_watch_events(&app.db_conn, events, &src, sender);
                    }
                }
            }
        }
        VideoPlayerOutput::EndOfFile => {
            app.screensaver.uninhibit(root);
            let _ = app.mpris.status_tx.send(PlayState::Stopped);
            let _ = app.mpris.metadata_tx.send(mpris::MprisMetadata::default());
            let _ = app.mpris.position_tx.send(0);
            let events = app.watch_tracker.stop(app.last_position);
            {
                let src = app.now_playing_source();
                dispatch_watch_events(&app.db_conn, events, &src, sender);
            }
            app.now_playing = None;
            let watch_data = load_watch_data(&app.db_conn);
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

/// Handle PlayMedia: set up the player for a new media URL.
pub fn handle_play_media(
    app: &mut App,
    url: String,
    media_item: Option<MediaItem>,
    sender: &ComponentSender<App>,
    root: &adw::ApplicationWindow,
) {
    info!("Playing media: {}...", &url[..url.len().min(80)]);
    // Resume where playback left off, preferring the source's own offset (e.g.
    // Plex view offset / Jellyfin resume) so it stays in sync across devices;
    // fall back to locally tracked progress only when the source has no opinion.
    // A server-watched item never resumes from a stale local offset (AE6).
    app.pending_resume = None;
    if let Some(ref item) = media_item {
        let local = app.db_conn.as_ref().and_then(|conn| {
            WatchProgressRepo::new(conn)
                .find_by_media_id(&item.id)
                .ok()
                .flatten()
        });
        app.pending_resume = super::utils::resume_position_for(item, local.as_ref());
    }
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
        url: Some(url),
        resume_secs: app.pending_resume.take(),
    });

    // Fetch skip-intro / skip-credits markers from the owning server.
    // skip_markers degrades to NotSupported for sources without them.
    if let Some(ref item) = media_item
        && item.source_type.reports_watch_state()
        && let Some(source) = app.sources.for_item(item)
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
    source_type: SourceType,
    user_id: Option<String>,
    sender: &ComponentSender<App>,
) {
    info!(
        "Connection saved: {} ({}) [{}]",
        name,
        url,
        source_type.as_str()
    );
    app.connection_dialog = None;

    let source = Source {
        id: Source::make_id(source_type, &url),
        source_type,
        name: name.clone(),
        config: SourceConfig {
            url: url.clone(),
            token,
            user_id,
        },
        enabled: true,
        last_synced_at: None,
    };

    // Persist (id-scoped upsert: delete-then-insert the *same* id, never others).
    if let Some(ref conn) = app.db_conn {
        let repo = crate::db::source_repo::SourceRepo::new(conn);
        let _ = repo.delete(&source.id);
        if let Err(e) = repo.insert(&source) {
            tracing::warn!("Failed to save source: {e}");
        }
    }

    // Build the source via the factory and wire it into every view.
    if let Some(built) = super::source_factory::build_source(&source) {
        app.wire_active_source(source_type, url, built, sender);
    }
    // A library loads when picked from the sidebar; the default view is Home.

    sender.input(AppMsg::ShowToast(format!("Connected to {name}")));
}
