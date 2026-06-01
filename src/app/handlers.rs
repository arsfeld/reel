use std::path::Path;
use std::time::Instant;

use adw::prelude::*;

use relm4::prelude::*;
use tracing::{debug, info, warn};

use crate::components::downloads::DownloadsViewMsg;
use crate::components::home::HomeViewMsg;
use crate::components::library::LibraryViewMsg;
use crate::components::player::video_player::{VideoPlayerMsg, VideoPlayerOutput};
use crate::db::downloads_repo::DownloadsRepo;
use crate::db::watch_progress_repo::WatchProgressRepo;
use crate::models::download::DownloadState;
use crate::models::library::LibrarySection;
use crate::models::media::{MediaItem, MediaType, SourceType};
use crate::models::source::{Source, SourceConfig};
use crate::models::watch::WatchProgress;
use crate::navigation::CurrentView;
use crate::player::PlayState;
use crate::services::mpris;
use crate::services::session_cache::{CachedHome, SessionContentCache};
use crate::services::watch_state::PlaybackState;

use super::App;
use super::AppCmd;
use super::AppMsg;
use super::db_helpers::{load_in_progress, load_watch_data};
use super::player_ui::{enter_player_mode, leave_player_mode, player_title_for_item};
use super::utils::iso_now;
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
                dispatch_watch_events(
                    &app.db,
                    events,
                    &src,
                    app.now_playing.as_ref().map(|i| i.id.as_str()),
                    sender,
                );
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
                        dispatch_watch_events(
                            &app.db,
                            events,
                            &src,
                            app.now_playing.as_ref().map(|i| i.id.as_str()),
                            sender,
                        );
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
                        dispatch_watch_events(
                            &app.db,
                            events,
                            &src,
                            app.now_playing.as_ref().map(|i| i.id.as_str()),
                            sender,
                        );
                    }
                }
            }
        }
        VideoPlayerOutput::EndOfFile => {
            // Stop the transcode session + keepalive now that the file ended (R14).
            stop_active_session(app, sender);
            app.screensaver.uninhibit(root);
            let _ = app.mpris.status_tx.send(PlayState::Stopped);
            let _ = app.mpris.metadata_tx.send(mpris::MprisMetadata::default());
            let _ = app.mpris.position_tx.send(0);
            let events = app.watch_tracker.stop(app.last_position);
            {
                let src = app.now_playing_source();
                dispatch_watch_events(
                    &app.db,
                    events,
                    &src,
                    app.now_playing.as_ref().map(|i| i.id.as_str()),
                    sender,
                );
            }
            let played_id = app.now_playing.as_ref().map(|i| i.id.clone());
            app.now_playing = None;
            let watch_data = load_watch_data(&app.db);
            // R8: reflect the just-saved progress in the session cache so a revisit
            // to this item's library/Home shows it immediately, before any
            // background revalidation.
            if let Some(id) = played_id {
                let watched = watch_data.get(&id).map(|&(_, w)| w).unwrap_or(false);
                let position_ms = if watched {
                    None
                } else {
                    Some((app.last_position * 1000.0).max(0.0) as i64)
                };
                app.note_local_watch_mutation(&id, watched, position_ms);
            }
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
        VideoPlayerOutput::SelectQuality {
            selection,
            position_secs,
        } => {
            // R10/KTD11: a manual capped rung forces a transcode (escape hatch);
            // Auto and Original let the server decide.
            let force = matches!(
                selection,
                crate::models::playback::QualitySelection::Manual(p)
                    if p != crate::models::playback::QualityPreset::Original
            );
            app.current_quality = selection;
            resolve_playback_at(app, selection, position_secs, force, sender);
        }
        VideoPlayerOutput::SeekReload { position_secs } => {
            // Seek during a transcode: re-resolve at the new offset, same quality.
            let quality = app.current_quality;
            resolve_playback_at(app, quality, position_secs, false, sender);
        }
        VideoPlayerOutput::RenderFailed { position_secs } => {
            // Only direct-play failures fall back to transcode (U4/R4). If we're
            // already transcoding (or direct-streaming) and the stream errors,
            // there's nothing better to switch to — re-resolving would just loop
            // on the same broken transcode. Leave the pipeline's error to surface
            // on the status plate (the pre-feature behavior for transcode errors).
            let was_direct_play = matches!(
                app.current_decision.as_ref().map(|d| d.kind),
                Some(crate::models::playback::PlaybackDecisionKind::DirectPlay)
            );
            if !was_direct_play {
                return;
            }
            // Direct-play couldn't render (U4/R4): fall back to a server
            // transcode for this item, bounded so a failing transcode can't loop.
            use crate::components::player::switch_state::FallbackAction;
            match app.render_fallback.on_render_failure() {
                FallbackAction::RetryWithTranscode => {
                    sender.input(AppMsg::ShowToast(
                        "Switching to a compatible format…".to_string(),
                    ));
                    let quality = app.current_quality;
                    // force_transcode=true here; resolve_playback_at also ORs in
                    // the now-sticky fallback state for subsequent re-resolves.
                    resolve_playback_at(app, quality, position_secs, true, sender);
                }
                FallbackAction::GiveUp => {
                    sender.input(AppMsg::ShowToast(
                        "Can't play this video on this device.".to_string(),
                    ));
                }
            }
        }
        VideoPlayerOutput::SelectAudioTrack {
            stream_id,
            position_secs,
        } => {
            // AE6: track change during a transcode → re-decide with the chosen
            // Plex audioStreamID + reload-at-position, preserving quality.
            app.current_audio_stream_id = Some(stream_id);
            let (quality, force) = quality_and_force(app);
            resolve_playback_at(app, quality, position_secs, force, sender);
        }
        VideoPlayerOutput::SelectSubtitleTrack {
            stream_id,
            position_secs,
        } => {
            app.current_subtitle_stream_id = stream_id;
            let (quality, force) = quality_and_force(app);
            resolve_playback_at(app, quality, position_secs, force, sender);
        }
    }
}

