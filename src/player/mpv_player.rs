use crate::config::Config;
use anyhow::Result;
use gtk4::GLArea;
use gtk4::{self, glib, prelude::*};
use libmpv2::Mpv;
use libmpv2_sys::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_void};
use std::ptr;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

// MPV render update flags
const MPV_RENDER_UPDATE_FRAME: u64 = 1;

#[derive(Debug, Clone)]
pub enum PlayerState {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopped,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UpscalingMode {
    None,
    HighQuality,
    FSR,
    Anime,
}

impl UpscalingMode {
    pub fn next(&self) -> Self {
        match self {
            UpscalingMode::None => UpscalingMode::HighQuality,
            UpscalingMode::HighQuality => UpscalingMode::FSR,
            UpscalingMode::FSR => UpscalingMode::Anime,
            UpscalingMode::Anime => UpscalingMode::None,
        }
    }

    pub fn to_string(&self) -> &'static str {
        match self {
            UpscalingMode::None => "None",
            UpscalingMode::HighQuality => "High Quality",
            UpscalingMode::FSR => "FSR",
            UpscalingMode::Anime => "Anime",
        }
    }
}

struct MpvPlayerInner {
    mpv: RefCell<Option<Mpv>>,
    mpv_gl: RefCell<Option<*mut mpv_render_context>>,
    state: Arc<RwLock<PlayerState>>,
    gl_area: RefCell<Option<GLArea>>,
    update_callback_registered: Cell<bool>,
    pending_media_url: RefCell<Option<String>>,
    frame_pending: Arc<AtomicBool>,
    last_render_time: RefCell<Instant>,
    render_count: Arc<AtomicU64>,
    cached_fbo: Cell<i32>,
    timer_handle: RefCell<Option<glib::SourceId>>,
    verbose_logging: bool,
    cache_size_mb: u32,
    cache_backbuffer_mb: u32,
    cache_secs: u32,
    seek_pending: Arc<Mutex<Option<(f64, Instant)>>>,
    seek_timer: RefCell<Option<glib::SourceId>>,
    last_seek_target: Arc<Mutex<Option<f64>>>,
    upscaling_mode: Arc<Mutex<UpscalingMode>>,
}

#[derive(Clone)]
pub struct MpvPlayer {
    inner: Rc<MpvPlayerInner>,
}

unsafe impl Send for MpvPlayer {}
unsafe impl Sync for MpvPlayer {}

impl MpvPlayer {
    pub fn new(config: &Config) -> Result<Self> {
        let verbose_logging = config.playback.mpv_verbose_logging;
        let cache_size_mb = config.playback.mpv_cache_size_mb;
        let cache_backbuffer_mb = config.playback.mpv_cache_backbuffer_mb;
        let cache_secs = config.playback.mpv_cache_secs;

        info!(
            "Initializing MPV player (verbose_logging: {}, cache: {}MB/{}s)",
            verbose_logging, cache_size_mb, cache_secs
        );

        Ok(Self {
            inner: Rc::new(MpvPlayerInner {
                mpv: RefCell::new(None),
                mpv_gl: RefCell::new(None),
                state: Arc::new(RwLock::new(PlayerState::Idle)),
                gl_area: RefCell::new(None),
                update_callback_registered: Cell::new(false),
                pending_media_url: RefCell::new(None),
                frame_pending: Arc::new(AtomicBool::new(false)),
                last_render_time: RefCell::new(Instant::now()),
                render_count: Arc::new(AtomicU64::new(0)),
                cached_fbo: Cell::new(-1),
                timer_handle: RefCell::new(None),
                verbose_logging,
                cache_size_mb,
                cache_backbuffer_mb,
                cache_secs,
                seek_pending: Arc::new(Mutex::new(None)),
                seek_timer: RefCell::new(None),
                last_seek_target: Arc::new(Mutex::new(None)),
                upscaling_mode: Arc::new(Mutex::new(UpscalingMode::None)),
            }),
        })
    }

    unsafe extern "C" fn get_proc_address(ctx: *mut c_void, name: *const i8) -> *mut c_void {
        unsafe {
            // Static cache for proc lookups - they never change once resolved
            static mut PROC_CACHE: Option<HashMap<String, *mut c_void>> = None;
            static mut GL_GET_PROC: Option<*mut c_void> = None;

            // Initialize cache on first use
            let cache_ptr = &raw mut PROC_CACHE;
            if (*cache_ptr).is_none() {
                *cache_ptr = Some(HashMap::new());
                // Cache the OpenGL proc address function itself
                let gl_ptr = &raw mut GL_GET_PROC;

                #[cfg(target_os = "macos")]
                {
                    // On macOS, we need to use the OpenGL framework directly
                    // Try to get NSOpenGLGetProcAddress or use dlsym on the OpenGL framework
                    *gl_ptr = None; // macOS doesn't have a global getProcAddress
                }

                #[cfg(not(target_os = "macos"))]
                {
                    // On Linux, use EGL
                    *gl_ptr = Some(libc::dlsym(
                        libc::RTLD_DEFAULT,
                        b"eglGetProcAddress\0".as_ptr() as *const i8,
                    ));
                }
            }

            let name_str = CStr::from_ptr(name).to_string_lossy().to_string();

            // Check cache first - use raw pointer to avoid reference issues
            let cache_ptr = &raw mut PROC_CACHE;
            if let Some(cache) = &mut *cache_ptr
                && let Some(&cached_proc) = cache.get(&name_str)
            {
                return cached_proc;
            }

            // Get the GLArea and make context current only if not cached
            let gl_area = &*(ctx as *const GLArea);
            if let Some(gl_context) = gl_area.context() {
                gl_context.make_current();
            }

            let mut func = ptr::null_mut();

            #[cfg(target_os = "macos")]
            {
                // On macOS, first try to get the proc address through GTK's GL context
                // GTK4 on macOS uses native OpenGL, so we need to use dlsym directly
                let cname = CString::new(name_str.clone()).unwrap();

                // Try different OpenGL libraries on macOS
                let framework1 = b"/System/Library/Frameworks/OpenGL.framework/OpenGL\0";
                let framework2 =
                    b"/System/Library/Frameworks/OpenGL.framework/Libraries/libGL.dylib\0";

                let handle = libc::dlopen(framework1.as_ptr() as *const i8, libc::RTLD_LAZY);
                if !handle.is_null() {
                    func = libc::dlsym(handle, cname.as_ptr());
                }

                if func.is_null() {
                    let handle = libc::dlopen(framework2.as_ptr() as *const i8, libc::RTLD_LAZY);
                    if !handle.is_null() {
                        func = libc::dlsym(handle, cname.as_ptr());
                    }
                }

                // Final fallback to RTLD_DEFAULT
                if func.is_null() {
                    func = libc::dlsym(libc::RTLD_DEFAULT, cname.as_ptr());
                }
            }

            #[cfg(not(target_os = "macos"))]
            {
                // Use cached EGL get proc function on Linux
                let gl_ptr = &raw const GL_GET_PROC;
                if let Some(egl_get_proc) = *gl_ptr
                    && !egl_get_proc.is_null()
                {
                    type EglGetProcFn = unsafe extern "C" fn(*const i8) -> *mut c_void;
                    let get_proc: EglGetProcFn = std::mem::transmute(egl_get_proc);
                    func = get_proc(name);
                }

                // Fallback to dlsym if needed
                if func.is_null() {
                    func = libc::dlsym(libc::RTLD_DEFAULT, name);
                }
            }

            // Cache the result - use raw pointer
            let cache_ptr = &raw mut PROC_CACHE;
            if let Some(cache) = &mut *cache_ptr {
                cache.insert(name_str.clone(), func);
            }

            if func.is_null() {
                warn!("Failed to get proc address for: {}", name_str);
            }

            func
        }
    }

