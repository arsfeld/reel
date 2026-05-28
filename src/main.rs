#![warn(clippy::too_many_lines)]

mod app;
mod components;
mod config;
mod db;
mod models;
#[allow(dead_code)]
mod navigation;
mod player;
#[allow(unused)]
mod schema;
mod services;
mod settings;

use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_env("REEL_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting Reel");

    if let Err(e) = gst::init() {
        tracing::error!("Failed to initialize GStreamer: {e}");
        std::process::exit(1);
    }

    let app = relm4::RelmApp::new("dev.arsfeld.Reel");

    // Force dark theme
    let style_manager = adw::StyleManager::default();
    style_manager.set_color_scheme(adw::ColorScheme::ForceDark);

    relm4::set_global_css(include_str!("style.css"));

    let file_arg = std::env::args().nth(1);
    app.run::<app::App>(file_arg);
}
