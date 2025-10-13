use crate::config::Config;
use anyhow::Result;
use gtk4::GLArea;
use gtk4::{self, glib, prelude::*};
use libmpv2::Mpv;
use libmpv2_sys::*;
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_void};
use std::ptr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

// MPV render update flags

// Wrapper for mpv_render_context pointer to make it Send/Sync
// Safety: MPV render context is thread-safe when properly synchronized
struct MpvRenderContextPtr(*mut mpv_render_context);
unsafe impl Send for MpvRenderContextPtr {}
unsafe impl Sync for MpvRenderContextPtr {}

// Cached OpenGL function pointers for thread-safe access
struct OpenGLFunctions {
    get_proc_address: unsafe extern "C" fn(*const i8) -> *mut c_void,
}

unsafe impl Send for OpenGLFunctions {}
unsafe impl Sync for OpenGLFunctions {}

static GL_GET_INTEGERV_FN: OnceLock<Option<unsafe extern "C" fn(u32, *mut i32)>> = OnceLock::new();
static GL_VIEWPORT_FN: OnceLock<Option<unsafe extern "C" fn(i32, i32, i32, i32)>> = OnceLock::new();
static GL_FLUSH_FN: OnceLock<Option<unsafe extern "C" fn()>> = OnceLock::new();

#[derive(Debug, Clone)]
pub enum PlayerState {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopped,
    Error,
}

use super::types::{UpscalingMode, ZoomMode};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[tokio::test]
    async fn test_audio_track_enumeration() {
        // Create player instance
        let config = Config::default();
        let player = MpvPlayer::new(&config).expect("Failed to create MpvPlayer");

        // Since we can't actually load media in unit tests without a real file,
        // we'll just verify the methods exist and return expected empty results
        let audio_tracks = player.get_audio_tracks().await;
        assert_eq!(
            audio_tracks.len(),
            0,
            "Should return empty list when no media is loaded"
        );
    }

    #[tokio::test]
    async fn test_subtitle_track_enumeration() {
        let config = Config::default();
        let player = MpvPlayer::new(&config).expect("Failed to create MpvPlayer");

        // Subtitle tracks should always have at least "None" option
        let subtitle_tracks = player.get_subtitle_tracks().await;
        assert_eq!(
            subtitle_tracks.len(),
            1,
            "Should have 'None' option even when no media is loaded"
        );
        assert_eq!(subtitle_tracks[0], (-1, "None".to_string()));
    }

    #[tokio::test]
    async fn test_set_audio_track() {
        let config = Config::default();
        let player = MpvPlayer::new(&config).expect("Failed to create MpvPlayer");

        // Setting track when no media is loaded should succeed (MPV handles it gracefully)
        let result = player.set_audio_track(0).await;
        assert!(result.is_ok(), "Setting audio track should not fail");
    }

    #[tokio::test]
    async fn test_set_subtitle_track() {
        let config = Config::default();
        let player = MpvPlayer::new(&config).expect("Failed to create MpvPlayer");

        // Test disabling subtitles
        let result = player.set_subtitle_track(-1).await;
        assert!(result.is_ok(), "Disabling subtitles should not fail");

        // Test setting a specific track
        let result = player.set_subtitle_track(0).await;
        assert!(result.is_ok(), "Setting subtitle track should not fail");
    }

    #[tokio::test]
    async fn test_get_current_tracks() {
        let config = Config::default();
        let player = MpvPlayer::new(&config).expect("Failed to create MpvPlayer");

        // Test getting current track when no media is loaded
        let current_audio = player.get_current_audio_track().await;
        assert_eq!(
            current_audio, -1,
            "Should return -1 when no audio track is set"
        );

        let current_subtitle = player.get_current_subtitle_track().await;
        assert_eq!(
            current_subtitle, -1,
            "Should return -1 when no subtitle track is set"
        );
    }
}

struct MpvPlayerInner {
    mpv: Arc<Mutex<Option<Mpv>>>,
    mpv_gl: Arc<Mutex<Option<MpvRenderContextPtr>>>,
    gl_functions: Arc<Mutex<Option<OpenGLFunctions>>>,
    state: Arc<RwLock<PlayerState>>,
    update_callback_registered: Arc<Mutex<bool>>,
    pending_media_url: Arc<Mutex<Option<String>>>,
    last_render_time: Arc<Mutex<Instant>>,
    render_count: Arc<AtomicU64>,
    cached_fbo: Arc<Mutex<i32>>,
    timer_handle: Arc<Mutex<Option<glib::SourceId>>>,
    verbose_logging: bool,
    cache_size_mb: u32,
    cache_backbuffer_mb: u32,
    cache_secs: u32,
    seek_pending: Arc<Mutex<Option<(f64, Instant)>>>,
    seek_timer: Arc<Mutex<Option<glib::SourceId>>>,
    last_seek_target: Arc<Mutex<Option<f64>>>,
    upscaling_mode: Arc<Mutex<UpscalingMode>>,
    zoom_mode: Arc<Mutex<ZoomMode>>,
    error_callback: Arc<Mutex<Option<Box<dyn Fn(String) + Send + 'static>>>>,
    event_monitor_handle: Arc<Mutex<Option<glib::SourceId>>>,
    gl_area_realized: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Clone)]
