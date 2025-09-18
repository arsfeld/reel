use anyhow::Result;

mod app;
mod backends;
mod config;
mod constants;
mod core;
mod db;
mod events;
mod mapper;
mod models;
mod player;
mod services;
// State module removed in Relm4 migration - components manage their own state
// mod state;
mod ui;
mod utils;
mod workers;

fn main() -> Result<()> {
    use app::AppPlatform;
    use std::sync::Arc;
    use tracing::info;

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("reel=debug")
        .init();

    info!("Starting Reel application");

    // Initialize GTK and Adwaita first
    gtk4::init()?;
    libadwaita::init()?;

    // Initialize GStreamer
    gstreamer::init()?;

    // Initialize Tokio runtime for async operations
    let runtime = Arc::new(tokio::runtime::Runtime::new()?);

    // Run the appropriate platform implementation
    AppPlatform::run_relm4(runtime)?;

    Ok(())
}
