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

fn main() -> Result<()> {
    use tracing::info;

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("reel=debug")
        .init();

    // Run the appropriate frontend based on compile-time features
    // Features are mutually exclusive - only one should be enabled at build time
    #[cfg(all(feature = "gtk", not(feature = "slint")))]
    {
        info!("Starting Reel with GTK frontend");
        // GTK needs async runtime
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(run_gtk_frontend())
    }

    #[cfg(all(feature = "slint", not(feature = "gtk")))]
    {
        info!("Starting Reel with Slint frontend");
        // Slint runs on main thread
        run_slint_frontend()
    }

    #[cfg(all(feature = "gtk", feature = "slint"))]
    {
        compile_error!(
            "Both 'gtk' and 'slint' features are enabled. Please enable only one UI framework at build time."
        )
    }

    #[cfg(not(any(feature = "gtk", feature = "slint")))]
    {
        compile_error!(
            "No UI framework feature enabled. Please enable either 'gtk' or 'slint' feature."
        )
    }
}

#[cfg(feature = "gtk")]
async fn run_gtk_frontend() -> Result<()> {
    use gtk4::prelude::*;
    use libadwaita as adw;
    use libadwaita::prelude::*;
    use platforms::gtk::ReelApp;
    use tracing::info;

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

#[cfg(feature = "slint")]
fn run_slint_frontend() -> Result<()> {
    use platforms::slint::ReelSlintApp;
    use tracing::info;

    info!("Initializing Slint frontend");

    // Initialize GStreamer for video playback
    gstreamer::init()?;

    // Load configuration
    let config = std::sync::Arc::new(tokio::sync::RwLock::new(crate::config::Config::load()?));

    // Create and initialize the Slint app
    let mut app = ReelSlintApp::new()?;
    app.initialize(config)?;

    // Run the application
    let exit_code = app.run()?;

    std::process::exit(exit_code);
}
