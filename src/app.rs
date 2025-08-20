use anyhow::Result;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use adw::glib;
use std::sync::Arc;
use tracing::info;

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
        
        // Set the application icon
        gtk4::IconTheme::default().add_resource_path("/com/github/arsfeld/Reel/icons");
        
        // Setup actions
        let state_clone = state.clone();
        let config_clone = config.clone();
        
        app.connect_activate(move |app| {
            info!("Application activated - Creating main window");
            
            // Load CSS
            let css_provider = gtk4::CssProvider::new();
            css_provider.load_from_resource("/com/github/arsfeld/Reel/style.css");
            gtk4::style_context_add_provider_for_display(
                &gtk4::gdk::Display::default().expect("Could not get default display"),
                &css_provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
            
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