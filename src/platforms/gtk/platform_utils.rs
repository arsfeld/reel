use tracing::{info, warn};

/// Configure platform-specific video output settings
pub fn configure_video_output() {
    #[cfg(target_os = "macos")]
    {
        configure_macos_video();
    }

    #[cfg(target_os = "linux")]
    {
        configure_linux_video();
    }
}

#[cfg(target_os = "macos")]
fn configure_macos_video() {
    info!("Configuring macOS video output settings");

    // Check if we're running under Rosetta 2 (x86_64 on ARM)
    let is_rosetta = std::env::var("PROCESSOR_ARCHITEW6432").is_ok();
    if is_rosetta {
        warn!("Running under Rosetta 2 - video performance may be degraded");
    }

    // Force MPV to use the libmpv API with proper macOS backend
    unsafe {
        std::env::set_var("MPV_HOME", "~/.config/mpv");
    }

    // Configure hardware acceleration
    if !is_rosetta {
        unsafe {
            std::env::set_var("MPV_HWDEC", "videotoolbox");
        }
    }

    // Ensure proper OpenGL context sharing
    unsafe {
        std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "0");
    }
}

#[cfg(target_os = "linux")]
fn configure_linux_video() {
    info!("Configuring Linux video output settings");

    // Check for Wayland vs X11
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        info!("Wayland display detected");
        unsafe {
            std::env::set_var("GST_GL_WINDOW", "wayland");
            std::env::set_var("GDK_BACKEND", "wayland");
        }
    } else if std::env::var("DISPLAY").is_ok() {
        info!("X11 display detected");
        unsafe {
            std::env::set_var("GST_GL_WINDOW", "x11");
            std::env::set_var("GDK_BACKEND", "x11");
        }
    }

    // Enable VA-API hardware acceleration if available
    if std::path::Path::new("/dev/dri").exists() {
        unsafe {
            std::env::set_var("GST_VAAPI_ALL_DRIVERS", "1");
            std::env::set_var("MPV_HWDEC", "vaapi");
        }
    }
}

/// Check if video hardware acceleration is available
pub fn check_hw_acceleration() -> bool {
    #[cfg(target_os = "macos")]
    {
        // On macOS 10.14+, Metal is always available
        // VideoToolbox is available on all modern Macs
        // Avoid slow system_profiler call - just check OS version
        let os_version = std::process::Command::new("sw_vers")
            .args(&["-productVersion"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default();

        // Check if we're on macOS 10.14 or later (Mojave+)
        // All these versions have Metal support
        let has_metal =
            if let Some(major_minor) = os_version.trim().split('.').collect::<Vec<_>>().get(..2) {
                if let (Ok(major), Ok(minor)) =
                    (major_minor[0].parse::<u32>(), major_minor[1].parse::<u32>())
                {
                    major > 10 || (major == 10 && minor >= 14)
                } else {
                    true // Assume modern macOS if we can't parse
                }
            } else {
                true // Assume modern macOS if format is unexpected
            };

        if has_metal {
            info!("Hardware acceleration available via VideoToolbox/Metal");
        } else {
            warn!("Hardware acceleration may be limited on older macOS");
        }

        has_metal
    }

    #[cfg(target_os = "linux")]
    {
        // Check for VA-API or VDPAU support
        let has_vaapi = std::path::Path::new("/dev/dri/renderD128").exists();
        let has_vdpau = std::env::var("VDPAU_DRIVER").is_ok();

        let hw_available = has_vaapi || has_vdpau;

        if hw_available {
            info!("Hardware acceleration available via VA-API/VDPAU");
        } else {
            warn!("Hardware acceleration not available");
        }

        hw_available
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        false
    }
}
