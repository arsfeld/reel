use tracing::info;

/// Configure platform-specific video output settings and native theming for Slint
pub fn configure_video_output() {
    info!("Configuring Slint platform settings for native appearance");

    // Configure native platform integration
    configure_native_theming();
    configure_video_backend();
}

/// Configure Slint to use native platform theming and widgets
pub fn configure_native_theming() {
    unsafe {
        // Force native style for platform-appropriate appearance
        std::env::set_var("SLINT_STYLE", "native");

        // Platform-specific optimizations
        #[cfg(target_os = "linux")]
        {
            info!("Configuring Linux/GTK native integration");
            // On Linux, native style will use Qt if available, fallback to Fluent
            std::env::set_var("SLINT_BACKEND", "qt");
        }

        #[cfg(target_os = "macos")]
        {
            info!("Configuring macOS native integration");
            // On macOS, use native Cocoa styling
            std::env::set_var("SLINT_BACKEND", "winit");
        }

        #[cfg(target_os = "windows")]
        {
            info!("Configuring Windows native integration");
            // On Windows, use native Windows styling
            std::env::set_var("SLINT_BACKEND", "winit");
        }
    }
}

/// Configure video backend for optimal media playback
pub fn configure_video_backend() {
    info!("Configuring video playback backend");

    unsafe {
        // Set environment variables for optimal video playback with Slint
        std::env::set_var("SLINT_BACKEND", "gl");

        // Configure GStreamer for Slint integration
        std::env::set_var("GST_GL_WINDOW", "auto");
        std::env::set_var("GST_GL_API", "opengl3");

        // Enable hardware acceleration when available
        std::env::set_var("GST_GL_PLATFORM", "auto");
    }
}