/// The current quality selection plus whether it forces a transcode (R10/KTD11):
/// a manual capped rung forces; Auto and Original let the server decide.
fn quality_and_force(app: &App) -> (crate::models::playback::QualitySelection, bool) {
    let quality = app.current_quality;
    let force = matches!(
        quality,
        crate::models::playback::QualitySelection::Manual(p)
            if p != crate::models::playback::QualityPreset::Original
    );
    (quality, force)
}

/// Re-resolve the current title at a new quality/offset (U8 switch + seek-reload,
/// U10 track change). Tags the resolve with a fresh switch epoch so a superseded
/// switch's result is discarded.
/// Whether a source backend resolves playback through a server-side decision
/// (quality ladder, transcode, decision indicator). Plex and Jellyfin both do;
/// `Local` plays the file directly. Pure so the gate is testable without GTK.
pub fn supports_server_decision(source_type: SourceType) -> bool {
    matches!(source_type, SourceType::Plex | SourceType::Jellyfin)
}

fn resolve_playback_at(
    app: &mut App,
    quality: crate::models::playback::QualitySelection,
    offset_secs: f64,
    force_transcode: bool,
    sender: &ComponentSender<App>,
) {
    let (Some(item), Some(source)) = (app.now_playing.clone(), app.now_playing_source()) else {
        return;
    };
    if !supports_server_decision(item.source_type) || item.file_path.is_none() {
        return;
    }
    // Once an item has fallen back to transcode after a render failure (U4),
    // every re-resolve for it stays on transcode so a quality/track change
    // doesn't retry direct-play and re-trigger the black screen.
    let force_transcode = force_transcode || app.render_fallback.force_transcode();
    let caps = app.playback_capabilities;
    let can_direct_play_10bit = crate::player::capabilities::can_direct_play(
        caps.gst_can_render_10bit || caps.mpv_available,
        quality,
        force_transcode,
    );
    let can_direct_play_hdr = crate::player::capabilities::can_direct_play(
        caps.active_backend == crate::models::playback::PlaybackBackendKind::Mpv,
        quality,
        force_transcode,
    );
    let req = crate::models::playback::PlaybackRequest {
        rating_key: item.external_id.clone(),
        part_key: item.file_path.clone().unwrap_or_default(),
        media_index: 0,
        part_index: 0,
        quality,
        force_transcode,
        can_direct_play_10bit,
        can_direct_play_hdr,
        backend_kind: caps.active_backend,
        // Carry the chosen tracks (AE6) so a quality switch preserves them.
        audio_stream_id: app.current_audio_stream_id,
        subtitle_stream_id: app.current_subtitle_stream_id,
        offset_secs,
    };
    let fallback_url = source.playback_url(&req.part_key);
    let epoch = app.switch_state.begin();
    sender.oneshot_command(async move {
        match source.resolve_playback(&req).await {
            Ok(decision) => AppCmd::PlaybackResolved {
                decision: Box::new(decision),
                resume_secs: Some(offset_secs),
                epoch,
            },
            Err(e) => AppCmd::PlaybackResolveFailed {
                message: format!("Couldn't switch quality: {e}"),
                fallback_url,
                resume_secs: Some(offset_secs),
                epoch,
            },
        }
    });
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

/// Whether a complete local downloaded copy exists for the item (R12). When it
/// does, Play uses it directly (no transcode decision needed).
fn local_redirect(app: &App, item: Option<&MediaItem>) -> Option<String> {
    let item = item?;
    let db = app.db.as_ref()?;
    let d = db
        .with(|conn| DownloadsRepo::new(conn).find(&item.id))
        .ok()
        .flatten()?;
    let exists = d
        .file_path
        .as_deref()
        .map(|p| Path::new(p).exists())
        .unwrap_or(false);
    local_playback_path(Some(d.state), d.file_path.as_deref(), exists).map(|local| {
        info!("Playing local downloaded copy for {}", item.id);
        format!("file://{local}")
    })
}

/// Handle PlayMedia: set up the player for a new media URL.
pub fn handle_play_media(
    app: &mut App,
    url: String,
    media_item: Option<MediaItem>,
    sender: &ComponentSender<App>,
    root: &adw::ApplicationWindow,
) {
    // Redact before slicing — a short host can place the token value within the
    // first 80 chars, so slicing the raw URL would leak it (U6/KTD8).
    let log_url = crate::models::playback::redact_plex_token(&url);
    info!("Playing media: {}...", &log_url[..log_url.len().min(80)]);
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
                // Persist the item being played so the watch_progress /
                // pending_sync foreign keys (-> media_items.id) resolve. Items
                // browsed on-demand from a source are otherwise only in memory,
                // never having gone through a library sync, so a later progress
                // write would hit a FOREIGN KEY constraint failure.
                if let Err(e) = crate::db::media_repo::MediaRepo::new(conn).upsert(item) {
                    warn!("Failed to persist media item before playback: {e}");
                }
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
    // Prefer a complete local downloaded copy when one exists; otherwise the
    // source decides (transcode/direct stream). This is the single Play choke
    // point (R12/R13).
    let local_url = local_redirect(app, media_item.as_ref());
    // R11: a quality override applies only to the current title. Stop any prior
    // transcode session + keepalive BEFORE switching `now_playing`, so the stop
    // resolves the *outgoing* item's source — switching titles never orphans a
    // session (U10/R14).
    stop_active_session(app, sender);
    app.now_playing = media_item.clone();
    app.last_position = 0.0;
    app.current_view = CurrentView::Player;
    // Reset the remaining session-only state on every new title.
    app.current_quality = crate::models::playback::QualitySelection::Auto;
    app.current_audio_stream_id = None;
    app.current_subtitle_stream_id = None;
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
    let resume_secs = app.pending_resume.take();
    begin_initial_playback(
        app,
        media_item.as_ref(),
        url,
        local_url,
        resume_secs,
        sender,
    );

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

/// Hand the initial stream to the player: a complete local download plays
/// directly; a streamed Plex item routes through the async transcode decision
/// (R1); any other source uses the direct URL the component supplied.
fn begin_initial_playback(
    app: &mut App,
    media_item: Option<&MediaItem>,
    url: String,
    local_url: Option<String>,
    resume_secs: Option<f64>,
    sender: &ComponentSender<App>,
) {
    // Reset/establish render-failure fallback state for the item being played
    // (U4). Switching items clears prior stickiness; replaying the same item
    // keeps it (it already failed to render).
    if let Some(item) = media_item {
        app.render_fallback.begin_item(&item.id);
    }

    // A complete local download plays directly — no transcode decision needed.
    if let Some(local_url) = local_url {
        app.video_player.emit(VideoPlayerMsg::SetUrl {
            url: Some(local_url),
            resume_secs,
            base_offset_secs: 0.0,
            is_transcode: false,
        });
        app.video_player.emit(VideoPlayerMsg::SetDecisionInfo {
            available: false,
            selection: crate::models::playback::QualitySelection::Auto,
            indicator: String::new(),
            audio_streams: Vec::new(),
            subtitle_streams: Vec::new(),
        });
        return;
    }

    // For a streamed Plex/Jellyfin item, route through the server-side playback
    // decision instead of the eager direct-play URL. The decision is async, so
    // SetUrl is deferred to PlaybackResolved. Local sources keep the direct URL.
    let decision_item =
        media_item.filter(|i| supports_server_decision(i.source_type) && i.file_path.is_some());
    let (Some(item), Some(source)) = (decision_item, app.now_playing_source()) else {
        app.video_player.emit(VideoPlayerMsg::SetUrl {
            url: Some(url),
            resume_secs,
            base_offset_secs: 0.0,
            is_transcode: false,
        });
        app.video_player.emit(VideoPlayerMsg::SetDecisionInfo {
            available: false,
            selection: crate::models::playback::QualitySelection::Auto,
            indicator: String::new(),
            audio_streams: Vec::new(),
            subtitle_streams: Vec::new(),
        });
        return;
    };

    let caps = app.playback_capabilities;
    let can_direct_play_10bit = crate::player::capabilities::can_direct_play(
        caps.gst_can_render_10bit || caps.mpv_available,
        crate::models::playback::QualitySelection::Auto,
        false,
    );
    let can_direct_play_hdr = crate::player::capabilities::can_direct_play(
        caps.active_backend == crate::models::playback::PlaybackBackendKind::Mpv,
        crate::models::playback::QualitySelection::Auto,
        false,
    );
    let req = crate::models::playback::PlaybackRequest {
        rating_key: item.external_id.clone(),
        part_key: item.file_path.clone().unwrap_or_default(),
        media_index: 0,
        part_index: 0,
        quality: crate::models::playback::QualitySelection::Auto,
        force_transcode: false,
        can_direct_play_10bit,
        can_direct_play_hdr,
        backend_kind: caps.active_backend,
        audio_stream_id: None,
        subtitle_stream_id: None,
        offset_secs: resume_secs.unwrap_or(0.0),
    };
    // `url` is kept as the last-resort direct-play fallback if the decision
    // cannot be reached (surfaced, not silent — U7).
    let fallback_url = url;
    // Make the quality menu available immediately (Auto, no indicator yet) so
    // the user can override even before the first decision resolves (U9).
    app.video_player.emit(VideoPlayerMsg::SetDecisionInfo {
        available: true,
        selection: crate::models::playback::QualitySelection::Auto,
        indicator: String::new(),
        audio_streams: Vec::new(),
        subtitle_streams: Vec::new(),
    });
    // Tag with a fresh switch epoch so a later switch can supersede the initial
    // play if the user picks quality immediately (U8).
    let epoch = app.switch_state.begin();
    sender.oneshot_command(async move {
        match source.resolve_playback(&req).await {
            Ok(decision) => AppCmd::PlaybackResolved {
                decision: Box::new(decision),
                resume_secs,
                epoch,
            },
            Err(e) => AppCmd::PlaybackResolveFailed {
                message: format!("Couldn't reach the server's transcoder: {e}"),
                fallback_url,
                resume_secs,
                epoch,
            },
        }
    });
}

/// Tear down the active transcode session and its keepalive (U10/R14): stop the
/// keepalive timer, `/stop` the session on the server, and clear the session-only
/// decision state. Called on EOF, navigate-away/`Leave`, and quality-fallback.
/// A no-op when no transcode is active (direct-play / local).
pub(super) fn stop_active_session(app: &mut App, sender: &ComponentSender<App>) {
    super::transcode_keepalive::stop(app);
    app.current_decision = None;
    app.transcode_base_offset = 0.0;
    if let Some(session) = app.active_transcode_session.take() {
        stop_session_async(app, session, sender);
    }
}

/// Stop a transcode session in the background (fire-and-forget, bounded retry in
/// the client). Resolves the now-playing item's source; Plex and Jellyfin
/// override stop_transcode, while sources without a server transcoder inherit
/// the no-op default. Also a no-op when no source is resolvable.
fn stop_session_async(app: &App, session: String, sender: &ComponentSender<App>) {
    if let Some(source) = app.now_playing_source() {
        sender.oneshot_command(async move {
            if let Err(e) = source.stop_transcode(&session).await {
                debug!("stop_transcode({session}) failed: {e}");
            }
            AppCmd::Noop
        });
    }
}

/// Handle a resolved playback decision (U7/U8): record the session-only decision
/// state and hand the URL to the player with the decision-kind-specific resume
/// policy (KTD1 — transcode resumes at 0 with a base offset; direct-play seeks).
///
/// A result from a superseded switch (stale epoch) is discarded and its session
/// stopped, so two rapid switches leave exactly one live stream (U8). On a fresh
/// apply the *previous* session is stopped only after the new one resolved, so a
/// failed re-decision never tears down a still-playable stream.
pub fn handle_playback_resolved(
    app: &mut App,
    decision: Box<crate::models::playback::PlaybackDecision>,
    resume_secs: Option<f64>,
    epoch: u64,
    sender: &ComponentSender<App>,
) {
    use crate::components::player::switch_state::SwitchOutcome;
    if app.switch_state.evaluate(epoch) == SwitchOutcome::DiscardStale {
        debug!("Discarding stale playback decision (epoch {epoch})");
        if let Some(session) = decision.session.clone() {
            stop_session_async(app, session, sender);
        }
        return;
    }

    let previous_session = app.active_transcode_session.take();
    let (url, resume_out, base_offset) = super::utils::set_url_for_decision(&decision, resume_secs);
    let is_transcode = decision.kind.is_transcode_like();
    let indicator = decision.indicator_text();
    info!(
        "Playback resolved: {:?} (resume={:?}, base_offset={base_offset})",
        decision.kind, resume_out
    );
    app.active_transcode_session = decision.session.clone();
    app.transcode_base_offset = base_offset;
    // Sync the selected track ids from what the server actually chose (AE6) so a
    // later quality switch carries them; only when the decision lists tracks
    // (transcode) — direct-play keeps live GStreamer selection.
    let audio_streams = decision.audio_streams.clone();
    let subtitle_streams = decision.subtitle_streams.clone();
    if let Some(sel) = audio_streams.iter().find(|s| s.selected) {
        app.current_audio_stream_id = Some(sel.id);
    }
    app.current_subtitle_stream_id = subtitle_streams.iter().find(|s| s.selected).map(|s| s.id);
    app.current_decision = Some(*decision);
    app.video_player.emit(VideoPlayerMsg::SetUrl {
        url: Some(url),
        resume_secs: resume_out,
        base_offset_secs: base_offset,
        is_transcode,
    });
    // Update the quality menu indicator (R16) + transcode-aware track menus (AE6)
    // from the server-actual decision.
    app.video_player.emit(VideoPlayerMsg::SetDecisionInfo {
        available: true,
        selection: app.current_quality,
        indicator,
        audio_streams,
        subtitle_streams,
    });
    // Restart the keepalive timer for the newly active session (U10/R15).
    super::transcode_keepalive::restart(app, sender);

    // Stop the prior session now that the new stream is loading (R14, U8).
    if let Some(prev) = previous_session
        && Some(&prev) != app.active_transcode_session.as_ref()
    {
        stop_session_async(app, prev, sender);
    }
}

/// Handle a failed playback decision (U7). Surfaces an actionable notice (so the
/// fallback is never silent — an incompatible file direct-playing reproduces the
/// original bug) and attempts a last-resort direct-play so something plays. The
/// quality menu (U9) upgrades this to a persistent banner with a force-transcode
/// action.
pub fn handle_playback_resolve_failed(
    app: &mut App,
    message: String,
    fallback_url: String,
    resume_secs: Option<f64>,
    epoch: u64,
    sender: &ComponentSender<App>,
) {
    use crate::components::player::switch_state::SwitchOutcome;
    // A failure from a superseded switch must not disturb the newer stream (U8):
    // the user already moved on, and the prior stream is still playing.
    if app.switch_state.evaluate(epoch) == SwitchOutcome::DiscardStale {
        debug!("Discarding stale playback failure (epoch {epoch})");
        return;
    }
    tracing::warn!("{message}; falling back to direct play");
    sender.input(AppMsg::ShowToast(message));
    let previous_session = app.active_transcode_session.take();
    app.current_decision = None;
    app.transcode_base_offset = 0.0;
    // The override is discarded on a failed switch — revert the menu to Auto so
    // the radio reflects the recovered (fallback) stream rather than the rung
    // the user tried and that failed to resolve (U9 menu-state-during-reload).
    app.current_quality = crate::models::playback::QualitySelection::Auto;
    app.video_player.emit(VideoPlayerMsg::SetUrl {
        url: Some(fallback_url),
        resume_secs,
        base_offset_secs: 0.0,
        is_transcode: false,
    });
    app.video_player.emit(VideoPlayerMsg::SetDecisionInfo {
        available: true,
        selection: crate::models::playback::QualitySelection::Auto,
        indicator: String::new(),
        audio_streams: Vec::new(),
        subtitle_streams: Vec::new(),
    });
    // No live transcode after a fallback — stop any keepalive, stop the old
    // session that the fallback superseded (U10/R14).
    super::transcode_keepalive::stop(app);
    if let Some(prev) = previous_session {
        stop_session_async(app, prev, sender);
    }
}

/// Handle ConnectionSaved: persist the source and wire it to all views. The
/// fields are the connection-dialog message payload threaded through verbatim
/// (`is_remote` is U2's connection-type classification, applied to the Plex
/// client for the default cap).
#[allow(clippy::too_many_arguments)]
pub fn handle_connection_saved(
    app: &mut App,
    url: String,
    token: String,
    name: String,
    source_type: SourceType,
    user_id: Option<String>,
    is_remote: bool,
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
    if let Some(db) = &app.db {
        db.with(|conn| {
            let mut repo = crate::db::source_repo::SourceRepo::new(conn);
            let _ = repo.delete(&source.id);
            if let Err(e) = repo.insert(&source) {
                tracing::warn!("Failed to save source: {e}");
            }
        });
    }

    // Build the source via the factory and wire it into every view. `is_remote`
    // (U2) is applied to the Plex client for the default bitrate cap (R6).
    if let Some(built) = super::source_factory::build_source(&source, is_remote) {
        // The downloads view needs the freshly-connected source too.
        app.downloads_view
            .emit(crate::components::downloads::DownloadsViewMsg::SetSource(
                built.clone(),
                app.artwork_cache.clone(),
            ));
        app.wire_active_source(source_type, url, built, sender);
    }

    // Send watch data to library view.
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

/// Handle NavigateHome: switch to the Home view, rendering from the session
/// content cache (instant + background revalidate) on a hit, or loading on a
/// miss.
pub fn handle_navigate_home(app: &mut App, root: &adw::ApplicationWindow) {
    app.current_view = CurrentView::Home;
    app.stack.set_visible_child_name("shell");
    app.nav_view.replace_with_tags(&["home"]);
    root.set_fullscreened(false);
    root.set_title(Some("Reel"));
    // Load home data: in-progress from local DB + recently_added from source
    app.home_view.emit(HomeViewMsg::SetVisibility(
        app.settings.library_visibility.hidden.clone(),
    ));
    // Home cache hit → instant render + background revalidate; miss → load.
    let set_key = app.home_source_set_key();
    if let Some(home) = app.content_cache.get_home(&set_key) {
        app.home_view
            .emit(HomeViewMsg::ShowCached(Box::new(home.clone())));
        // Revalidate in the background only when a server source can be
        // refreshed (Local-only Home revalidation is deferred). Gate on
        // the same condition HomeView uses so the in-flight flag can't
        // get stuck with no completion to clear it.
        let has_server = app
            .sources
            .iter()
            .any(|e| e.source_type.provides_server_hubs());
        if has_server && !app.home_revalidating {
            app.home_revalidating = true;
            app.home_revalidate_epoch = app.content_cache.source_set_epoch();
            app.home_view.emit(HomeViewMsg::Revalidate);
        }
    } else {
        let in_progress = load_in_progress(&app.db);
        app.home_view.emit(HomeViewMsg::LoadHome { in_progress });
    }
}

/// Handle Navigate: switch to a library view, pointing the browsed source at
/// the navigated library's owner and rendering from the session content cache
/// (instant + background revalidate) on a hit, or loading on a miss.
pub fn handle_navigate(
    app: &mut App,
    section: LibrarySection,
    source_type: String,
    source_id: String,
    sender: &ComponentSender<App>,
    root: &adw::ApplicationWindow,
) {
    app.current_view = CurrentView::Library(section.key.clone());
    app.stack.set_visible_child_name("shell");
    app.nav_view.replace_with_tags(&["library"]);
    root.set_fullscreened(false);
    root.set_title(Some("Reel"));
    app.library_title.set_label(&section.title);
    // Point the browsed source at the navigated library's owner and
    // feed that source to the LibraryView list (per-item paths still
    // resolve their own source).
    let parsed = SourceType::from_str(&source_type);
    if let Some(st) = parsed {
        app.browsed_source = Some((st, source_id.clone()));
        if let Some(src) = app.sources.get(st, &source_id) {
            app.library_view
                .emit(LibraryViewMsg::SetSource(src, app.artwork_cache.clone()));
        }
    }
    // Session-cache hit → render instantly without a fetch; miss → load.
    let cached = parsed.and_then(|st| {
        let key = SessionContentCache::library_key(st, &source_id, &section.key);
        app.content_cache.get_library(&key).map(<[_]>::to_vec)
    });
    match cached {
        Some(items) => {
            // Instant render, then revalidate in the background (U5).
            let st = parsed.expect("cache hit implies a parsed source type");
            let cache_key = SessionContentCache::library_key(st, &source_id, &section.key);
            let section_key = section.key.clone();
            let src = app.sources.get(st, &source_id);
            app.library_view
                .emit(LibraryViewMsg::ShowCached(section, items));
            if let Some(src) = src {
                app.revalidate_library(cache_key, section_key, src, sender);
            }
        }
        None => app.library_view.emit(LibraryViewMsg::LoadLibrary(section)),
    }
}

/// Handle CacheLibraryItems: store freshly-fetched items in the session content
/// cache, keyed by the items' OWN source so a late-returning fetch caches under
/// the correct key even after the user navigated away.
pub fn handle_cache_library_items(app: &mut App, section_key: String, items: Vec<MediaItem>) {
    // Key from the items' OWN source, not the (possibly already
    // advanced) browsed source: a fetch for library A returning after
    // the user navigated to B must still cache under A's key. Empty
    // results carry no source, so fall back to the browsed source.
    let source = items
        .first()
        .map(|i| (i.source_type, i.source_id.clone()))
        .or_else(|| app.browsed_source.clone());
    if let Some((st, source_id)) = source {
        let key = SessionContentCache::library_key(st, &source_id, &section_key);
        app.content_cache.put_library(&key, items);
    }
}

/// Handle CacheHome: stamp the assembled Home payload with the current source
/// set and store it, re-rendering in place only when a background revalidation
/// actually changed the content.
pub fn handle_cache_home(app: &mut App, home: CachedHome) {
    // Stamp the assembled payload with the current source set.
    let mut home = home;
    home.source_set_key = app.home_source_set_key();
    let was_revalidating = std::mem::take(&mut app.home_revalidating);
    // A revalidation whose source set changed mid-flight is stale: drop
    // it (the changed set already invalidated Home and will reload).
    if was_revalidating
        && app
            .content_cache
            .source_set_changed_since(app.home_revalidate_epoch)
    {
        return;
    }
    // Re-render only when revalidation actually changed the content, so
    // a revisit doesn't reset Home's scroll for nothing. Clone the
    // payload only when we actually re-render; otherwise store by move.
    let changed = app
        .content_cache
        .get_home(&home.source_set_key)
        .is_none_or(|cached| !crate::services::session_cache::home_content_eq(cached, &home));
    if was_revalidating && changed && app.current_view == CurrentView::Home {
        app.content_cache.set_home(home.clone());
        app.home_view.emit(HomeViewMsg::ShowCached(Box::new(home)));
    } else {
        app.content_cache.set_home(home);
    }
}

/// Handle SidebarEditModeExited: refresh Home's visibility, and if the library
/// currently being viewed was just hidden, drop to Home.
pub fn handle_sidebar_edit_mode_exited(app: &mut App, sender: &ComponentSender<App>) {
    let hidden = app.settings.library_visibility.hidden.clone();
    app.home_view
        .emit(HomeViewMsg::SetVisibility(hidden.clone()));
    // If the library being viewed was just hidden, drop to Home.
    // Key by the browsed source's own type + id (not a hardcoded
    // "plex") so per-source visibility resolves for any backend.
    let redirect = match (&app.current_view, &app.browsed_source) {
        (CurrentView::Library(key), Some((source_type, source_id))) => hidden.contains(
            &LibrarySection::visibility_key_for(source_type.as_str(), source_id, key),
        ),
        _ => false,
    };
    if redirect {
        sender.input(AppMsg::NavigateHome);
    } else if matches!(app.current_view, CurrentView::Home) {
        // Refresh Home in place so it reflects the new visibility.
        let in_progress = load_in_progress(&app.db);
        app.home_view.emit(HomeViewMsg::LoadHome { in_progress });
    }
}

/// Handle OpenDownloadDetail: resolve a download's stored `media_item_id` to a
/// library `MediaItem` (an episode routes to its parent show) and open the
/// matching detail page, or toast if it can't be resolved offline.
pub fn handle_open_download_detail(
    app: &mut App,
    media_item_id: String,
    sender: &ComponentSender<App>,
) {
    // Resolve the download's stored id to a library MediaItem; an
    // episode opens its parent show's detail page.
    let resolved = app.db.as_ref().and_then(|db| {
        db.with(|conn| {
            let mut repo = crate::db::media_repo::MediaRepo::new(conn);
            let item = repo.find_by_id(&media_item_id).ok().flatten()?;
            if item.media_type == MediaType::Episode {
                let parent_id = item.parent_id.clone()?;
                repo.find_by_id(&parent_id).ok().flatten()
            } else {
                Some(item)
            }
        })
    });
    match resolved {
        Some(item) => match item.media_type {
            MediaType::Movie => sender.input(AppMsg::ShowMovieDetail(item)),
            MediaType::Show => sender.input(AppMsg::ShowShowDetail(item)),
            _ => sender.input(AppMsg::ShowToast("Can't open this download".to_string())),
        },
        None => {
            sender.input(AppMsg::ShowToast("Details unavailable offline".to_string()));
        }
    }
}

/// Handle MarkWatched: persist watched state locally, fire-and-forget scrobble
/// to the owning server, patch the session cache for an instant revisit, and
/// refresh the library view's watch data.
pub fn handle_mark_watched(app: &mut App, item: MediaItem, sender: &ComponentSender<App>) {
    info!("Marking as watched: {}", item.title);
    if let Some(db) = &app.db {
        db.with(|conn| {
            let mut repo = WatchProgressRepo::new(conn);
            let progress = WatchProgress {
                media_item_id: item.id.clone(),
                position_seconds: 0.0,
                duration_seconds: item.runtime_minutes.map(|m| m as f64 * 60.0).unwrap_or(0.0),
                watched: true,
                last_watched_at: iso_now(),
            };
            let _ = repo.upsert(&progress);
        });
    }
    // Fire-and-forget scrobble to the item's owning server.
    if item.source_type.reports_watch_state()
        && let Some(source) = app.sources.for_item(&item)
    {
        let key = item.external_id.clone();
        sender.oneshot_command(async move {
            if let Err(e) = source.scrobble(&key).await {
                tracing::warn!("Scrobble failed: {e}");
            }
            AppCmd::Noop
        });
    }
    // R8: patch cached entries so a revisit shows it immediately.
    app.note_local_watch_mutation(&item.id, true, None);
    // Refresh watch data
    let watch_data = load_watch_data(&app.db);
    app.library_view
        .emit(LibraryViewMsg::SetWatchData(watch_data));
    sender.input(AppMsg::ShowToast(format!(
        "Marked \"{}\" as watched",
        item.title
    )));
}

/// Handle MarkUnwatched: clear watched state locally, fire-and-forget unscrobble
/// to the owning server, patch the session cache for an instant revisit, and
/// refresh the library view's watch data.
pub fn handle_mark_unwatched(app: &mut App, item: MediaItem, sender: &ComponentSender<App>) {
    info!("Marking as unwatched: {}", item.title);
    if let Some(db) = &app.db {
        db.with(|conn| {
            let mut repo = WatchProgressRepo::new(conn);
            let _ = repo.mark_unwatched(&item.id);
        });
    }
    // Fire-and-forget unscrobble to the item's owning server.
    if item.source_type.reports_watch_state()
        && let Some(source) = app.sources.for_item(&item)
    {
        let key = item.external_id.clone();
        sender.oneshot_command(async move {
            if let Err(e) = source.unscrobble(&key).await {
                tracing::warn!("Unscrobble failed: {e}");
            }
            AppCmd::Noop
        });
    }
    // R8: patch cached entries so a revisit shows it immediately.
    app.note_local_watch_mutation(&item.id, false, None);
    // Refresh watch data
    let watch_data = load_watch_data(&app.db);
    app.library_view
        .emit(LibraryViewMsg::SetWatchData(watch_data));
    sender.input(AppMsg::ShowToast(format!(
        "Marked \"{}\" as unwatched",
        item.title
    )));
}

/// Handle SourceValidated: persist the (possibly rediscovered) source config
/// id-scoped, build + register it, make the first validated source browsed, and
/// flush any offline-recorded progress on reconnect.
pub fn handle_source_validated(
    app: &mut App,
    source: Source,
    original_id: String,
    is_remote: bool,
    sender: &ComponentSender<App>,
) {
    let source_start = Instant::now();
    info!(
        "{:?} source validated: {} (url={})",
        source.source_type, source.name, source.config.url
    );

    app.source_connecting = false;

    // Id-scoped upsert: clear only THIS source's row(s) — the new id
    // and the pre-validation id (in case a Plex URL changed) — then
    // insert. Never delete-all: another source's saved config (and so
    // its ability to reconnect) must survive this revalidation.
    // Media / watch rows are NEVER touched here — eviction happens
    // only via the explicit remove-source path (R5 / U8).
    if let Some(db) = &app.db {
        db.with(|conn| {
            let mut repo = crate::db::source_repo::SourceRepo::new(conn);
            let _ = repo.delete(&source.id);
            if original_id != source.id {
                let _ = repo.delete(&original_id);
            }
            if let Err(e) = repo.insert(&source) {
                tracing::warn!("Failed to update source: {e}");
            }
        });
    }

    // Build via the factory and register. The first validated source
    // becomes the browsed one and is wired into the views; additional
    // sources are registered so per-item resolution and (U8/U9) the
    // multi-source UI can reach them. `is_remote` (U2) is applied to
    // the Plex client for the default bitrate cap (R6).
    if let Some(built) = super::source_factory::build_source(&source, is_remote) {
        let source_id = source.config.url.clone();
        // Every validated source becomes its own sidebar group so
        // all servers appear; only the first one also becomes the
        // browsed source that drives Home + the LibraryView list.
        let make_browsed = app.browsed_source.is_none();
        app.feed_sidebar_source(
            source.source_type,
            &source_id,
            &source.name,
            built.clone(),
            sender,
        );
        if make_browsed {
            app.downloads_view.emit(DownloadsViewMsg::SetSource(
                built.clone(),
                app.artwork_cache.clone(),
            ));
            app.set_browsed_views(source.source_type, &source_id, built);
        }
    }

    // Reconnect: flush any progress recorded offline back to the
    // source before any inbound browse can surface a stale offset.
    super::watch_events::flush_pending_sync(app, sender);

    info!(
        "Source setup took {:?} (DB save + watch data)",
        source_start.elapsed()
    );

    // Switch home view from connecting page to shelves.
    app.home_view.emit(HomeViewMsg::SetConnecting(false));
    // A specific library loads when the user picks it from the
    // sidebar; the default view is Home, so no eager load here.

    // The source is live — start any queued/recovered downloads.
    super::download_handlers::start_pending(app);
}

/// Handle LibraryRevalidated: apply a background library refetch, guarded by the
/// stamped epochs and merged with any racing local mutation, applying in place
/// only when that exact library is currently on screen.
pub fn handle_library_revalidated(
    app: &mut App,
    cache_key: String,
    entry_epoch: u64,
    source_set_epoch: u64,
    dispatch_seq: u64,
    result: Result<Vec<MediaItem>, String>,
) {
    app.revalidating.remove(&cache_key);
    // Failed refetch: leave the cached content untouched (R6).
    let Ok(items) = result else {
        return;
    };
    // Drop if the entry was evicted/superseded or the source set
    // changed since dispatch (KTD-5 staleness guards).
    if app.content_cache.library_epoch(&cache_key) != Some(entry_epoch)
        || app.content_cache.source_set_changed_since(source_set_epoch)
    {
        return;
    }
    let Some(cached) = app
        .content_cache
        .peek_library(&cache_key)
        .map(<[_]>::to_vec)
    else {
        return;
    };
    // Merge: server-authoritative except for a racing local mutation.
    let merged = crate::services::library_filter::merge_revalidated(
        items,
        &cached,
        &app.content_last_mutation,
        dispatch_seq,
    );
    let diff = crate::services::session_cache::diff_items(&cached, &merged);
    if diff.is_empty() {
        // Nothing changed: leave the view (and its scroll) untouched.
        return;
    }
    // Store the merged result; apply in place only if THIS library
    // (by full composite key, not bare section key — two servers can
    // share a section key) is on screen. Clone only when both the
    // cache and the view need it.
    let on_screen = matches!(
        (&app.current_view, &app.browsed_source),
        (CurrentView::Library(sk), Some((bt, bid)))
            if SessionContentCache::library_key(*bt, bid, sk) == cache_key
    );
    if on_screen {
        app.content_cache.put_library(&cache_key, merged.clone());
        app.library_view.emit(LibraryViewMsg::ApplyRevalidated {
            cache_key,
            items: merged,
        });
    } else {
        app.content_cache.put_library(&cache_key, merged);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_decision_gate_admits_plex_and_jellyfin_only() {
        assert!(supports_server_decision(SourceType::Plex));
        assert!(supports_server_decision(SourceType::Jellyfin));
        assert!(!supports_server_decision(SourceType::Local));
    }

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
