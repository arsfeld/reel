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
// State module removed in Relm4 migration - components manage their own state
// mod state;
mod utils;

fn main() -> Result<()> {
    use core::frontend::Frontend;
    use platforms::relm4::Relm4Platform;
    use std::sync::Arc;
    use tracing::info;

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("reel=debug")
        .init();

    info!("Starting Reel Relm4 frontend");

    // Initialize GTK and Adwaita first
    gtk4::init()?;
    libadwaita::init()?;

    // Initialize GStreamer
    gstreamer::init()?;

    // Initialize Tokio runtime for async operations
    let runtime = Arc::new(tokio::runtime::Runtime::new()?);

    // Create and run the Relm4 platform
    let platform = Relm4Platform::new();
    platform.run(runtime)?;

    Ok(())
}
