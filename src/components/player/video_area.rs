use std::cell::RefCell;
use std::ffi::c_void;
use std::rc::Rc;
use std::time::Duration;

use gtk4::glib;
use gtk4::prelude::*;
use libmpv2::Mpv;
use libmpv2_sys::*;
use relm4::prelude::*;
use tracing::{debug, error, info, warn};

use crate::components::player::controls::{ControlsInput, ControlsOutput, PlayerControls};
use crate::components::player::drop_target;
use crate::components::player::overlay_controller::{self, OverlayAction, OverlayState};
use crate::player::backend::{self, EndReason, PlayState};
use crate::player::mpv::gl_render;
use crate::player::playback_tracker::{PlaybackEvent, PlaybackTracker, PollData};

/// Wrapper for mpv_render_context pointer.
struct RenderCtxPtr(*mut mpv_render_context);

/// State shared between closures via Rc<RefCell<>>.
struct MpvState {
    mpv: Option<Mpv>,
    render_ctx: Option<RenderCtxPtr>,
    tracker: PlaybackTracker,
}

impl Drop for MpvState {
    fn drop(&mut self) {
        // Free render context before mpv handle (order matters)
        if let Some(RenderCtxPtr(ctx)) = self.render_ctx.take() {
            unsafe {
                gl_render::free_render_context(ctx);
            }
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum VideoAreaMsg {
    // Playback
    LoadFile(String),
    TogglePause,
    SeekAbsolute(f64),
    SeekRelative(f64),
    // Volume
    SetVolume(f64),
    VolumeStep(f64),
    ToggleMute,
    // Speed
    SetSpeed(f64),
    SpeedUp,
    SpeedDown,
    SpeedReset,
    // Tracks
    SetAudioTrack(i64),
    SetSubtitleTrack(i64),
    DisableSubtitles,
    AddSubtitleFile(String),
    // Chapters
    SetChapter(i64),
    // Window
    ToggleFullscreen,
    FullscreenChanged(bool),
    // File drop
    FilesDropped(String),
    // Internal
    PollState,
    MouseMoved,
    HideTimeout,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum VideoAreaOutput {
    StateChanged(PlayState),
    PositionChanged { position: f64, duration: f64 },
    FileLoaded,
    EndOfFile(EndReason),
    VolumeChanged { volume: f64, muted: bool },
    SpeedChanged(f64),
    ToggleFullscreen,
}

pub struct VideoArea {
    state: Rc<RefCell<MpvState>>,
    controls: Controller<PlayerControls>,
    overlay_state: OverlayState,
    hide_timer: Option<glib::SourceId>,
    current_volume: f64,
    current_speed: f64,
    has_video: bool,
    #[allow(dead_code)]
    gl_area: gtk4::GLArea,
    #[allow(dead_code)]
    timer_handle: Option<glib::SourceId>,
    #[allow(dead_code)]
    poll_handle: Option<glib::SourceId>,
}

#[relm4::component(pub)]
impl Component for VideoArea {
    type Init = ();
    type Input = VideoAreaMsg;
    type Output = VideoAreaOutput;
    type CommandOutput = ();

    view! {
        #[name = "overlay"]
        gtk4::Overlay {
            #[name = "gl_area"]
            gtk4::GLArea {
                set_vexpand: true,
                set_hexpand: true,
                set_auto_render: true,
            },
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = view_output!();

        let gl_area = widgets.gl_area.clone();

        let state = Rc::new(RefCell::new(MpvState {
            mpv: None,
            render_ctx: None,
            tracker: PlaybackTracker::new(),
        }));

        // --- Realize: init mpv + render context ---
        let state_realize = state.clone();
        let gl_area_for_callback = gl_area.clone();
        gl_area.connect_realize(move |gl_area| {
            debug!("GLArea realized");
            gl_area.make_current();

            if let Some(error) = gl_area.error() {
                error!("GLArea error: {:?}", error);
                return;
            }

            let mut st = state_realize.borrow_mut();

            // Init mpv
            let mpv = match crate::player::mpv::MpvBackend::new() {
                Ok(backend) => backend.mpv,
                Err(e) => {
                    error!("Failed to create mpv: {:?}", e);
                    return;
                }
            };

            // Create render context
            let mpv_handle = mpv.ctx.as_ptr();
            let render_ctx = match unsafe { gl_render::create_render_context(mpv_handle) } {
                Ok(ctx) => ctx,
                Err(code) => {
                    error!("Failed to create render context: {}", code);
                    return;
                }
            };

            // Set update callback: when mpv has a new frame, queue a render on the GTK main thread
            let gl_area_weak = glib::SendWeakRef::from(gl_area_for_callback.downgrade());

            unsafe extern "C" fn on_render_update(ctx: *mut c_void) {
                unsafe {
                    let weak_ref = &*(ctx as *const glib::SendWeakRef<gtk4::GLArea>);
                    let weak_clone = weak_ref.clone();
                    glib::idle_add_once(move || {
                        if let Some(gl_area) = weak_clone.upgrade() {
                            gl_area.queue_render();
                        }
                    });
                }
            }

            let callback_ctx = Box::new(gl_area_weak);
            let callback_ctx_ptr = Box::into_raw(callback_ctx) as *mut c_void;

            unsafe {
                gl_render::set_update_callback(
                    render_ctx,
                    Some(on_render_update),
                    callback_ctx_ptr,
                );
            }

            st.mpv = Some(mpv);
            st.render_ctx = Some(RenderCtxPtr(render_ctx));

            info!("Video area initialized");
        });

        // --- Render: draw mpv frame ---
        let state_render = state.clone();
        gl_area.connect_render(move |gl_area, _ctx| {
            let st = state_render.borrow();
            if let Some(RenderCtxPtr(render_ctx)) = &st.render_ctx {
                let scale = gl_area.scale_factor();
                let w = gl_area.width() * scale;
                let h = gl_area.height() * scale;

                if w > 0 && h > 0 {
                    gl_area.attach_buffers();
                    unsafe {
                        gl_render::render_frame(*render_ctx, w, h);
                    }
                }
            }
            glib::Propagation::Proceed
        });

        // --- Unrealize: cleanup ---
        let state_unrealize = state.clone();
        gl_area.connect_unrealize(move |_gl_area| {
            debug!("GLArea unrealized");
            let mut st = state_unrealize.borrow_mut();
            if let Some(RenderCtxPtr(ctx)) = st.render_ctx.take() {
                unsafe {
                    gl_render::free_render_context(ctx);
                }
            }
        });

        // Render timer (16ms ~ 60fps)
        let gl_area_timer = gl_area.clone();
        let state_timer = state.clone();
        let timer_handle = glib::timeout_add_local(Duration::from_millis(16), move || {
            let st = state_timer.borrow();
            if st.render_ctx.is_some()
                && let Some(ref mpv) = st.mpv
                && let Ok(paused) = mpv.get_property::<bool>("pause")
                && !paused
            {
                gl_area_timer.queue_render();
            }
            glib::ControlFlow::Continue
        });

        // Poll mpv state (position, pause, etc.) every 100ms
        let sender_poll = sender.input_sender().clone();
        let poll_handle = glib::timeout_add_local(Duration::from_millis(100), move || {
            let _ = sender_poll.send(VideoAreaMsg::PollState);
            glib::ControlFlow::Continue
        });

        // Create player controls and add as overlay
        let controls =
            PlayerControls::builder()
                .launch(())
                .forward(sender.input_sender(), |output| match output {
                    ControlsOutput::TogglePause => VideoAreaMsg::TogglePause,
                    ControlsOutput::SeekTo(pos) => VideoAreaMsg::SeekAbsolute(pos),
                    ControlsOutput::SetVolume(vol) => VideoAreaMsg::SetVolume(vol),
                    ControlsOutput::ToggleMute => VideoAreaMsg::ToggleMute,
                    ControlsOutput::ToggleFullscreen => VideoAreaMsg::ToggleFullscreen,
                });

        // Add controls widget as overlay
        widgets.overlay.add_overlay(controls.widget());

        // Mouse motion controller for auto-hide
        let motion_controller = gtk4::EventControllerMotion::new();
        let sender_motion = sender.input_sender().clone();
        motion_controller.connect_motion(move |_controller, _x, _y| {
            let _ = sender_motion.send(VideoAreaMsg::MouseMoved);
        });
        widgets.overlay.add_controller(motion_controller);

        // Double-click gesture for fullscreen toggle
        let click_gesture = gtk4::GestureClick::new();
        click_gesture.set_button(1); // Left button
        let sender_click = sender.input_sender().clone();
        click_gesture.connect_released(move |gesture, n_press, _x, _y| {
            if n_press == 2 {
                let _ = sender_click.send(VideoAreaMsg::ToggleFullscreen);
                gesture.set_state(gtk4::EventSequenceState::Claimed);
            }
        });
        gl_area.add_controller(click_gesture);

        let model = Self {
            state,
            controls,
            overlay_state: OverlayState::default(),
            hide_timer: None,
            current_volume: 100.0,
            current_speed: 1.0,
            has_video: false,
            gl_area,
            timer_handle: Some(timer_handle),
            poll_handle: Some(poll_handle),
        };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            VideoAreaMsg::LoadFile(uri) => {
                info!("Loading file: {}", uri);
                self.current_speed = 1.0;
                self.has_video = false;
                let mut st = self.state.borrow_mut();
                let has_mpv = st.mpv.is_some();
                if has_mpv {
                    st.tracker.reset();
                }
                if let Some(ref mpv) = st.mpv {
                    if let Err(e) = mpv.command("loadfile", &[&uri, "replace"]) {
                        error!("Failed to load file: {:?}", e);
                    }
                } else {
                    warn!("mpv not initialized yet");
                }
            }
            VideoAreaMsg::TogglePause => {
                let st = self.state.borrow();
                if let Some(ref mpv) = st.mpv
                    && let Err(e) = mpv.command("cycle", &["pause"])
                {
                    error!("Failed to toggle pause: {:?}", e);
                }
            }
            VideoAreaMsg::SeekAbsolute(pos) => {
                // pos can be absolute seconds or a fraction 0.0-1.0 (from controls)
                let st = self.state.borrow();
                if let Some(ref mpv) = st.mpv {
                    // If pos is a fraction (from progress bar), convert to seconds
                    let seconds = if (0.0..=1.0).contains(&pos) {
                        let dur = mpv.get_property::<f64>("duration").unwrap_or(0.0);
                        pos * dur
                    } else {
                        pos
                    };
                    let pos_str = format!("{seconds}");
                    if let Err(e) = mpv.command("seek", &[&pos_str, "absolute"]) {
                        error!("Failed to seek: {:?}", e);
                    }
                }
            }
            VideoAreaMsg::SeekRelative(offset) => {
                let st = self.state.borrow();
                if let Some(ref mpv) = st.mpv {
                    let offset_str = format!("{offset}");
                    if let Err(e) = mpv.command("seek", &[&offset_str, "relative"]) {
                        error!("Failed to seek: {:?}", e);
                    }
                }
            }
            VideoAreaMsg::SetVolume(vol) => {
                self.current_volume = vol;
                let st = self.state.borrow();
                if let Some(ref mpv) = st.mpv
                    && let Err(e) = mpv.set_property("volume", vol)
                {
                    error!("Failed to set volume: {:?}", e);
                }
            }
            VideoAreaMsg::VolumeStep(delta) => {
                let new_vol = (self.current_volume + delta).clamp(0.0, 150.0);
                self.current_volume = new_vol;
                let st = self.state.borrow();
                if let Some(ref mpv) = st.mpv
                    && let Err(e) = mpv.set_property("volume", new_vol)
                {
                    error!("Failed to set volume: {:?}", e);
                }
            }
            VideoAreaMsg::ToggleMute => {
                let st = self.state.borrow();
                if let Some(ref mpv) = st.mpv {
                    let muted = mpv.get_property::<bool>("mute").unwrap_or(false);
                    if let Err(e) = mpv.set_property("mute", !muted) {
                        error!("Failed to toggle mute: {:?}", e);
                    }
                }
            }
            VideoAreaMsg::SetSpeed(speed) => {
                self.current_speed = speed;
                let st = self.state.borrow();
                if let Some(ref mpv) = st.mpv
                    && let Err(e) = mpv.set_property("speed", speed)
                {
                    error!("Failed to set speed: {:?}", e);
                }
            }
            VideoAreaMsg::SpeedUp => {
                let new_speed = backend::next_speed(self.current_speed);
                self.current_speed = new_speed;
                let st = self.state.borrow();
                if let Some(ref mpv) = st.mpv
                    && let Err(e) = mpv.set_property("speed", new_speed)
                {
                    error!("Failed to set speed: {:?}", e);
                }
            }
            VideoAreaMsg::SpeedDown => {
                let new_speed = backend::prev_speed(self.current_speed);
                self.current_speed = new_speed;
                let st = self.state.borrow();
                if let Some(ref mpv) = st.mpv
                    && let Err(e) = mpv.set_property("speed", new_speed)
                {
                    error!("Failed to set speed: {:?}", e);
                }
            }
            VideoAreaMsg::SpeedReset => {
                self.current_speed = 1.0;
                let st = self.state.borrow();
                if let Some(ref mpv) = st.mpv
                    && let Err(e) = mpv.set_property("speed", 1.0)
                {
                    error!("Failed to set speed: {:?}", e);
                }
            }
            VideoAreaMsg::SetAudioTrack(track_id) => {
                let st = self.state.borrow();
                if let Some(ref mpv) = st.mpv
                    && let Err(e) = mpv.set_property("aid", track_id)
                {
                    error!("Failed to set audio track: {:?}", e);
                }
            }
            VideoAreaMsg::SetSubtitleTrack(track_id) => {
                let st = self.state.borrow();
                if let Some(ref mpv) = st.mpv
                    && let Err(e) = mpv.set_property("sid", track_id)
                {
                    error!("Failed to set subtitle track: {:?}", e);
                }
            }
            VideoAreaMsg::DisableSubtitles => {
                let st = self.state.borrow();
                if let Some(ref mpv) = st.mpv
                    && let Err(e) = mpv.set_property("sid", "no")
                {
                    error!("Failed to disable subtitles: {:?}", e);
                }
            }
            VideoAreaMsg::SetChapter(chapter) => {
                let st = self.state.borrow();
                if let Some(ref mpv) = st.mpv
                    && let Err(e) = mpv.set_property("chapter", chapter)
                {
                    error!("Failed to set chapter: {:?}", e);
                }
            }
            VideoAreaMsg::ToggleFullscreen => {
                let _ = sender.output(VideoAreaOutput::ToggleFullscreen);
            }
            VideoAreaMsg::MouseMoved => {
                self.apply_overlay_action(&OverlayAction::MouseMoved, &sender);
            }
            VideoAreaMsg::HideTimeout => {
                self.apply_overlay_action(&OverlayAction::TimeoutFired, &sender);
            }
            VideoAreaMsg::FullscreenChanged(fs) => {
                self.apply_overlay_action(&OverlayAction::FullscreenChanged(fs), &sender);
            }
            VideoAreaMsg::AddSubtitleFile(path) => {
                let st = self.state.borrow();
                if let Some(ref mpv) = st.mpv
                    && let Err(e) = mpv.command("sub-add", &[&path, "select"])
                {
                    error!("Failed to add subtitle file: {:?}", e);
                }
            }
            VideoAreaMsg::FilesDropped(uri) => {
                let action = drop_target::classify_drop(&uri, self.has_video);
                match action {
                    drop_target::DropAction::PlayVideo(path) => {
                        sender.input(VideoAreaMsg::LoadFile(path));
                    }
                    drop_target::DropAction::LoadSubtitle(path) => {
                        sender.input(VideoAreaMsg::AddSubtitleFile(path));
                    }
                    drop_target::DropAction::Unsupported => {}
                }
            }
            VideoAreaMsg::PollState => {
                // Collect all values from mpv with an immutable borrow first
                let poll_data = {
                    let st = self.state.borrow();
                    let Some(ref mpv) = st.mpv else {
                        return;
                    };

                    PollData {
                        path: mpv.get_property::<String>("path").ok(),
                        duration: mpv.get_property::<f64>("duration").ok(),
                        position: mpv.get_property::<f64>("playback-time").ok(),
                        paused: mpv.get_property::<bool>("pause").ok(),
                        eof_reached: mpv.get_property::<bool>("eof-reached").ok(),
                        hwdec_current: mpv.get_property::<String>("hwdec-current").ok(),
                        volume: mpv.get_property::<f64>("volume").ok(),
                        muted: mpv.get_property::<bool>("mute").ok(),
                        speed: mpv.get_property::<f64>("speed").ok(),
                    }
                };

                // Process through the tracker and emit events
                let events = {
                    let mut st = self.state.borrow_mut();
                    st.tracker.process(&poll_data)
                };

                for event in events {
                    match event {
                        PlaybackEvent::FileLoaded { hwdec, .. } => {
                            info!("File loaded");
                            self.has_video = true;
                            if let Some(ref h) = hwdec {
                                info!("Hardware decoding: {}", h);
                            }
                            let _ = sender.output(VideoAreaOutput::FileLoaded);
                        }
                        PlaybackEvent::PositionChanged { position, duration } => {
                            self.controls
                                .emit(ControlsInput::Position { position, duration });
                            let _ = sender
                                .output(VideoAreaOutput::PositionChanged { position, duration });
                        }
                        PlaybackEvent::StateChanged(state) => {
                            info!("State changed: {:?}", state);
                            self.controls.emit(ControlsInput::PlayStateChanged(state));
                            let is_paused = state == PlayState::Paused;
                            self.apply_overlay_action(
                                &OverlayAction::PauseChanged(is_paused),
                                &sender,
                            );
                            let _ = sender.output(VideoAreaOutput::StateChanged(state));
                        }
                        PlaybackEvent::EndOfFile(reason) => {
                            info!("End of file reached");
                            let _ = sender.output(VideoAreaOutput::EndOfFile(reason));
                        }
                        PlaybackEvent::VolumeChanged { volume, muted } => {
                            self.current_volume = volume;
                            self.controls.emit(ControlsInput::Volume { volume, muted });
                            let _ = sender.output(VideoAreaOutput::VolumeChanged { volume, muted });
                        }
                        PlaybackEvent::SpeedChanged(speed) => {
                            self.current_speed = speed;
                            let _ = sender.output(VideoAreaOutput::SpeedChanged(speed));
                        }
                    }
                }
            }
        }
    }
}

impl VideoArea {
    fn apply_overlay_action(&mut self, action: &OverlayAction, sender: &ComponentSender<Self>) {
        let visible = overlay_controller::compute_overlay_visibility(&self.overlay_state, action);

        // Update overlay state
        match action {
            OverlayAction::FullscreenChanged(fs) => self.overlay_state.is_fullscreen = *fs,
            OverlayAction::PauseChanged(p) => self.overlay_state.is_paused = *p,
            OverlayAction::PopoverOpened => self.overlay_state.popover_open = true,
            OverlayAction::PopoverClosed => self.overlay_state.popover_open = false,
            _ => {}
        }
        self.overlay_state.visible = visible;

        // Show/hide controls
        self.controls.widget().set_visible(visible);

        // Cursor hiding in fullscreen
        if overlay_controller::should_hide_cursor(self.overlay_state.is_fullscreen, visible) {
            self.gl_area.set_cursor_from_name(Some("none"));
        } else {
            self.gl_area.set_cursor_from_name(Some("default"));
        }

        // Manage auto-hide timer
        if let Some(handle) = self.hide_timer.take() {
            handle.remove();
        }

        // Start new timer if visible and should auto-hide
        if visible
            && self.overlay_state.is_fullscreen
            && !self.overlay_state.is_paused
            && !self.overlay_state.popover_open
        {
            let sender_timeout = sender.input_sender().clone();
            self.hide_timer = Some(glib::timeout_add_local_once(
                Duration::from_secs(3),
                move || {
                    let _ = sender_timeout.send(VideoAreaMsg::HideTimeout);
                },
            ));
        }
    }
}
