#![allow(dead_code)]

use anyhow::Result;
use gtk4::{glib, prelude::*};
use libadwaita as adw;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::config::Config;
use crate::core::state::AppState;
use crate::platforms::gtk::ui::main_window::ReelMainWindow;

const APP_ID: &str = "dev.arsfeld.Reel";

pub struct ReelApp {
    app: adw::Application,
}

impl ReelApp {
    pub fn new() -> Result<Self> {
        let app = adw::Application::builder().application_id(APP_ID).build();

        Ok(Self { app })
    }

    pub fn run(&self) -> glib::ExitCode {
        let app = &self.app;

        app.connect_activate(move |app| {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

            rt.block_on(async move {
                info!("Application activated, initializing...");

                // Initialize configuration
                let config = Arc::new(RwLock::new(Config::load().unwrap_or_default()));

                // Initialize app state
                let app_state = match AppState::new_async(config.clone()).await {
                    Ok(state) => Arc::new(state),
                    Err(e) => {
                        eprintln!("Failed to initialize app state: {}", e);
                        app.quit();
                        return;
                    }
                };

                // Create and show main window
                let window = ReelMainWindow::new(&app, app_state, config);
                window.present();
            });
        });

        app.run()
    }
}
