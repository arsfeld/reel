#![warn(clippy::too_many_lines)]

mod app;
mod components;
mod player;
mod services;

use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("REEL_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting Reel");

    let app = relm4::RelmApp::new("dev.arsfeld.Reel");

    // Force dark theme
    let style_manager = libadwaita::StyleManager::default();
    style_manager.set_color_scheme(libadwaita::ColorScheme::ForceDark);

    relm4::set_global_css(include_str!("style.css"));

    let file_arg = std::env::args().nth(1);
    app.run::<app::App>(file_arg);
}
