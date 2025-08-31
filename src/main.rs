use anyhow::Result;

mod backends;
mod config;
mod constants;
mod core;
mod db;
mod events;
mod models;
mod platforms;
mod player;
mod services;
mod state;
mod utils;

#[cfg(all(feature = "gtk"))]
fn main() -> Result<()> {
    use gtk4::prelude::*;
    use libadwaita as adw;
    use libadwaita::prelude::*;
    use platforms::gtk::ReelApp;
    use tracing::info;

    const APP_ID: &str = "dev.arsfeld.Reel";

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("reel=debug")
        .init();

    info!("Starting Reel GTK frontend");

    // Initialize Tokio runtime for async operations
    let runtime = tokio::runtime::Runtime::new()?;
    let _guard = runtime.enter();

    // Spawn a thread to keep the runtime alive
    std::thread::spawn(move || {
        runtime.block_on(async {
            // Keep runtime alive for the duration of the application
            std::future::pending::<()>().await;
        });
    });

    // Platform-specific initialization
    #[cfg(target_os = "macos")]
    {
        info!("Detected macOS platform - setting up environment");
        // Force GTK to use OpenGL backend on macOS for better video playback
        unsafe {
            std::env::set_var("GDK_GL", "prefer-gl");
            std::env::set_var("GSK_RENDERER", "gl");
            // Ensure MPV uses the right video output on macOS
            std::env::set_var("MPV_COCOA_FORCE_DEDICATED_GPU", "1");
        }
    }

    // Initialize GTK and Adwaita
    gtk4::init()?;
    adw::init()?;

    // Initialize GStreamer with platform-specific settings
    #[cfg(target_os = "macos")]
    {
        // Set GStreamer to use native macOS video sinks
        unsafe {
            std::env::set_var("GST_PLUGIN_PATH", "/usr/local/lib/gstreamer-1.0");
            std::env::set_var("GST_PLUGIN_SYSTEM_PATH", "/usr/local/lib/gstreamer-1.0");
        }
    }
    gstreamer::init()?;

    // Load compiled resources
    gtk4::gio::resources_register_include!("resources.gresource")
        .expect("Failed to register resources");

    // Create and run the application
    let app = ReelApp::new()?;
    let exit_code = app.run();

    std::process::exit(exit_code.into());
}