    unsafe extern "C" fn on_mpv_render_update(ctx: *mut c_void) {
        unsafe {
            // Use a simpler struct to pass through the context
            struct UpdateContext {
                gl_area: *const GLArea,
                frame_pending: *const AtomicBool,
            }

            let update_ctx = &*(ctx as *const UpdateContext);
            let gl_area = &*update_ctx.gl_area;
            let frame_pending = &*update_ctx.frame_pending;

            // Only queue render if no frame is already pending
            if !frame_pending.swap(true, Ordering::AcqRel) {
                // Queue render directly without idle_add for lower latency
                gl_area.queue_render();
            }
        }
    }

    fn init_gl_render_context(&self, gl_area: &GLArea) -> Result<()> {
        info!("Initializing OpenGL render context");

        // Ensure GL context is current
        gl_area.make_current();

        // Check if we have a valid GL context
        if let Some(error) = gl_area.error() {
            error!("GLArea has an error: {:?}", error);
            return Err(anyhow::anyhow!(
                "GLArea has an error: {:?}, cannot initialize render context",
                error
            ));
        }

        // Check if GL context is actually realized
        let gl_context = gl_area
            .context()
            .ok_or_else(|| anyhow::anyhow!("GLArea has no GL context"))?;

        // Check GL context properties
        gl_context.make_current();
        let is_legacy = gl_context.is_legacy();
        debug!("GL context available - Legacy: {}", is_legacy);

        let mpv = self.inner.mpv.borrow();
        let mpv = mpv
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("MPV not initialized"))?;

