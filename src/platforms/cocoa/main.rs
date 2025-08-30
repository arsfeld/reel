use super::app::CocoaApp;
use crate::config::Config;
use crate::state::app_state::AppState;
use std::sync::Arc;
use tokio::sync::RwLock;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Load configuration
    let config = Arc::new(RwLock::new(Config::load()?));

    // Initialize app state with config
    let state = Arc::new(AppState::new(config)?);

    // Create and run Cocoa app
    let app = CocoaApp::new(state)?;
    app.run();

    Ok(())
}
