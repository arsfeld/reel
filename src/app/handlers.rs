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
use crate::db::watch_progress_repo::WatchProgressRepo;
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
            dispatch_watch_events(&app.db_conn, events, &app.active_source, sender);
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
                    dispatch_watch_events(&app.db_conn, events, &app.active_source, sender);
                }
                PlayState::Paused | PlayState::Stopped => {
                    app.screensaver.uninhibit(root);
                    let events = app.watch_tracker.process_state_change(
                        PlaybackState::Paused,
                        app.last_position,
                        Instant::now(),
                    );
                    dispatch_watch_events(&app.db_conn, events, &app.active_source, sender);
                }
            }
        }
        VideoPlayerOutput::EndOfFile => {
            app.screensaver.uninhibit(root);
            let _ = app.mpris.status_tx.send(PlayState::Stopped);
            let _ = app.mpris.metadata_tx.send(mpris::MprisMetadata::default());
            let _ = app.mpris.position_tx.send(0);
            let events = app.watch_tracker.stop(app.last_position);
            dispatch_watch_events(&app.db_conn, events, &app.active_source, sender);
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
    // Plex view offset) so it stays in sync across devices; fall back to locally
    // tracked progress only when the source reports none.
    app.pending_resume = None;
    if let Some(ref item) = media_item {
        app.pending_resume = item.resume_position_secs().or_else(|| {
            let conn = app.db_conn.as_ref()?;
            let repo = WatchProgressRepo::new(conn);
            repo.find_by_media_id(&item.id)
                .ok()
                .flatten()
                .filter(|progress| progress.should_show_resume())
                .map(|progress| progress.resume_position())
        });
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
    if let Some(ref conn) = app.db_conn {
        let source = Source {
            id: Source::make_id(SourceType::Plex, &url),
            source_type: SourceType::Plex,
            name: name.clone(),
            config: SourceConfig {
                url: url.clone(),
                token: token.clone(),
                user_id: None,
            },
            enabled: true,
            last_synced_at: None,
        };

        let repo = crate::db::source_repo::SourceRepo::new(conn);
        let _ = repo.delete(&source.id);
        if let Err(e) = repo.insert(&source) {
            tracing::warn!("Failed to save source: {e}");
        }
    }

    let client = PlexClient::new(&url, &token);
    let source = Arc::new(PlexSource::new(client, name.clone()));
    let artwork_cache = Arc::new(ArtworkCache::new(config::artwork_dir()));

    app.active_source = Some(source.clone() as Arc<dyn MediaSource>);
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
    app.show_detail.emit(
        crate::components::detail::show_detail::ShowDetailMsg::SetSource(source, artwork_cache),
    );

    // Send watch data to library view
    let watch_data = load_watch_data(&app.db_conn);
    app.library_view
        .emit(LibraryViewMsg::SetWatchData(watch_data));
    // A library loads when picked from the sidebar; the default view is Home.

    sender.input(AppMsg::ShowToast(format!("Connected to {name}")));
}
