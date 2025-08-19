use anyhow::Result;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use adw::glib;
use std::sync::Arc;
use tracing::{debug, info};

use crate::config::Config;
use crate::state::AppState;
use crate::ui::MainWindow;
use crate::APP_ID;

pub struct ReelApp {
    app: adw::Application,
    state: Arc<AppState>,
    config: Arc<Config>,
}

impl ReelApp {
    pub fn new() -> Result<Self> {
        // Load configuration
        let config = Arc::new(Config::load()?);
        
        // Initialize application state
        let state = Arc::new(AppState::new(config.clone())?);
        
        // Create the application
        let app = adw::Application::builder()
            .application_id(APP_ID)
            .build();
        
        // Setup actions
        let state_clone = state.clone();
        let config_clone = config.clone();
        
        app.connect_activate(move |app| {
            info!("Application activated - Creating main window");
            
            // Create main window
            let window = MainWindow::new(app, state_clone.clone(), config_clone.clone());
            info!("Main window created, presenting...");
            window.present();
            info!("Main window presented");
        });
        
        Ok(Self {
            app,
            state,
            config,
        })
    }
    
    pub fn run(&self) -> glib::ExitCode {
        info!("Running Reel application");
        self.app.run()
    }
}