use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::config::Config;
use crate::core::frontend::Frontend;
use crate::core::state::AppState;
use crate::platforms::macos::MacOSFrontend;

pub async fn macos_main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive("reel=debug".parse()?),
        )
        .init();

    info!("Starting Reel macOS frontend");

    // Initialize configuration
    let config = Arc::new(RwLock::new(Config::load()?));

    // Initialize core state
    let state = Arc::new(AppState::new_async(config).await?);

    // Create macOS frontend
    let mut frontend = MacOSFrontend::new();

    // Initialize frontend with core state
    frontend.initialize(state).await?;

    // Run the frontend
    frontend.run().await?;

    // Shutdown
    frontend.shutdown().await?;

    Ok(())
}
