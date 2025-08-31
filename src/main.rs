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

    // Initialize GTK and Adwaita
    gtk4::init()?;
    adw::init()?;

    // Initialize GStreamer
    gstreamer::init()?;

    // Load compiled resources
    gtk4::gio::resources_register_include!("resources.gresource")
        .expect("Failed to register resources");

    // Create and run the application
    let app = ReelApp::new()?;
    let exit_code = app.run();

    std::process::exit(exit_code.into());
}
