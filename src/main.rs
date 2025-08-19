use anyhow::Result;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use tracing::info;
use tracing_subscriber;

mod app;
mod config;
mod ui;
mod backends;
mod models;
mod services;
mod state;
mod player;
mod utils;

use app::ReelApp;

const APP_ID: &str = "com.github.arsfeld.Reel";

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("reel=debug")
        .init();

    info!("Starting Reel");

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