        unsafe {
            // Get raw MPV handle
            let mpv_handle = mpv.ctx.as_ptr();

            // Prepare OpenGL render context parameters
            let mut mpv_gl: *mut mpv_render_context = ptr::null_mut();

            // Create render params
            let api_type = CString::new("opengl").unwrap();
            let gl_area_ptr = gl_area as *const GLArea as *mut c_void;

            // Log API version first
            debug!("Setting up MPV render API with type: opengl");

            let opengl_params = mpv_opengl_init_params {
                get_proc_address: Some(Self::get_proc_address),
                get_proc_address_ctx: gl_area_ptr,
            };

            let mut params = vec![
                mpv_render_param {
                    type_: mpv_render_param_type_MPV_RENDER_PARAM_API_TYPE,
                    data: api_type.as_ptr() as *mut c_void,
                },
                mpv_render_param {
                    type_: mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                    data: &opengl_params as *const _ as *mut c_void,
                },
                mpv_render_param {
                    type_: mpv_render_param_type_MPV_RENDER_PARAM_INVALID,
                    data: ptr::null_mut(),
                },
            ];

            // Create render context
            let result = mpv_render_context_create(&mut mpv_gl, mpv_handle, params.as_mut_ptr());

            if result < 0 {
                let error_msg = match result {
                    -1 => "MPV_ERROR_EVENT_QUEUE_FULL",
                    -2 => "MPV_ERROR_NOMEM",
                    -3 => "MPV_ERROR_UNINITIALIZED",
                    -4 => "MPV_ERROR_INVALID_PARAMETER",
                    -5 => "MPV_ERROR_OPTION_NOT_FOUND",
                    -6 => "MPV_ERROR_OPTION_FORMAT",
                    -7 => "MPV_ERROR_OPTION_ERROR",
                    -8 => "MPV_ERROR_PROPERTY_NOT_FOUND",
                    -9 => "MPV_ERROR_PROPERTY_FORMAT",
                    -10 => "MPV_ERROR_PROPERTY_UNAVAILABLE",
                    -11 => "MPV_ERROR_PROPERTY_ERROR",
                    -12 => "MPV_ERROR_COMMAND",
                    -13 => "MPV_ERROR_LOADING_FAILED",
                    -14 => "MPV_ERROR_AO_INIT_FAILED",
                    -15 => "MPV_ERROR_VO_INIT_FAILED",
                    -16 => "MPV_ERROR_NOTHING_TO_PLAY",
                    -17 => "MPV_ERROR_UNKNOWN_FORMAT",
                    -18 => "MPV_ERROR_UNSUPPORTED",
                    -19 => "MPV_ERROR_NOT_IMPLEMENTED",
                    -20 => "MPV_ERROR_GENERIC",
                    _ => "Unknown error",
                };
                error!(
                    "MPV render context creation failed: {} ({})",
                    error_msg, result
                );
                return Err(anyhow::anyhow!(
                    "Failed to create render context: {} ({})",
                    error_msg,
                    result
                ));
            }

            // Store the render context
            self.inner.mpv_gl.replace(Some(mpv_gl));

            // Set up the update callback with our custom context
            if !self.inner.update_callback_registered.get() {
                // Create update context that includes both GLArea and frame_pending flag
                struct UpdateContext {
                    gl_area: *const GLArea,
                    frame_pending: *const AtomicBool,
                }

                let update_ctx = Box::new(UpdateContext {
                    gl_area: gl_area as *const GLArea,
                    frame_pending: Arc::as_ptr(&self.inner.frame_pending),
                });

                mpv_render_context_set_update_callback(
                    mpv_gl,
                    Some(Self::on_mpv_render_update),
                    Box::into_raw(update_ctx) as *mut c_void,
                );
                self.inner.update_callback_registered.set(true);
            }

            info!("OpenGL render context initialized");

            // Load pending media if any - do this after a small delay to ensure context is ready
            if let Some(url) = self.inner.pending_media_url.borrow_mut().take() {
                debug!("Loading pending media: {}", url);
                let inner_clone = self.inner.clone();
                let url_clone = url.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(100), move || {
                    if let Some(ref mpv) = *inner_clone.mpv.borrow() {
                        debug!("Actually loading media now: {}", url_clone);
                        if let Err(e) = mpv.command("loadfile", &[&url_clone, "replace"]) {
                            error!("Failed to load pending media: {:?}", e);
                        }
                    }
                });
            }
        }

        Ok(())
    }

    pub fn create_video_widget(&self) -> gtk4::Widget {
        debug!("Creating GLArea for MPV rendering");

        let gl_area = GLArea::new();
        gl_area.set_vexpand(true);
        gl_area.set_hexpand(true);
        gl_area.set_can_focus(true);
        // Enable auto-render so GTK manages the framebuffer properly
        gl_area.set_auto_render(true);

        // Don't request a specific version - let GTK choose what's available
        // MPV should work with whatever GL context GTK provides

        // Clone inner for use in closures
        let inner = self.inner.clone();

        // Handle realize signal - initialize GL context
        let inner_realize = inner.clone();
        let player_self = self.clone();
        gl_area.connect_realize(move |gl_area| {
            debug!("GLArea realized - initializing MPV render context");

            // Make GL context current
            gl_area.make_current();

            // Initialize MPV if not done
            if inner_realize.mpv.borrow().is_none() {
                match MpvPlayerInner::init_mpv(&inner_realize) {
                    Ok(mpv) => {
                        // Apply initial upscaling mode
                        let initial_mode = *inner_realize.upscaling_mode.lock().unwrap();
                        player_self
                            .apply_upscaling_settings(&mpv, initial_mode)
                            .unwrap_or(());

                        inner_realize.mpv.replace(Some(mpv));
                    }
                    Err(e) => {
                        error!("Failed to initialize MPV: {}", e);
                        return;
                    }
                }
            }

            // Initialize render context
            if let Err(e) = player_self.init_gl_render_context(gl_area) {
                error!("Failed to initialize GL render context: {}", e);
            }
        });

        // Handle render signal - draw video frame
        let inner_render = inner.clone();
        gl_area.connect_render(move |gl_area, _gl_context| {
            // Reset frame pending flag
            inner_render.frame_pending.store(false, Ordering::Release);

            // Track render performance
            let now = Instant::now();
            let elapsed = now.duration_since(*inner_render.last_render_time.borrow());
            inner_render.last_render_time.replace(now);

            // Only log render performance in verbose mode
            if inner_render.verbose_logging {
                let render_count = inner_render.render_count.fetch_add(1, Ordering::Relaxed);
                if render_count % 60 == 0 {
                    let fps = if elapsed.as_millis() > 0 {
                        1000.0 / elapsed.as_millis() as f64
                    } else {
                        0.0
                    };
                    debug!(
                        "Render performance: {:.1} FPS ({}ms frame time)",
                        fps,
                        elapsed.as_millis()
                    );
                }
            } else {
                inner_render.render_count.fetch_add(1, Ordering::Relaxed);
            }

            // Render the current frame
            if let Some(mpv_gl) = &*inner_render.mpv_gl.borrow() {
                unsafe {
                    let (width, height) = (gl_area.width(), gl_area.height());

                    // Skip rendering if area has no size
                    if width <= 0 || height <= 0 {
                        return glib::Propagation::Stop;
                    }

                    // Check if MPV needs update
                    let update_flags = mpv_render_context_update(*mpv_gl);
                    if update_flags == 0 {
                        // No update needed, skip rendering
                        return glib::Propagation::Stop;
                    }

                    // Attach GTK's buffers before rendering
                    gl_area.attach_buffers();

                    // Get or cache the FBO
                    let fbo = if inner_render.cached_fbo.get() < 0 {
                        // Query FBO only once and cache it
                        static mut GL_GET_INTEGERV: Option<unsafe extern "C" fn(u32, *mut i32)> =
                            None;

                        let gl_integerv_ptr = &raw mut GL_GET_INTEGERV;
                        if (*gl_integerv_ptr).is_none() {
                            #[cfg(target_os = "macos")]
                            {
                                // On macOS, get glGetIntegerv directly
                                let gl_get_integerv = libc::dlsym(
                                    libc::RTLD_DEFAULT,
                                    b"glGetIntegerv\0".as_ptr() as *const i8,
                                );
                                if !gl_get_integerv.is_null() {
                                    *gl_integerv_ptr = Some(std::mem::transmute(gl_get_integerv));
                                }
                            }

                            #[cfg(not(target_os = "macos"))]
                            {
                                let egl_get_proc = libc::dlsym(
                                    libc::RTLD_DEFAULT,
                                    b"eglGetProcAddress\0".as_ptr() as *const i8,
                                );
                                if !egl_get_proc.is_null() {
                                    type EglGetProcFn =
                                        unsafe extern "C" fn(*const i8) -> *const c_void;
                                    let get_proc: EglGetProcFn = std::mem::transmute(egl_get_proc);
                                    let gl_get_integerv =
                                        get_proc(b"glGetIntegerv\0".as_ptr() as *const i8);
                                    if !gl_get_integerv.is_null() {
                                        *gl_integerv_ptr =
                                            Some(std::mem::transmute(gl_get_integerv));
                                    }
                                }
                            }
                        }

                        let mut current_fbo = 0i32;
                        if let Some(get_integerv) = *gl_integerv_ptr {
                            const GL_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
                            get_integerv(GL_FRAMEBUFFER_BINDING, &mut current_fbo);
                        }

                        inner_render.cached_fbo.set(current_fbo);
                        current_fbo
                    } else {
                        inner_render.cached_fbo.get()
                    };

                    let flip_y = 1i32; // GTK4 needs Y-flipping

                    let opengl_fbo = mpv_opengl_fbo {
                        fbo,
                        w: width,
                        h: height,
                        internal_format: 0,
                    };

                    let mut params = vec![
                        mpv_render_param {
                            type_: mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_FBO,
                            data: &opengl_fbo as *const _ as *mut c_void,
                        },
                        mpv_render_param {
                            type_: mpv_render_param_type_MPV_RENDER_PARAM_FLIP_Y,
                            data: &flip_y as *const _ as *mut c_void,
                        },
                        mpv_render_param {
                            type_: mpv_render_param_type_MPV_RENDER_PARAM_INVALID,
                            data: ptr::null_mut(),
                        },
                    ];

                    // Render the frame
                    let result = mpv_render_context_render(*mpv_gl, params.as_mut_ptr());
                    if result < 0 {
                        error!("mpv_render_context_render failed with error: {}", result);
                    } else {
                        // Only flush if we actually rendered something
                        static mut GL_FLUSH: Option<unsafe extern "C" fn()> = None;
                        let gl_flush_ptr = &raw mut GL_FLUSH;
                        if (*gl_flush_ptr).is_none() {
                            let gl_flush =
                                libc::dlsym(libc::RTLD_DEFAULT, b"glFlush\0".as_ptr() as *const i8);
                            if !gl_flush.is_null() {
                                *gl_flush_ptr = Some(std::mem::transmute(gl_flush));
                            }
                        }

                        if let Some(flush) = *gl_flush_ptr {
                            flush();
                        }

                        // Report that the frame was displayed
                        mpv_render_context_report_swap(*mpv_gl);
                    }
                }
            }

            // Return Proceed to let GTK finish the render
            glib::Propagation::Proceed
        });

        // Handle unrealize signal - cleanup
        let inner_unrealize = inner.clone();
        gl_area.connect_unrealize(move |_gl_area| {
            debug!("GLArea unrealized - cleaning up MPV render context");

            // Clean up render context
            if let Some(mpv_gl) = inner_unrealize.mpv_gl.borrow_mut().take() {
                unsafe {
                    mpv_render_context_free(mpv_gl);
                }
            }
        });

        // Store the GLArea
        self.inner.gl_area.replace(Some(gl_area.clone()));

        // Adaptive timer - only runs when playing and adjusts frequency based on content
        let gl_area_timer = gl_area.clone();
        let inner_timer = inner.clone();
        let timer_id = glib::timeout_add_local(Duration::from_millis(16), move || {
            // Only render if we have a context and video is playing
            if inner_timer.mpv_gl.borrow().is_some() {
                // Check if we're actually playing and not seeking
                if let Some(ref mpv) = *inner_timer.mpv.borrow() {
                    // Check if we're seeking - if so, skip automatic render
                    let is_seeking = inner_timer.seek_pending.lock().unwrap().is_some();
                    if !is_seeking
                        && let Ok(paused) = mpv.get_property::<bool>("pause")
                        && !paused
                    {
                        // Only queue render if no frame is pending
                        if !inner_timer.frame_pending.load(Ordering::Acquire) {
                            gl_area_timer.queue_render();
                        }
                    }
                }
            }
            glib::ControlFlow::Continue
        });

        // Store timer handle so we can cancel it on cleanup
        self.inner.timer_handle.replace(Some(timer_id));

        debug!("GLArea created with adaptive rendering");
        gl_area.upcast::<gtk4::Widget>()
    }

    pub async fn load_media(&self, url: &str, _video_sink: Option<()>) -> Result<()> {
        info!("Loading media: {}", url);

        // Update state
        {
            let mut state = self.inner.state.write().await;
            *state = PlayerState::Loading;
        }

        // Check if render context is initialized
        if self.inner.mpv_gl.borrow().is_none() {
            warn!(
                "MpvPlayer::load_media() - Render context not initialized yet, deferring media load"
            );
            // Store the URL to load later when render context is ready
            self.inner.pending_media_url.replace(Some(url.to_string()));
            return Ok(());
        }

        // Initialize MPV if not already done
        if self.inner.mpv.borrow().is_none() {
            let mpv = MpvPlayerInner::init_mpv(&self.inner)?;
            self.inner.mpv.replace(Some(mpv));
        }

        // Load the media file
        if let Some(ref mpv) = *self.inner.mpv.borrow() {
            mpv.command("loadfile", &[url, "replace"])
                .map_err(|e| anyhow::anyhow!("Failed to load media: {:?}", e))?;
            debug!("Media loaded successfully");
        } else {
            return Err(anyhow::anyhow!("MPV not initialized"));
        }

        Ok(())
    }

    pub async fn play(&self) -> Result<()> {
        debug!("Starting playback");

        if let Some(ref mpv) = *self.inner.mpv.borrow() {
            mpv.set_property("pause", false)
                .map_err(|e| anyhow::anyhow!("Failed to set pause=false: {:?}", e))?;

            let mut state = self.inner.state.write().await;
            *state = PlayerState::Playing;
            debug!("Playback started");
        } else {
            // If MPV not initialized yet, just update state - it will auto-play when loaded
            warn!("MpvPlayer::play() - MPV not initialized yet, will auto-play when ready");
            let mut state = self.inner.state.write().await;
            *state = PlayerState::Playing;
        }

        Ok(())
    }

    pub async fn pause(&self) -> Result<()> {
        debug!("MpvPlayer::pause() - Pausing playback");

        if let Some(ref mpv) = *self.inner.mpv.borrow() {
            mpv.set_property("pause", true)
                .map_err(|e| anyhow::anyhow!("Failed to set pause=true: {:?}", e))?;

            let mut state = self.inner.state.write().await;
            *state = PlayerState::Paused;
        }
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        debug!("MpvPlayer::stop() - Stopping playback");

        if let Some(ref mpv) = *self.inner.mpv.borrow() {
            mpv.command("stop", &[])
                .map_err(|e| anyhow::anyhow!("Failed to stop: {:?}", e))?;

            let mut state = self.inner.state.write().await;
            *state = PlayerState::Stopped;
        }
        Ok(())
    }

    pub async fn seek(&self, position: Duration) -> Result<()> {
        debug!("MpvPlayer::seek() - Seeking to {:?}", position);

        let position_secs = position.as_secs_f64();

        // Update the last seek target for position tracking
        {
            let mut last_target = self.inner.last_seek_target.lock().unwrap();
            *last_target = Some(position_secs);
        }

        // Store the pending seek position
        {
            let mut pending = self.inner.seek_pending.lock().unwrap();
            *pending = Some((position_secs, Instant::now()));
        }

        // Cancel any existing seek timer
        if let Some(timer) = self.inner.seek_timer.borrow_mut().take() {
            timer.remove();
        }

        // Clone references needed for the closure
        let inner = self.inner.clone();
        let seek_pending = self.inner.seek_pending.clone();
        let last_seek_target = self.inner.last_seek_target.clone();

        // Very short delay for coalescing (5ms)
        let timer_id = glib::timeout_add_local_once(Duration::from_millis(5), move || {
            // Get the latest seek position
            let seek_pos = {
                let mut pending = seek_pending.lock().unwrap();
                pending.take()
            };

            if let Some((pos, _timestamp)) = seek_pos
                && let Some(ref mpv) = *inner.mpv.borrow()
            {
                // Use keyframe seeking for speed
                if let Err(e) = mpv.command("seek", &[&pos.to_string(), "absolute"]) {
                    error!("Failed to seek: {:?}", e);
                    // Clear last seek target on error
                    let mut last_target = last_seek_target.lock().unwrap();
                    *last_target = None;
                } else {
                    // Force a frame update after seeking
                    if let Some(gl_area) = &*inner.gl_area.borrow() {
                        gl_area.queue_render();
                    }
                }
            }

            // Clear the timer reference
            inner.seek_timer.replace(None);
        });

        self.inner.seek_timer.replace(Some(timer_id));
        Ok(())
    }

    pub async fn get_position(&self) -> Option<Duration> {
        // If we have a pending seek, return that as the effective position
        {
            let last_target = self.inner.last_seek_target.lock().unwrap();
            if let Some(target_pos) = *last_target {
                // Check if the seek is recent (within 100ms)
                if let Some((_, timestamp)) = *self.inner.seek_pending.lock().unwrap()
                    && timestamp.elapsed() < Duration::from_millis(100)
                {
                    return Some(Duration::from_secs_f64(target_pos));
                }
            }
        }

        // Otherwise return the actual position
        if let Some(ref mpv) = *self.inner.mpv.borrow()
            && let Ok(pos) = mpv.get_property::<f64>("time-pos")
        {
            // Clear the last seek target since we're at the actual position now
            let mut last_target = self.inner.last_seek_target.lock().unwrap();
            *last_target = None;
            return Some(Duration::from_secs_f64(pos));
        }
        None
    }

    pub async fn get_duration(&self) -> Option<Duration> {
        if let Some(ref mpv) = *self.inner.mpv.borrow()
            && let Ok(dur) = mpv.get_property::<f64>("duration")
        {
            return Some(Duration::from_secs_f64(dur));
        }
        None
    }

    pub async fn set_volume(&self, volume: f64) -> Result<()> {
        if let Some(ref mpv) = *self.inner.mpv.borrow() {
            // MPV expects volume in 0-100 range
            let mpv_volume = (volume * 100.0).clamp(0.0, 100.0);
            mpv.set_property("volume", mpv_volume)
                .map_err(|e| anyhow::anyhow!("Failed to set volume: {:?}", e))?;
        }
        Ok(())
    }

    pub fn get_video_widget(&self) -> Option<gtk4::Widget> {
        self.inner
            .gl_area
            .borrow()
            .as_ref()
            .map(|area| area.clone().upcast())
    }

    pub async fn get_video_dimensions(&self) -> Option<(i32, i32)> {
        if let Some(ref mpv) = *self.inner.mpv.borrow()
            && let (Ok(width), Ok(height)) = (
                mpv.get_property::<i64>("width"),
                mpv.get_property::<i64>("height"),
            )
        {
            return Some((width as i32, height as i32));
        }
        None
    }

    pub async fn get_state(&self) -> PlayerState {
        self.inner.state.read().await.clone()
    }

    pub async fn get_audio_tracks(&self) -> Vec<(i32, String)> {
        let mut tracks = Vec::new();

        if let Some(ref mpv) = *self.inner.mpv.borrow()
            && let Ok(count) = mpv.get_property::<i64>("track-list/count")
        {
            debug!("mpv: track-list/count={}", count);
            for i in 0..count {
                let type_key = format!("track-list/{}/type", i);
                match mpv.get_property::<String>(&type_key) {
                    Ok(track_type) => {
                        debug!("mpv: track {} type={} (expect audio)", i, track_type);
                        if track_type != "audio" {
                            continue;
                        }
                    }
                    Err(e) => {
                        debug!("mpv: failed to get {}: {:?}", type_key, e);
                        continue;
                    }
                }
                let id_key = format!("track-list/{}/id", i);
                let title_key = format!("track-list/{}/title", i);
                let lang_key = format!("track-list/{}/lang", i);

                if let Ok(id) = mpv.get_property::<i64>(&id_key) {
                    let mut title = format!("Audio Track {}", id);

                    if let Ok(track_title) = mpv.get_property::<String>(&title_key) {
                        title = track_title;
                    } else if let Ok(lang) = mpv.get_property::<String>(&lang_key) {
                        title = format!("Audio Track {} ({})", id, lang);
                    }

                    debug!("mpv: audio id={} title={}", id, title);
                    tracks.push((id as i32, title));
                }
            }
        }

        tracks
    }

    pub async fn get_subtitle_tracks(&self) -> Vec<(i32, String)> {
        let mut tracks = Vec::new();

        // Add "None" option
        tracks.push((-1, "None".to_string()));

        if let Some(ref mpv) = *self.inner.mpv.borrow()
            && let Ok(count) = mpv.get_property::<i64>("track-list/count")
        {
            debug!("mpv: track-list/count={}", count);
            for i in 0..count {
                let type_key = format!("track-list/{}/type", i);
                match mpv.get_property::<String>(&type_key) {
                    Ok(track_type) => {
                        debug!("mpv: track {} type={} (expect sub)", i, track_type);
                        if track_type != "sub" {
                            continue;
                        }
                    }
                    Err(e) => {
                        debug!("mpv: failed to get {}: {:?}", type_key, e);
                        continue;
                    }
                }
                let id_key = format!("track-list/{}/id", i);
                let title_key = format!("track-list/{}/title", i);
                let lang_key = format!("track-list/{}/lang", i);

                if let Ok(id) = mpv.get_property::<i64>(&id_key) {
                    let mut title = format!("Subtitle {}", id);

                    if let Ok(track_title) = mpv.get_property::<String>(&title_key) {
                        title = track_title;
                    } else if let Ok(lang) = mpv.get_property::<String>(&lang_key) {
                        title = format!("Subtitle {} ({})", id, lang);
                    }

                    debug!("mpv: subtitle id={} title={}", id, title);
                    tracks.push((id as i32, title));
                }
            }
        }

        tracks
    }

    pub async fn set_audio_track(&self, track_index: i32) -> Result<()> {
        if let Some(ref mpv) = *self.inner.mpv.borrow() {
            mpv.set_property("aid", track_index as i64)
                .map_err(|e| anyhow::anyhow!("Failed to set audio track: {:?}", e))?;
            debug!("Set audio track to {}", track_index);
        }
        Ok(())
    }

    pub async fn set_subtitle_track(&self, track_index: i32) -> Result<()> {
        if let Some(ref mpv) = *self.inner.mpv.borrow() {
            if track_index < 0 {
                // Disable subtitles
                mpv.set_property("sid", "no")
                    .map_err(|e| anyhow::anyhow!("Failed to disable subtitles: {:?}", e))?;
                debug!("Disabled subtitles");
            } else {
                // Enable subtitles and set track
                mpv.set_property("sid", track_index as i64)
                    .map_err(|e| anyhow::anyhow!("Failed to set subtitle track: {:?}", e))?;
                debug!("Set subtitle track to {}", track_index);
            }
        }
        Ok(())
    }

    pub async fn get_current_audio_track(&self) -> i32 {
        if let Some(ref mpv) = *self.inner.mpv.borrow()
            && let Ok(aid) = mpv.get_property::<i64>("aid")
        {
            return aid as i32;
        }
        -1
    }

    pub async fn get_current_subtitle_track(&self) -> i32 {
        if let Some(ref mpv) = *self.inner.mpv.borrow()
            && let Ok(sid) = mpv.get_property::<i64>("sid")
        {
            return sid as i32;
        }
        -1
    }

    pub async fn get_buffer_percentage(&self) -> Option<f64> {
        // MPV maintains a fixed ~10 second buffer, which isn't useful to display
        // This method is kept for compatibility but returns None
        None
    }

    pub async fn set_upscaling_mode(&self, mode: UpscalingMode) -> Result<()> {
        let mut current_mode = self.inner.upscaling_mode.lock().unwrap();
        *current_mode = mode;
        drop(current_mode);

        if let Some(ref mpv) = *self.inner.mpv.borrow() {
            self.apply_upscaling_settings(mpv, mode)?;
        }
        Ok(())
    }

    pub async fn get_upscaling_mode(&self) -> UpscalingMode {
        *self.inner.upscaling_mode.lock().unwrap()
    }

    pub async fn cycle_upscaling_mode(&self) -> Result<UpscalingMode> {
        let current = self.get_upscaling_mode().await;
        let next = current.next();
        self.set_upscaling_mode(next).await?;
        Ok(next)
    }

    fn apply_upscaling_settings(&self, mpv: &Mpv, mode: UpscalingMode) -> Result<()> {
        // Clear any existing shaders first
        mpv.set_property("glsl-shaders", "").unwrap_or(());

        // Get the shader directory path
        let shader_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .map(|p| p.join("../share/reel/shaders"))
            .or_else(|| {
                // Fallback to development path
                std::env::current_dir()
                    .ok()
                    .map(|p| p.join("assets/shaders"))
            })
            .unwrap_or_else(|| std::path::PathBuf::from("assets/shaders"));

        match mode {
            UpscalingMode::None => {
                // Use basic bilinear scaling
                mpv.set_property("scale", "bilinear").unwrap_or(());
                mpv.set_property("cscale", "bilinear").unwrap_or(());
                mpv.set_property("dscale", "bilinear").unwrap_or(());
                mpv.set_property("sigmoid-upscaling", false).unwrap_or(());
                mpv.set_property("deband", false).unwrap_or(());
                debug!("Upscaling disabled - using bilinear");
            }
            UpscalingMode::HighQuality => {
                // Use FSRCNNX shader for high quality upscaling
                let shader_path = shader_dir.join("FSRCNNX_x2_8-0-4-1.glsl");
                if shader_path.exists() {
                    mpv.set_property("glsl-shaders", shader_path.to_str().unwrap_or(""))
                        .unwrap_or(());
                    debug!("High quality upscaling enabled with FSRCNNX shader");
                } else {
                    // Fallback to built-in high quality scalers
                    mpv.set_property("scale", "ewa_lanczossharp").unwrap_or(());
                    mpv.set_property("cscale", "ewa_lanczossharp").unwrap_or(());
                    mpv.set_property("dscale", "mitchell").unwrap_or(());
                    mpv.set_property("sigmoid-upscaling", true).unwrap_or(());
                    mpv.set_property("deband", true).unwrap_or(());
                    mpv.set_property("deband-iterations", 2).unwrap_or(());
                    mpv.set_property("deband-threshold", 48).unwrap_or(());
                    mpv.set_property("deband-range", 16).unwrap_or(());
                    mpv.set_property("deband-grain", 24).unwrap_or(());
                    debug!(
                        "High quality upscaling enabled with built-in scalers (shader not found)"
                    );
                }
            }
            UpscalingMode::FSR => {
                // AMD FSR shader
                let shader_path = shader_dir.join("FSR.glsl");
                if shader_path.exists() {
                    mpv.set_property("glsl-shaders", shader_path.to_str().unwrap_or(""))
                        .unwrap_or(());
                    debug!("FSR upscaling enabled with shader");
                } else {
                    // Fallback to spline36
                    mpv.set_property("scale", "spline36").unwrap_or(());
                    mpv.set_property("cscale", "spline36").unwrap_or(());
                    mpv.set_property("dscale", "mitchell").unwrap_or(());
                    mpv.set_property("sigmoid-upscaling", true).unwrap_or(());
                    mpv.set_property("deband", true).unwrap_or(());
                    debug!("FSR upscaling mode set with fallback (shader not found)");
                }
            }
            UpscalingMode::Anime => {
                // Anime4K shaders - combine Clamp Highlights and Upscale
                let clamp_path = shader_dir.join("Anime4K_Clamp_Highlights.glsl");
                let upscale_path = shader_dir.join("Anime4K_Upscale_CNN_x2_M.glsl");

                if clamp_path.exists() && upscale_path.exists() {
                    // Use both shaders in sequence for best results
                    let shader_list = format!(
                        "{}:{}",
                        clamp_path.to_str().unwrap_or(""),
                        upscale_path.to_str().unwrap_or("")
                    );
                    mpv.set_property("glsl-shaders", shader_list.as_str())
                        .unwrap_or(());
                    debug!("Anime upscaling enabled with Anime4K shaders");
                } else if upscale_path.exists() {
                    // Use only upscale if clamp is missing
                    mpv.set_property("glsl-shaders", upscale_path.to_str().unwrap_or(""))
                        .unwrap_or(());
                    debug!("Anime upscaling enabled with Anime4K upscale only");
                } else {
                    // Fallback to optimized built-in settings for anime
                    mpv.set_property("scale", "ewa_lanczossharp").unwrap_or(());
                    mpv.set_property("cscale", "ewa_lanczossoft").unwrap_or(());
                    mpv.set_property("dscale", "mitchell").unwrap_or(());
                    mpv.set_property("sigmoid-upscaling", false).unwrap_or(());
                    mpv.set_property("deband", true).unwrap_or(());
                    mpv.set_property("deband-iterations", 4).unwrap_or(());
                    mpv.set_property("deband-threshold", 64).unwrap_or(());
                    mpv.set_property("deband-range", 16).unwrap_or(());
                    mpv.set_property("deband-grain", 48).unwrap_or(());
                    debug!("Anime upscaling mode set with fallback (shaders not found)");
                }
            }
        }

        info!("Upscaling mode changed to: {}", mode.to_string());
        Ok(())
    }
}