pub struct MpvPlayer {
    inner: Arc<MpvPlayerInner>,
}

// MpvPlayer is now automatically Send + Sync because all its fields are

impl MpvPlayer {
    /// Set a callback to be called when errors occur
    pub fn set_error_callback<F>(&self, callback: F)
    where
        F: Fn(String) + Send + 'static,
    {
        *self.inner.error_callback.lock().unwrap() = Some(Box::new(callback));
    }

    /// Start monitoring MPV events for errors
    fn start_event_monitoring(&self) {
        let inner = self.inner.clone();

        // Cancel any existing monitor
        if let Some(handle) = self.inner.event_monitor_handle.lock().unwrap().take() {
            handle.remove();
        }

        // Start a new monitoring task
        let handle = glib::timeout_add_local(Duration::from_millis(100), move || {
            if let Some(ref mpv) = *inner.mpv.lock().unwrap() {
                // Check if media failed to load by checking idle state
                if let Ok(idle) = mpv.get_property::<bool>("idle-active") {
                    if idle {
                        // Check if we were trying to play something
                        if let Ok(path) = mpv.get_property::<String>("path") {
                            if !path.is_empty() {
                                // We have a path but are idle - this indicates an error
                                if let Some(ref callback) = *inner.error_callback.lock().unwrap() {
                                    callback(
                                        "Media playback failed - player became idle unexpectedly"
                                            .to_string(),
                                    );
                                }
                                // Set state to error
                                if let Ok(mut state) = inner.state.try_write() {
                                    *state = PlayerState::Error;
                                }
                            }
                        }
                    }
                }

                // Check for EOF which can indicate errors
                if let Ok(eof) = mpv.get_property::<bool>("eof-reached") {
                    if eof {
                        // Check if we have a valid duration - if not, it's likely an error
                        if let Ok(duration) = mpv.get_property::<f64>("duration") {
                            if duration <= 0.0 {
                                if let Some(ref callback) = *inner.error_callback.lock().unwrap() {
                                    callback(
                                        "Media failed to load - no valid duration".to_string(),
                                    );
                                }
                                // Set state to error
                                if let Ok(mut state) = inner.state.try_write() {
                                    *state = PlayerState::Error;
                                }
                            }
                        }
                    }
                }

                // Check demuxer cache state for errors
                if let Ok(cache_state) = mpv.get_property::<i64>("demuxer-cache-state") {
                    if cache_state < 0 {
                        // Negative values often indicate errors
                        if let Some(ref callback) = *inner.error_callback.lock().unwrap() {
                            callback(
                                "Media streaming error - cache state indicates failure".to_string(),
                            );
                        }
                    }
                }
            }
            glib::ControlFlow::Continue
        });

        *self.inner.event_monitor_handle.lock().unwrap() = Some(handle);
    }

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
            inner: Arc::new(MpvPlayerInner {
                mpv: Arc::new(Mutex::new(None)),
                mpv_gl: Arc::new(Mutex::new(None)),
                gl_functions: Arc::new(Mutex::new(None)),
                state: Arc::new(RwLock::new(PlayerState::Idle)),
                update_callback_registered: Arc::new(Mutex::new(false)),
                pending_media_url: Arc::new(Mutex::new(None)),
                last_render_time: Arc::new(Mutex::new(Instant::now())),
                render_count: Arc::new(AtomicU64::new(0)),
                cached_fbo: Arc::new(Mutex::new(-1)),
                timer_handle: Arc::new(Mutex::new(None)),
                verbose_logging,
                cache_size_mb,
                cache_backbuffer_mb,
                cache_secs,
                seek_pending: Arc::new(Mutex::new(None)),
                seek_timer: Arc::new(Mutex::new(None)),
                last_seek_target: Arc::new(Mutex::new(None)),
                upscaling_mode: Arc::new(Mutex::new(UpscalingMode::None)),
                zoom_mode: Arc::new(Mutex::new(ZoomMode::default())),
                error_callback: Arc::new(Mutex::new(None)),
                event_monitor_handle: Arc::new(Mutex::new(None)),
                gl_area_realized: Arc::new(AtomicBool::new(false)),
            }),
        })
    }

    fn load_gl_function_ptr(name: &str) -> Option<*mut c_void> {
        let cname = CString::new(name).ok()?;
        let ptr = unsafe { Self::get_proc_address_cached(ptr::null_mut(), cname.as_ptr()) };
        (!ptr.is_null()).then_some(ptr)
    }

    // Thread-safe proc address function that doesn't access GLArea
    unsafe extern "C" fn get_proc_address_cached(
        _ctx: *mut c_void,
        name: *const libc::c_char,
    ) -> *mut c_void {
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
                        b"eglGetProcAddress\0".as_ptr() as *const libc::c_char,
                    ));
                }
            }

            let name_str = CStr::from_ptr(name as *const libc::c_char)
                .to_string_lossy()
                .to_string();

            // Check cache first - use raw pointer to avoid reference issues
            let cache_ptr = &raw mut PROC_CACHE;
            if let Some(cache) = &mut *cache_ptr
                && let Some(&cached_proc) = cache.get(&name_str)
            {
                return cached_proc;
            }

            // NOTE: We don't access GLArea here for thread safety.
            // The GL context should already be current when this is called from MPV render thread

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
                    type EglGetProcFn = unsafe extern "C" fn(*const libc::c_char) -> *mut c_void;
                    let get_proc: EglGetProcFn = std::mem::transmute(egl_get_proc);
                    func = get_proc(name as *const libc::c_char);
                }

                // Fallback to dlsym if needed
                if func.is_null() {
                    func = libc::dlsym(libc::RTLD_DEFAULT, name as *const libc::c_char);
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
            // Use a simple struct that just marks frame as pending
            struct UpdateContext {}

            let _update_ctx = &*(ctx as *const UpdateContext);
            // Frame update handling removed
            // frame_pending.store(true, Ordering::Release); // Removed unused field
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

        let mpv = self.inner.mpv.lock().unwrap();
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

            // Log API version first
            debug!("Setting up MPV render API with type: opengl");

            // Use the cached proc address function that doesn't need GLArea
            let opengl_params = mpv_opengl_init_params {
                get_proc_address: Some(Self::get_proc_address_cached),
                get_proc_address_ctx: ptr::null_mut(), // No context needed for cached version
            };

            // macOS-specific: Add advanced control flag for better compatibility
            #[cfg(target_os = "macos")]
            let advanced_control = 1i32;

            let mut params = vec![
                mpv_render_param {
                    type_: mpv_render_param_type_MPV_RENDER_PARAM_API_TYPE,
                    data: api_type.as_ptr() as *mut c_void,
                },
                mpv_render_param {
                    type_: mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                    data: &opengl_params as *const _ as *mut c_void,
                },
            ];

            // macOS: Add advanced control parameter for better GL handling
            #[cfg(target_os = "macos")]
            params.push(mpv_render_param {
                type_: mpv_render_param_type_MPV_RENDER_PARAM_ADVANCED_CONTROL,
                data: &advanced_control as *const _ as *mut c_void,
            });

            params.push(mpv_render_param {
                type_: mpv_render_param_type_MPV_RENDER_PARAM_INVALID,
                data: ptr::null_mut(),
            });

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
            *self.inner.mpv_gl.lock().unwrap() = Some(MpvRenderContextPtr(mpv_gl));

            // Set up the update callback with our custom context
            if !*self.inner.update_callback_registered.lock().unwrap() {
                // Create update context that just signals frame pending
                struct UpdateContext {}

                let update_ctx = Box::new(UpdateContext {});

                mpv_render_context_set_update_callback(
                    mpv_gl,
                    Some(Self::on_mpv_render_update),
                    Box::into_raw(update_ctx) as *mut c_void,
                );
                *self.inner.update_callback_registered.lock().unwrap() = true;
            }

            info!("OpenGL render context initialized");

            // Load pending media if any - do this after a small delay to ensure context is ready
            if let Some(url) = self.inner.pending_media_url.lock().unwrap().take() {
                debug!("Loading pending media: {}", url);
                let inner_clone = self.inner.clone();
                let url_clone = url.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(100), move || {
                    if let Some(ref mpv) = *inner_clone.mpv.lock().unwrap() {
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
            debug!("GLArea realized - checking if MPV render context needs initialization");

            // Mark as realized
            inner_realize
                .gl_area_realized
                .store(true, Ordering::Release);

            // Make GL context current
            gl_area.make_current();

            // Check if we already have a render context (might happen on macOS with re-realize)
            if inner_realize.mpv_gl.lock().unwrap().is_some() {
                debug!("MPV render context already exists, skipping re-initialization");
                return;
            }

            // Initialize MPV if not done
            if inner_realize.mpv.lock().unwrap().is_none() {
                match MpvPlayerInner::init_mpv(&inner_realize) {
                    Ok(mpv) => {
                        // Apply initial upscaling mode
                        let initial_mode = *inner_realize.upscaling_mode.lock().unwrap();
                        player_self
                            .apply_upscaling_settings(&mpv, initial_mode)
                            .unwrap_or(());

                        *inner_realize.mpv.lock().unwrap() = Some(mpv);
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
            // Don't render if GLArea is not realized (prevents OpenGL errors on macOS)
            if !inner_render.gl_area_realized.load(Ordering::Acquire) {
                // Return Proceed to let GTK handle it, but skip our rendering
                return glib::Propagation::Proceed;
            }

            // Reset frame pending flag
            // inner_render.frame_pending.store(false, Ordering::Release); // Removed unused field

            // Track render performance
            let now = Instant::now();
            let elapsed = now.duration_since(*inner_render.last_render_time.lock().unwrap());
            *inner_render.last_render_time.lock().unwrap() = now;

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
            if let Some(MpvRenderContextPtr(mpv_gl)) = &*inner_render.mpv_gl.lock().unwrap() {
                unsafe {
                    // Get logical and pixel dimensions
                    let (logical_width, logical_height) = (gl_area.width(), gl_area.height());
                    let scale_factor = gl_area.scale_factor();
                    let (pixel_width, pixel_height) =
                        (logical_width * scale_factor, logical_height * scale_factor);

                    // Skip rendering if area has no size
                    if logical_width <= 0
                        || logical_height <= 0
                        || pixel_width <= 0
                        || pixel_height <= 0
                    {
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
                    let fbo = {
                        let mut cached_fbo = inner_render.cached_fbo.lock().unwrap();
                        if *cached_fbo < 0 {
                            let mut current_fbo = 0i32;
                            if let Some(&get_integerv) = GL_GET_INTEGERV_FN
                                .get_or_init(|| {
                                    Self::load_gl_function_ptr("glGetIntegerv").map(|ptr| unsafe {
                                        std::mem::transmute::<
                                            *mut c_void,
                                            unsafe extern "C" fn(u32, *mut i32),
                                        >(ptr)
                                    })
                                })
                                .as_ref()
                            {
                                const GL_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
                                get_integerv(GL_FRAMEBUFFER_BINDING, &mut current_fbo);
                            }
                            *cached_fbo = current_fbo;
                            current_fbo
                        } else {
                            *cached_fbo
                        }
                    };

                    let flip_y = 1i32; // GTK4 needs Y-flipping

                    let opengl_fbo = mpv_opengl_fbo {
                        fbo,
                        w: pixel_width,
                        h: pixel_height,
                        internal_format: 0,
                    };

                    // Set OpenGL viewport to match the render area
                    if let Some(&viewport) = GL_VIEWPORT_FN
                        .get_or_init(|| {
                            Self::load_gl_function_ptr("glViewport").map(|ptr| unsafe {
                                std::mem::transmute::<
                                    *mut c_void,
                                    unsafe extern "C" fn(i32, i32, i32, i32),
                                >(ptr)
                            })
                        })
                        .as_ref()
                    {
                        viewport(0, 0, pixel_width, pixel_height);
                    }

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
                        if let Some(&flush) = GL_FLUSH_FN
                            .get_or_init(|| {
                                Self::load_gl_function_ptr("glFlush").map(|ptr| unsafe {
                                    std::mem::transmute::<*mut c_void, unsafe extern "C" fn()>(ptr)
                                })
                            })
                            .as_ref()
                        {
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

            // Mark as not realized
            inner_unrealize
                .gl_area_realized
                .store(false, Ordering::Release);

            // On macOS, don't immediately free the render context as it might be temporary
            // The unrealize/realize cycle can happen during widget reparenting
            #[cfg(target_os = "macos")]
            {
                // Just log it, don't free the context yet
                warn!("GLArea unrealized on macOS - keeping render context alive");
            }

            #[cfg(not(target_os = "macos"))]
            {
                // Clean up render context on other platforms
                if let Some(MpvRenderContextPtr(mpv_gl)) =
                    inner_unrealize.mpv_gl.lock().unwrap().take()
                {
                    unsafe {
                        mpv_render_context_free(mpv_gl);
                    }
                }
            }
        });

        // Note: We don't store the GLArea to keep the player Send+Sync

        // Adaptive timer - only runs when playing and adjusts frequency based on content
        let gl_area_timer = gl_area.clone();
        let inner_timer = inner.clone();
        let timer_id = glib::timeout_add_local(Duration::from_millis(16), move || {
            // Only render if GLArea is realized and we have a context
            if inner_timer.gl_area_realized.load(Ordering::Acquire)
                && inner_timer.mpv_gl.lock().unwrap().is_some()
            {
                // Check if we're actually playing and not seeking
                if let Some(ref mpv) = *inner_timer.mpv.lock().unwrap() {
                    // Check if we're seeking - if so, skip automatic render
                    let is_seeking = inner_timer.seek_pending.lock().unwrap().is_some();
                    if !is_seeking
                        && let Ok(paused) = mpv.get_property::<bool>("pause")
                        && !paused
                    {
                        // Queue render - the render callback will check if it's safe
                        gl_area_timer.queue_render();
                    }
                }
            }
            glib::ControlFlow::Continue
        });

        // Store timer handle so we can cancel it on cleanup
        *self.inner.timer_handle.lock().unwrap() = Some(timer_id);

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
        if self.inner.mpv_gl.lock().unwrap().is_none() {
            warn!(
                "MpvPlayer::load_media() - Render context not initialized yet, deferring media load"
            );
            // Store the URL to load later when render context is ready
            *self.inner.pending_media_url.lock().unwrap() = Some(url.to_string());
            return Ok(());
        }

        // Initialize MPV if not already done
        if self.inner.mpv.lock().unwrap().is_none() {
            let mpv = MpvPlayerInner::init_mpv(&self.inner)?;
            *self.inner.mpv.lock().unwrap() = Some(mpv);
        }

        // Load the media file
        if let Some(ref mpv) = *self.inner.mpv.lock().unwrap() {
            mpv.command("loadfile", &[url, "replace"])
                .map_err(|e| anyhow::anyhow!("Failed to load media: {:?}", e))?;
            debug!("Media load command sent");

            // Start monitoring for errors
            self.start_event_monitoring();
        } else {
            return Err(anyhow::anyhow!("MPV not initialized"));
        }

        Ok(())
    }

    pub async fn play(&self) -> Result<()> {
        debug!("Starting playback");

        if let Some(ref mpv) = *self.inner.mpv.lock().unwrap() {
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

        if let Some(ref mpv) = *self.inner.mpv.lock().unwrap() {
            mpv.set_property("pause", true)
                .map_err(|e| anyhow::anyhow!("Failed to set pause=true: {:?}", e))?;

            let mut state = self.inner.state.write().await;
            *state = PlayerState::Paused;
        }
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        debug!("MpvPlayer::stop() - Stopping playback with immediate audio cut");

        if let Some(ref mpv) = *self.inner.mpv.lock().unwrap() {
            // IMMEDIATE: Mute volume for instant silence
            if let Err(e) = mpv.command("set", &["volume", "0"]) {
                warn!("Failed to mute volume during stop: {:?}", e);
            }

            // IMMEDIATE: Disable audio output completely
            if let Err(e) = mpv.command("set", &["ao", "null"]) {
                warn!("Failed to disable audio output: {:?}", e);
            }

            // THEN: Stop media playback
            debug!("MpvPlayer::stop() - Sending stop command to MPV");
            mpv.command("stop", &[])
                .map_err(|e| anyhow::anyhow!("Failed to stop: {:?}", e))?;

            // Also clear the playlist to ensure no media is loaded
            debug!("MpvPlayer::stop() - Clearing MPV playlist");
            mpv.command("playlist-clear", &[])
                .map_err(|e| anyhow::anyhow!("Failed to clear playlist: {:?}", e))?;

            // Set idle mode to prevent MPV from closing
            debug!("MpvPlayer::stop() - Setting MPV to idle mode");
            mpv.command("set", &["idle", "yes"])
                .map_err(|e| anyhow::anyhow!("Failed to set idle mode: {:?}", e))?;

            let mut state = self.inner.state.write().await;
            *state = PlayerState::Stopped;

            info!("MpvPlayer::stop() - Playback stopped with immediate audio termination");
        } else {
            debug!("MpvPlayer::stop() - No MPV instance found, nothing to stop");
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
        if let Some(timer) = self.inner.seek_timer.lock().unwrap().take() {
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
                && let Some(ref mpv) = *inner.mpv.lock().unwrap()
            {
                // Use keyframe seeking for speed
                if let Err(e) = mpv.command("seek", &[&pos.to_string(), "absolute"]) {
                    error!("Failed to seek: {:?}", e);
                    // Clear last seek target on error
                    let mut last_target = last_seek_target.lock().unwrap();
                    *last_target = None;
                }
                // Note: Frame update will happen on next render cycle
            }

            // Clear the timer reference
            *inner.seek_timer.lock().unwrap() = None;
        });

        *self.inner.seek_timer.lock().unwrap() = Some(timer_id);
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
                    return Some(Duration::from_secs_f64(target_pos.max(0.0)));
                }
            }
        }

        // Otherwise return the actual position
        if let Some(ref mpv) = *self.inner.mpv.lock().unwrap()
            && let Ok(pos) = mpv.get_property::<f64>("time-pos")
        {
            // Clear the last seek target since we're at the actual position now
            let mut last_target = self.inner.last_seek_target.lock().unwrap();
            *last_target = None;
            return Some(Duration::from_secs_f64(pos.max(0.0)));
        }
        None
    }

    pub async fn get_duration(&self) -> Option<Duration> {
        if let Some(ref mpv) = *self.inner.mpv.lock().unwrap()
            && let Ok(dur) = mpv.get_property::<f64>("duration")
        {
            // Duration should never be negative, but clamp just in case
            return Some(Duration::from_secs_f64(dur.max(0.0)));
        }
        None
    }

    pub async fn set_volume(&self, volume: f64) -> Result<()> {
        if let Some(ref mpv) = *self.inner.mpv.lock().unwrap() {
            // MPV expects volume in 0-100 range
            let mpv_volume = (volume * 100.0).clamp(0.0, 100.0);
            mpv.set_property("volume", mpv_volume)
                .map_err(|e| anyhow::anyhow!("Failed to set volume: {:?}", e))?;
        }
        Ok(())
    }

    pub async fn get_video_dimensions(&self) -> Option<(i32, i32)> {
        if let Some(ref mpv) = *self.inner.mpv.lock().unwrap()
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
        // Query MPV for the actual state instead of relying on cached state
        if let Some(ref mpv) = *self.inner.mpv.lock().unwrap() {
            // Check if MPV is actually paused or playing
            if let Ok(paused) = mpv.get_property::<bool>("pause") {
                if paused {
                    return PlayerState::Paused;
                } else {
                    // Check if we're actually playing something
                    if let Ok(idle) = mpv.get_property::<bool>("idle-active") {
                        if idle {
                            return PlayerState::Idle;
                        } else {
                            return PlayerState::Playing;
                        }
                    }
                    return PlayerState::Playing;
                }
            }
        }

        // Fall back to cached state if MPV is not available
        self.inner.state.read().await.clone()
    }

    pub async fn get_audio_tracks(&self) -> Vec<(i32, String)> {
        let mut tracks = Vec::new();

        if let Some(ref mpv) = *self.inner.mpv.lock().unwrap()
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

        if let Some(ref mpv) = *self.inner.mpv.lock().unwrap()
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
        if let Some(ref mpv) = *self.inner.mpv.lock().unwrap() {
            mpv.set_property("aid", track_index as i64)
                .map_err(|e| anyhow::anyhow!("Failed to set audio track: {:?}", e))?;
            debug!("Set audio track to {}", track_index);
        }
        Ok(())
    }

    pub async fn set_subtitle_track(&self, track_index: i32) -> Result<()> {
        if let Some(ref mpv) = *self.inner.mpv.lock().unwrap() {
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
        if let Some(ref mpv) = *self.inner.mpv.lock().unwrap()
            && let Ok(aid) = mpv.get_property::<i64>("aid")
        {
            return aid as i32;
        }
        -1
    }

    pub async fn get_current_subtitle_track(&self) -> i32 {
        if let Some(ref mpv) = *self.inner.mpv.lock().unwrap()
            && let Ok(sid) = mpv.get_property::<i64>("sid")
        {
            return sid as i32;
        }
        -1
    }

    pub async fn set_upscaling_mode(&self, mode: UpscalingMode) -> Result<()> {
        let mut current_mode = self.inner.upscaling_mode.lock().unwrap();
        *current_mode = mode;
        drop(current_mode);

        if let Some(ref mpv) = *self.inner.mpv.lock().unwrap() {
            self.apply_upscaling_settings(mpv, mode)?;
        }
        Ok(())
    }

    fn apply_upscaling_settings(&self, mpv: &Mpv, mode: UpscalingMode) -> Result<()> {
        // Clear any existing shaders first
        let _ = mpv.set_property("glsl-shaders", "");

        match mode {
            UpscalingMode::None => {
                // Use basic bilinear scaling
                let _ = mpv.set_property("scale", "bilinear");
                let _ = mpv.set_property("cscale", "bilinear");
                let _ = mpv.set_property("dscale", "bilinear");
                let _ = mpv.set_property("sigmoid-upscaling", false);
                let _ = mpv.set_property("deband", false);
                debug!("Upscaling disabled - using bilinear");
            }
            UpscalingMode::HighQuality => {
                // Use built-in high quality scalers
                let _ = mpv.set_property("scale", "ewa_lanczossharp");
                let _ = mpv.set_property("cscale", "ewa_lanczossharp");
                let _ = mpv.set_property("dscale", "mitchell");
                let _ = mpv.set_property("sigmoid-upscaling", true);
                let _ = mpv.set_property("deband", true);
                let _ = mpv.set_property("deband-iterations", 2);
                let _ = mpv.set_property("deband-threshold", 48);
                let _ = mpv.set_property("deband-range", 16);
                let _ = mpv.set_property("deband-grain", 24);
                debug!("High quality upscaling enabled with built-in scalers");
            }
            UpscalingMode::FSR => {
                // Use spline36 fallback
                let _ = mpv.set_property("scale", "spline36");
                let _ = mpv.set_property("cscale", "spline36");
                let _ = mpv.set_property("dscale", "mitchell");
                let _ = mpv.set_property("sigmoid-upscaling", true);
                let _ = mpv.set_property("deband", true);
                debug!("FSR upscaling using fallback scalers");
            }
            UpscalingMode::Anime => {
                // Use optimized built-in settings for anime
                let _ = mpv.set_property("scale", "ewa_lanczossharp");
                let _ = mpv.set_property("cscale", "ewa_lanczossoft");
                let _ = mpv.set_property("dscale", "mitchell");
                let _ = mpv.set_property("sigmoid-upscaling", false);
                let _ = mpv.set_property("deband", true);
                let _ = mpv.set_property("deband-iterations", 4);
                let _ = mpv.set_property("deband-threshold", 64);
                let _ = mpv.set_property("deband-range", 16);
                let _ = mpv.set_property("deband-grain", 48);
                debug!("Anime upscaling using fallback scalers");
            }
            UpscalingMode::Custom => {
                // Custom mode - for future extension
                debug!("Custom upscaling mode selected - no implementation yet");
            }
        }

        info!("Upscaling mode changed to: {}", mode.to_string());
        Ok(())
    }

    pub async fn set_playback_speed(&self, speed: f64) -> Result<()> {
        let inner = self.inner.clone();
        if let Some(ref mpv) = *inner.mpv.lock().unwrap() {
            mpv.set_property("speed", speed)
                .map_err(|e| anyhow::anyhow!("Failed to set playback speed: {:?}", e))?;
        }
        Ok(())
    }

    pub async fn get_playback_speed(&self) -> f64 {
        let inner = self.inner.clone();
        if let Some(ref mpv) = *inner.mpv.lock().unwrap() {
            mpv.get_property::<f64>("speed").unwrap_or(1.0)
        } else {
            1.0
        }
    }

    pub async fn frame_step_forward(&self) -> Result<()> {
        let inner = self.inner.clone();
        if let Some(ref mpv) = *inner.mpv.lock().unwrap() {
            mpv.command("frame-step", &[])
                .map_err(|e| anyhow::anyhow!("Failed to step forward: {:?}", e))?;
        }
        Ok(())
    }

    pub async fn frame_step_backward(&self) -> Result<()> {
        let inner = self.inner.clone();
        if let Some(ref mpv) = *inner.mpv.lock().unwrap() {
            mpv.command("frame-back-step", &[])
                .map_err(|e| anyhow::anyhow!("Failed to step backward: {:?}", e))?;
        }
        Ok(())
    }

    pub async fn toggle_mute(&self) -> Result<()> {
        let inner = self.inner.clone();
        if let Some(ref mpv) = *inner.mpv.lock().unwrap() {
            let muted = mpv.get_property::<bool>("mute").unwrap_or(false);
            mpv.set_property("mute", !muted)
                .map_err(|e| anyhow::anyhow!("Failed to toggle mute: {:?}", e))?;
        }
        Ok(())
    }

    pub async fn is_muted(&self) -> bool {
        let inner = self.inner.clone();
        if let Some(ref mpv) = *inner.mpv.lock().unwrap() {
            mpv.get_property::<bool>("mute").unwrap_or(false)
        } else {
            false
        }
    }

    pub async fn cycle_subtitle_track(&self) -> Result<()> {
        let inner = self.inner.clone();
        if let Some(ref mpv) = *inner.mpv.lock().unwrap() {
            mpv.command("cycle", &["sub"])
                .map_err(|e| anyhow::anyhow!("Failed to cycle subtitle track: {:?}", e))?;
        }
        Ok(())
    }

    pub async fn cycle_audio_track(&self) -> Result<()> {
        let inner = self.inner.clone();
        if let Some(ref mpv) = *inner.mpv.lock().unwrap() {
            mpv.command("cycle", &["audio"])
                .map_err(|e| anyhow::anyhow!("Failed to cycle audio track: {:?}", e))?;
        }
        Ok(())
    }

    pub async fn set_zoom_mode(&self, mode: ZoomMode) -> Result<()> {
        let inner = self.inner.clone();

        // Update internal state
        *inner.zoom_mode.lock().unwrap() = mode;

        if let Some(ref mpv) = *inner.mpv.lock().unwrap() {
            match mode {
                ZoomMode::Fit => {
                    // Reset to fit entire video
                    mpv.set_property("video-zoom", 0.0)
                        .map_err(|e| anyhow::anyhow!("Failed to set video-zoom: {:?}", e))?;
                    mpv.set_property("video-pan-x", 0.0)
                        .map_err(|e| anyhow::anyhow!("Failed to set video-pan-x: {:?}", e))?;
                    mpv.set_property("video-pan-y", 0.0)
                        .map_err(|e| anyhow::anyhow!("Failed to set video-pan-y: {:?}", e))?;
                    mpv.set_property("video-aspect-override", "-1")
                        .map_err(|e| {
                            anyhow::anyhow!("Failed to set video-aspect-override: {:?}", e)
                        })?;
                }
                ZoomMode::Fill => {
                    // Calculate zoom to fill window (will be done dynamically based on aspect ratios)
                    // For now, just zoom in slightly to demonstrate
                    mpv.set_property("video-zoom", 0.5)
                        .map_err(|e| anyhow::anyhow!("Failed to set video-zoom: {:?}", e))?;
                    mpv.set_property("video-aspect-override", "-1")
                        .map_err(|e| {
                            anyhow::anyhow!("Failed to set video-aspect-override: {:?}", e)
                        })?;
                }
                ZoomMode::Zoom16_9 => {
                    // Force 16:9 aspect ratio
                    mpv.set_property("video-aspect-override", "16:9")
                        .map_err(|e| {
                            anyhow::anyhow!("Failed to set video-aspect-override: {:?}", e)
                        })?;
                    mpv.set_property("video-zoom", 0.0)
                        .map_err(|e| anyhow::anyhow!("Failed to set video-zoom: {:?}", e))?;
                }
                ZoomMode::Zoom4_3 => {
                    // Force 4:3 aspect ratio
                    mpv.set_property("video-aspect-override", "4:3")
                        .map_err(|e| {
                            anyhow::anyhow!("Failed to set video-aspect-override: {:?}", e)
                        })?;
                    mpv.set_property("video-zoom", 0.0)
                        .map_err(|e| anyhow::anyhow!("Failed to set video-zoom: {:?}", e))?;
                }
                ZoomMode::Zoom2_35 => {
                    // Force 2.35:1 (cinematic) aspect ratio
                    mpv.set_property("video-aspect-override", "2.35:1")
                        .map_err(|e| {
                            anyhow::anyhow!("Failed to set video-aspect-override: {:?}", e)
                        })?;
                    mpv.set_property("video-zoom", 0.0)
                        .map_err(|e| anyhow::anyhow!("Failed to set video-zoom: {:?}", e))?;
                }
                ZoomMode::Custom(level) => {
                    // Custom zoom level (in log2 scale for MPV)
                    // Convert percentage to log2 scale: 1.0 = 100% = 0 zoom, 2.0 = 200% = 1 zoom
                    let zoom_value = (level.max(0.1)).log2();
                    mpv.set_property("video-zoom", zoom_value)
                        .map_err(|e| anyhow::anyhow!("Failed to set video-zoom: {:?}", e))?;
                    mpv.set_property("video-aspect-override", "-1")
                        .map_err(|e| {
                            anyhow::anyhow!("Failed to set video-aspect-override: {:?}", e)
                        })?;
                }
            }
        }

        Ok(())
    }

    pub async fn get_zoom_mode(&self) -> ZoomMode {
        *self.inner.zoom_mode.lock().unwrap()
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
            // Enable info level for network/streaming to help debug issues
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
        let _ = mpv.set_property("opengl-swapinterval", 1); // May not be available on all systems

        // Seek optimization settings - prioritize speed
        mpv.set_property("hr-seek", "no") // Disable HR seeking for speed
            .map_err(|e| anyhow::anyhow!("Failed to set hr-seek: {:?}", e))?;
        mpv.set_property("hr-seek-framedrop", true) // Allow frame drops for smoother seeking
            .map_err(|e| anyhow::anyhow!("Failed to set hr-seek-framedrop: {:?}", e))?;
        mpv.set_property("index", "default") // Use index for faster seeking
            .map_err(|e| anyhow::anyhow!("Failed to set index: {:?}", e))?;
        let _ = mpv.set_property("force-seekable", true); // Force seekable even for streams

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

        // Use MPV's default cache settings for optimal streaming performance

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
        if let Some(timer_id) = self.timer_handle.lock().unwrap().take() {
            timer_id.remove();
        }

        // Cancel the seek timer if it's running
        if let Some(seek_timer) = self.seek_timer.lock().unwrap().take() {
            seek_timer.remove();
        }

        // Clean up render context
        if let Some(MpvRenderContextPtr(mpv_gl)) = self.mpv_gl.lock().unwrap().take() {
            unsafe {
                mpv_render_context_free(mpv_gl);
            }
        }
    }
}
