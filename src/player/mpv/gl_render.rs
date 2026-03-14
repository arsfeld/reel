use std::collections::HashMap;
use std::ffi::{CStr, CString, c_void};
use std::ptr;
use std::sync::OnceLock;

use libmpv2_sys::*;
use tracing::{error, info, warn};

// Cached GL function pointers
static GL_GET_INTEGERV_FN: OnceLock<Option<unsafe extern "C" fn(u32, *mut i32)>> = OnceLock::new();
static GL_VIEWPORT_FN: OnceLock<Option<unsafe extern "C" fn(i32, i32, i32, i32)>> = OnceLock::new();
static GL_FLUSH_FN: OnceLock<Option<unsafe extern "C" fn()>> = OnceLock::new();

const GL_FRAMEBUFFER_BINDING: u32 = 0x8CA6;

/// Thread-safe proc address resolver that uses EGL + dlsym fallback.
/// This function is called by mpv from its render thread.
///
/// # Safety
/// Called from mpv's render thread with a valid GL function name.
pub unsafe extern "C" fn get_proc_address(
    _ctx: *mut c_void,
    name: *const libc::c_char,
) -> *mut c_void {
    unsafe {
        static mut PROC_CACHE: Option<HashMap<String, *mut c_void>> = None;
        static mut EGL_GET_PROC: Option<*mut c_void> = None;

        let cache_ptr = &raw mut PROC_CACHE;
        if (*cache_ptr).is_none() {
            *cache_ptr = Some(HashMap::new());
            let egl_ptr = &raw mut EGL_GET_PROC;
            *egl_ptr = Some(libc::dlsym(
                libc::RTLD_DEFAULT,
                c"eglGetProcAddress".as_ptr(),
            ));
        }

        let name_str = CStr::from_ptr(name).to_string_lossy().to_string();

        let cache_ptr = &raw mut PROC_CACHE;
        if let Some(cache) = &*cache_ptr
            && let Some(&cached) = cache.get(&name_str)
        {
            return cached;
        }

        let mut func = ptr::null_mut();

        // Try EGL first
        let egl_ptr = &raw const EGL_GET_PROC;
        if let Some(egl_get_proc) = *egl_ptr
            && !egl_get_proc.is_null()
        {
            type EglGetProcFn = unsafe extern "C" fn(*const libc::c_char) -> *mut c_void;
            let get_proc: EglGetProcFn = std::mem::transmute(egl_get_proc);
            func = get_proc(name);
        }

        // Fallback to dlsym
        if func.is_null() {
            func = libc::dlsym(libc::RTLD_DEFAULT, name);
        }

        let cache_ptr = &raw mut PROC_CACHE;
        if let Some(cache) = &mut *cache_ptr {
            cache.insert(name_str.clone(), func);
        }

        if func.is_null() {
            warn!("Failed to resolve GL proc: {}", name_str);
        }

        func
    }
}

fn load_gl_fn_ptr(name: &str) -> Option<*mut c_void> {
    let cname = CString::new(name).ok()?;
    let ptr = unsafe { get_proc_address(ptr::null_mut(), cname.as_ptr()) };
    (!ptr.is_null()).then_some(ptr)
}

/// Create an mpv render context for OpenGL rendering.
///
/// # Safety
/// `mpv_handle` must be a valid mpv client handle.
pub unsafe fn create_render_context(
    mpv_handle: *mut mpv_handle,
) -> Result<*mut mpv_render_context, i32> {
    unsafe {
        let mut render_ctx: *mut mpv_render_context = ptr::null_mut();

        let api_type = CString::new("opengl").unwrap();

        let opengl_params = mpv_opengl_init_params {
            get_proc_address: Some(get_proc_address),
            get_proc_address_ctx: ptr::null_mut(),
        };

        let params = [
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

        let result =
            mpv_render_context_create(&mut render_ctx, mpv_handle, params.as_ptr().cast_mut());

        if result < 0 {
            error!("mpv_render_context_create failed: {}", result);
            return Err(result);
        }

        info!("mpv render context created");
        Ok(render_ctx)
    }
}

/// Render a video frame into the current GL framebuffer.
///
/// # Safety
/// `render_ctx` must be a valid mpv render context. A GL context must be current.
pub unsafe fn render_frame(render_ctx: *mut mpv_render_context, width: i32, height: i32) -> bool {
    unsafe {
        let update_flags = mpv_render_context_update(render_ctx);
        if update_flags == 0 {
            return false;
        }

        // Get current FBO
        let fbo = {
            let mut current_fbo = 0i32;
            if let Some(&get_integerv) = GL_GET_INTEGERV_FN
                .get_or_init(|| {
                    load_gl_fn_ptr("glGetIntegerv").map(|ptr| {
                        std::mem::transmute::<*mut c_void, unsafe extern "C" fn(u32, *mut i32)>(ptr)
                    })
                })
                .as_ref()
            {
                get_integerv(GL_FRAMEBUFFER_BINDING, &mut current_fbo);
            }
            current_fbo
        };

        // Set viewport
        if let Some(&viewport) = GL_VIEWPORT_FN
            .get_or_init(|| {
                load_gl_fn_ptr("glViewport").map(|ptr| {
                    std::mem::transmute::<*mut c_void, unsafe extern "C" fn(i32, i32, i32, i32)>(
                        ptr,
                    )
                })
            })
            .as_ref()
        {
            viewport(0, 0, width, height);
        }

        let flip_y = 1i32;

        let opengl_fbo = mpv_opengl_fbo {
            fbo,
            w: width,
            h: height,
            internal_format: 0,
        };

        let mut params = [
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

        let result = mpv_render_context_render(render_ctx, params.as_mut_ptr());
        if result < 0 {
            error!("mpv_render_context_render failed: {}", result);
            return false;
        }

        // Flush
        if let Some(&flush) = GL_FLUSH_FN
            .get_or_init(|| {
                load_gl_fn_ptr("glFlush")
                    .map(|ptr| std::mem::transmute::<*mut c_void, unsafe extern "C" fn()>(ptr))
            })
            .as_ref()
        {
            flush();
        }

        mpv_render_context_report_swap(render_ctx);

        true
    }
}

/// Set the render update callback.
///
/// # Safety
/// `render_ctx` must be a valid mpv render context.
/// `callback` and `ctx` must remain valid for the lifetime of the render context.
pub unsafe fn set_update_callback(
    render_ctx: *mut mpv_render_context,
    callback: Option<unsafe extern "C" fn(*mut c_void)>,
    ctx: *mut c_void,
) {
    unsafe {
        mpv_render_context_set_update_callback(render_ctx, callback, ctx);
    }
}

/// Free the render context.
///
/// # Safety
/// `render_ctx` must be a valid mpv render context that hasn't been freed yet.
pub unsafe fn free_render_context(render_ctx: *mut mpv_render_context) {
    unsafe {
        mpv_render_context_free(render_ctx);
    }
}