impl MpvPlayerInner {
    fn init_mpv(&self) -> Result<Mpv> {
        info!("Creating MPV instance");

        // MPV requires LC_NUMERIC to be set to "C"
        unsafe {
            let c_locale = CString::new("C").unwrap();
            libc::setlocale(libc::LC_NUMERIC, c_locale.as_ptr());
        }

        let mpv =
            Mpv::new().map_err(|e| anyhow::anyhow!("Failed to create MPV instance: {:?}", e))?;

        // Enable terminal output so MPV can log messages
        mpv.set_property("terminal", true)
            .map_err(|e| anyhow::anyhow!("Failed to enable terminal: {:?}", e))?;

        // Set log level - this needs to be set before other properties
        if self.verbose_logging {
            let _ = mpv.set_property("msg-level", "all=debug");
        } else {
            let _ = mpv.set_property("msg-level", "all=info");
        }

        // Always log MPV version as it's useful for debugging
        if let Ok(version) = mpv.get_property::<String>("mpv-version") {
            info!("MPV version: {}", version);
        }
        // Only log configuration in verbose mode
        if self.verbose_logging
            && let Ok(config) = mpv.get_property::<String>("mpv-configuration")
        {
            debug!("MPV configuration: {}", config);
        }

        // Configure MPV for render API with performance optimizations
        mpv.set_property("vo", "libmpv")
            .map_err(|e| anyhow::anyhow!("Failed to set vo=libmpv: {:?}", e))?;

        // Only enable GPU debug options if verbose logging is enabled
        if self.verbose_logging {
            let _ = mpv.set_property("gpu-debug", true);
            let _ = mpv.set_property("opengl-debug", true);
        }

        // Performance optimizations - improved for seeking
        mpv.set_property("video-sync", "audio")
            .map_err(|e| anyhow::anyhow!("Failed to set video-sync: {:?}", e))?;
        mpv.set_property("interpolation", false)
            .map_err(|e| anyhow::anyhow!("Failed to set interpolation: {:?}", e))?;
        mpv.set_property("opengl-swapinterval", 1).unwrap_or(()); // May not be available on all systems

        // Seek optimization settings - prioritize speed
        mpv.set_property("hr-seek", "no") // Disable HR seeking for speed
            .map_err(|e| anyhow::anyhow!("Failed to set hr-seek: {:?}", e))?;
        mpv.set_property("hr-seek-framedrop", true) // Allow frame drops for smoother seeking
            .map_err(|e| anyhow::anyhow!("Failed to set hr-seek-framedrop: {:?}", e))?;
        mpv.set_property("index", "default") // Use index for faster seeking
            .map_err(|e| anyhow::anyhow!("Failed to set index: {:?}", e))?;
        mpv.set_property("force-seekable", true) // Force seekable even for streams
            .unwrap_or(());

        // Set basic options
        mpv.set_property("keep-open", "yes")
            .map_err(|e| anyhow::anyhow!("Failed to set keep-open: {:?}", e))?;
        mpv.set_property("hwdec", "auto-safe") // auto-safe is more stable than auto
            .map_err(|e| anyhow::anyhow!("Failed to set hwdec: {:?}", e))?;
        mpv.set_property("input-default-bindings", false)
            .map_err(|e| anyhow::anyhow!("Failed to set input-default-bindings: {:?}", e))?;
        mpv.set_property("input-vo-keyboard", false)
            .map_err(|e| anyhow::anyhow!("Failed to set input-vo-keyboard: {:?}", e))?;
        mpv.set_property("osc", false)
            .map_err(|e| anyhow::anyhow!("Failed to set osc: {:?}", e))?;
        mpv.set_property("ytdl", false)
            .map_err(|e| anyhow::anyhow!("Failed to set ytdl: {:?}", e))?;
        mpv.set_property("load-scripts", false)
            .map_err(|e| anyhow::anyhow!("Failed to set load-scripts: {:?}", e))?;

        // Audio/subtitle preferences
        mpv.set_property("aid", "auto")
            .map_err(|e| anyhow::anyhow!("Failed to set aid: {:?}", e))?;
        mpv.set_property("sid", "auto")
            .map_err(|e| anyhow::anyhow!("Failed to set sid: {:?}", e))?;
        mpv.set_property("alang", "eng,en")
            .map_err(|e| anyhow::anyhow!("Failed to set alang: {:?}", e))?;
        mpv.set_property("slang", "eng,en")
            .map_err(|e| anyhow::anyhow!("Failed to set slang: {:?}", e))?;
        mpv.set_property("sub-auto", "fuzzy")
            .map_err(|e| anyhow::anyhow!("Failed to set sub-auto: {:?}", e))?;
        mpv.set_property("audio-file-auto", "fuzzy")
            .map_err(|e| anyhow::anyhow!("Failed to set audio-file-auto: {:?}", e))?;

        // Aggressive cache settings - cache entire episodes when possible (configurable)
        mpv.set_property("cache", true)
            .map_err(|e| anyhow::anyhow!("Failed to set cache: {:?}", e))?;
        mpv.set_property("cache-secs", self.cache_secs as i64)
            .map_err(|e| anyhow::anyhow!("Failed to set cache-secs: {:?}", e))?;
        mpv.set_property("cache-pause-initial", false) // Don't pause initially for cache
            .map_err(|e| anyhow::anyhow!("Failed to set cache-pause-initial: {:?}", e))?;
        mpv.set_property("cache-pause-wait", 0.1) // Resume very quickly after cache runs out
            .map_err(|e| anyhow::anyhow!("Failed to set cache-pause-wait: {:?}", e))?;

        // Configurable cache sizes
        let cache_size = format!("{}MiB", self.cache_size_mb);
        let backbuffer_size = format!("{}MiB", self.cache_backbuffer_mb);

        mpv.set_property("demuxer-max-bytes", cache_size.as_str())
            .map_err(|e| anyhow::anyhow!("Failed to set demuxer-max-bytes: {:?}", e))?;
        mpv.set_property("demuxer-max-back-bytes", backbuffer_size.as_str())
            .map_err(|e| anyhow::anyhow!("Failed to set demuxer-max-back-bytes: {:?}", e))?;
        mpv.set_property("demuxer-readahead-secs", self.cache_secs as i64)
            .map_err(|e| anyhow::anyhow!("Failed to set demuxer-readahead-secs: {:?}", e))?;

        info!(
            "MPV cache configured: {}MB forward, {}MB backward, up to {} minutes buffering",
            self.cache_size_mb,
            self.cache_backbuffer_mb,
            self.cache_secs / 60
        );

        // Additional cache optimizations
        mpv.set_property("demuxer-seekable-cache", true) // Enable seekable cache
            .map_err(|e| anyhow::anyhow!("Failed to set demuxer-seekable-cache: {:?}", e))?;
        mpv.set_property("cache-on-disk", false) // Keep cache in RAM for speed
            .map_err(|e| anyhow::anyhow!("Failed to set cache-on-disk: {:?}", e))?;
        mpv.set_property("demuxer-donate-buffer", true) // Allow demuxer to use all available cache
            .unwrap_or(());
        mpv.set_property("stream-buffer-size", "512KiB") // Larger network buffer
            .unwrap_or(());

        // Disable OSD
        mpv.set_property("osd-level", 0i64)
            .map_err(|e| anyhow::anyhow!("Failed to set osd-level: {:?}", e))?;

        info!("MPV instance configured");
        Ok(mpv)
    }
}

impl Drop for MpvPlayerInner {
    fn drop(&mut self) {
        // Cancel the timer if it's running
        if let Some(timer_id) = self.timer_handle.borrow_mut().take() {
            timer_id.remove();
        }

        // Cancel the seek timer if it's running
        if let Some(seek_timer) = self.seek_timer.borrow_mut().take() {
            seek_timer.remove();
        }

        // Clean up render context
        if let Some(mpv_gl) = self.mpv_gl.borrow_mut().take() {
            unsafe {
                mpv_render_context_free(mpv_gl);
            }
        }
    }
}
