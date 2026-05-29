//! Fixed-interval keepalive for the active Plex transcode session (U10/R15).
//!
//! Plex reaps an un-pinged transcode session after ~76s (measured, U1). We ping
//! on a timer independent of playback progress — at ≤ ⅓ of that reap window
//! (KTD7) — so a *paused* transcode is not reaped. The timer is owned by `App`
//! (`keepalive_source`); it is restarted whenever a new transcode session
//! becomes active and stopped on stop/leave/fallback. After a run of
//! consecutive ping failures the session is declared lost and the user is asked
//! to re-pick a quality (no silent auto-reload — deferred per the plan).

use gtk::glib;
use relm4::ComponentSender;
use relm4::prelude::*;
use tracing::debug;

use super::{App, AppCmd, AppMsg};

/// Keepalive cadence. ≤ ⅓ of the measured ~76s reap timeout (U1, KTD7).
const KEEPALIVE_INTERVAL_SECS: u64 = 15;

/// Consecutive ping failures before a session is declared lost.
pub(super) const MAX_FAILURES: u32 = 3;

/// (Re)start the keepalive timer for the current `active_transcode_session`.
/// Stops any prior timer first; a no-op when no transcode session is active
/// (e.g. a direct-play decision).
pub(super) fn restart(app: &mut App, sender: &ComponentSender<App>) {
    stop(app);
    app.keepalive_failures = 0;
    if app.active_transcode_session.is_none() {
        return;
    }
    let sender = sender.clone();
    let id = glib::timeout_add_local(
        std::time::Duration::from_secs(KEEPALIVE_INTERVAL_SECS),
        move || {
            sender.input(AppMsg::TranscodeKeepalive);
            glib::ControlFlow::Continue
        },
    );
    app.keepalive_source = Some(id);
}

/// Stop and clear the keepalive timer (no session active / playback ended).
pub(super) fn stop(app: &mut App) {
    if let Some(id) = app.keepalive_source.take() {
        id.remove();
    }
    app.keepalive_failures = 0;
}

/// Timer tick: ping the active session in the background. No-op when the session
/// or source is gone (a race with teardown).
pub(super) fn tick(app: &App, sender: &ComponentSender<App>) {
    let (Some(session), Some(source)) = (
        app.active_transcode_session.clone(),
        app.now_playing_source(),
    ) else {
        return;
    };
    sender.oneshot_command(async move {
        match source.ping_transcode(&session).await {
            Ok(()) => AppCmd::KeepalivePinged(true),
            Err(e) => {
                debug!("keepalive ping failed: {e}");
                AppCmd::KeepalivePinged(false)
            }
        }
    });
}

/// Record a ping result. Resets the failure counter on success; on a run of
/// `MAX_FAILURES` consecutive failures the session is declared lost: the timer
/// stops, the session id is dropped (no further stop attempt — it is gone or
/// unreachable), and the user is told to re-pick a quality to restart.
pub(super) fn record_result(app: &mut App, ok: bool, sender: &ComponentSender<App>) {
    if ok {
        app.keepalive_failures = 0;
        return;
    }
    app.keepalive_failures = app.keepalive_failures.saturating_add(1);
    if app.keepalive_failures < MAX_FAILURES {
        return;
    }
    stop(app);
    app.active_transcode_session = None;
    sender.input(AppMsg::ShowToast(
        "Transcode session lost — pick a quality to restart playback".to_string(),
    ));
}
