use adw::glib;
use anyhow::Result;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::sync::Arc;
use tracing::info;

const APP_ID: &str = "dev.arsfeld.Reel";
use super::platform_utils;
use super::ui::MainWindow;
use crate::config::Config;
use crate::state::AppState;
use tokio::sync::RwLock;

pub struct ReelApp {
    app: adw::Application,
    state: Arc<AppState>,
    config: Arc<RwLock<Config>>,
}

impl ReelApp {
    pub fn new() -> Result<Self> {
        // Configure platform-specific video output settings
        platform_utils::configure_video_output();

        // Check hardware acceleration availability
        platform_utils::check_hw_acceleration();

        // Load configuration once
        let config = Arc::new(RwLock::new(Config::load()?));

        // Initialize application state with shared config
        let state = Arc::new(AppState::new(config.clone())?);

        // SourceCoordinator is already initialized in AppState::new()

        // Create the application
        let app = adw::Application::builder().application_id(APP_ID).build();

        // Set the application icon
        gtk4::IconTheme::default().add_resource_path("/dev/arsfeld/Reel/icons");

        // Setup actions
        let state_clone = state.clone();
        let config_clone = config.clone();

        app.connect_activate(move |app| {
            info!("Application activated - Creating main window");

            // Platform-specific initialization
            #[cfg(target_os = "macos")]
            {
                info!("Initializing macOS-specific settings");
                // On macOS, we may need to set specific environment variables for video playback
                unsafe {
                    std::env::set_var("GST_GL_WINDOW", "cocoa");
                    std::env::set_var("GST_GL_PLATFORM", "cgl");
                    // Use OpenGL instead of Metal for better compatibility
                    std::env::set_var("GSK_RENDERER", "gl");
                }
            }

            // Load CSS
            let css_provider = gtk4::CssProvider::new();
            css_provider.load_from_resource("/dev/arsfeld/Reel/style.css");
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

        Ok(Self { app, state, config })
    }

    pub fn run(&self) -> glib::ExitCode {
        info!("Running Reel application");
        self.app.run()
    }
}
