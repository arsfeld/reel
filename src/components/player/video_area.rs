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

use crate::player::backend::{EndReason, PlayState};
use crate::player::mpv::gl_render;

/// Intermediate struct to hold polled mpv property values.
struct PollData {
    file_loaded: bool,
    last_position: f64,
    last_paused: Option<bool>,
    path: Option<String>,
    duration: Option<f64>,
    position: Option<f64>,
    paused: Option<bool>,
    eof: Option<bool>,
    hwdec: Option<String>,
}

/// Wrapper for mpv_render_context pointer.
struct RenderCtxPtr(*mut mpv_render_context);

/// State shared between closures via Rc<RefCell<>>.
struct MpvState {
    mpv: Option<Mpv>,
    render_ctx: Option<RenderCtxPtr>,
    last_position: f64,
    last_paused: Option<bool>,
    file_loaded: bool,
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
pub enum VideoAreaMsg {
    LoadFile(String),
    TogglePause,
    PollState,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum VideoAreaOutput {
    StateChanged(PlayState),
    PositionChanged { position: f64, duration: f64 },
    FileLoaded,
    EndOfFile(EndReason),
}

pub struct VideoArea {
    state: Rc<RefCell<MpvState>>,
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
            last_position: -1.0,
            last_paused: None,
            file_loaded: false,
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

        let model = Self {
            state,
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
                let st = self.state.borrow();
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
            VideoAreaMsg::PollState => {
                // Collect all values from mpv with an immutable borrow first
                let poll_data = {
                    let st = self.state.borrow();
                    let Some(ref mpv) = st.mpv else {
                        return;
                    };

                    PollData {
                        file_loaded: st.file_loaded,
                        last_position: st.last_position,
                        last_paused: st.last_paused,
                        path: mpv.get_property::<String>("path").ok(),
                        duration: mpv.get_property::<f64>("duration").ok(),
                        position: mpv.get_property::<f64>("playback-time").ok(),
                        paused: mpv.get_property::<bool>("pause").ok(),
                        eof: mpv.get_property::<bool>("eof-reached").ok(),
                        hwdec: mpv.get_property::<String>("hwdec-current").ok(),
                    }
                };

                // Now process the polled data and update state
                let mut st = self.state.borrow_mut();

                // Check file loaded
                if !poll_data.file_loaded
                    && let Some(ref path) = poll_data.path
                    && !path.is_empty()
                    && let Some(dur) = poll_data.duration
                    && dur > 0.0
                {
                    st.file_loaded = true;
                    info!("File loaded");
                    if let Some(ref hwdec) = poll_data.hwdec {
                        info!("Hardware decoding: {}", hwdec);
                    }
                    let _ = sender.output(VideoAreaOutput::FileLoaded);
                }

                // Position update
                if let Some(pos) = poll_data.position
                    && (pos - poll_data.last_position).abs() > 0.05
                {
                    st.last_position = pos;
                    let dur = poll_data.duration.unwrap_or(0.0);
                    let _ = sender.output(VideoAreaOutput::PositionChanged {
                        position: pos,
                        duration: dur,
                    });
                }

                // Pause state change
                if let Some(paused) = poll_data.paused
                    && poll_data.last_paused != Some(paused)
                {
                    st.last_paused = Some(paused);
                    let play_state = if paused {
                        PlayState::Paused
                    } else {
                        PlayState::Playing
                    };
                    info!("State changed: {:?}", play_state);
                    let _ = sender.output(VideoAreaOutput::StateChanged(play_state));
                }

                // EOF check
                if let Some(true) = poll_data.eof
                    && poll_data.file_loaded
                {
                    info!("End of file reached");
                    st.file_loaded = false;
                    let _ = sender.output(VideoAreaOutput::EndOfFile(EndReason::Finished));
                }
            }
        }
    }
}
