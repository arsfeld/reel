use anyhow::Result;
use std::sync::Arc;
use tracing::info;

use crate::config::Config;
use crate::state::AppState;

// Include the compiled Slint UI
slint::include_modules!();

pub struct ReelSlintApp {
    app_state: Option<Arc<AppState>>,
    is_running: bool,
}

impl ReelSlintApp {
    pub fn new() -> Result<Self> {
        // Configure platform-specific video output settings
        super::platform_utils::configure_video_output();

        Ok(Self {
            app_state: None,
            is_running: false,
        })
    }

    pub fn initialize(&mut self, config: Arc<tokio::sync::RwLock<Config>>) -> Result<()> {
        info!("Initializing Slint frontend");

        // Initialize application state with shared config
        let app_state = Arc::new(AppState::new(config)?);
        self.app_state = Some(app_state);

        Ok(())
    }

    pub fn run(&mut self) -> Result<i32> {
        info!("Starting Slint frontend with native theming");

        if self.app_state.is_none() {
            return Err(anyhow::anyhow!(
                "App state not initialized. Call initialize() first."
            ));
        }

        self.is_running = true;

        // Ensure native styling is used at runtime
        // This will make the UI look native to the platform (Windows, macOS, Linux)
        unsafe {
            std::env::set_var("SLINT_STYLE", "native");
        }

        // Create the main window
        let main_window = AppWindow::new()?;

        // Set initial library name
        main_window.set_current_library_name("All Media".into());

        // Set up callbacks for desktop navigation
        let main_window_weak = main_window.as_weak();
        main_window.on_show_sources(move || {
            info!("Show sources page callback triggered");
            if let Some(window) = main_window_weak.upgrade() {
                window.set_current_library_name("Sources".into());
                // TODO: Implement sources page navigation
            }
        });

        let main_window_weak = main_window.as_weak();
        main_window.on_show_home(move || {
            info!("Show home page callback triggered");
            if let Some(window) = main_window_weak.upgrade() {
                window.set_current_library_name("Home".into());
                // TODO: Implement home page content loading
            }
        });

        let main_window_weak = main_window.as_weak();
        main_window.on_show_library(move |library_id| {
            info!("Show library callback triggered for: {}", library_id);
            if let Some(window) = main_window_weak.upgrade() {
                // Parse library_id to get source and library info
                let parts: Vec<&str> = library_id.as_str().splitn(2, ':').collect();
                if parts.len() == 2 {
                    let (_source_name, lib_name) = (parts[0], parts[1]);
                    window.set_current_library_name(lib_name.into());
                    // TODO: Implement library content loading
                } else {
                    window.set_current_library_name(library_id);
                }
            }
        });

        let main_window_weak = main_window.as_weak();
        main_window.on_search(move |query| {
            info!("Search callback triggered with query: {}", query);
            // TODO: Implement search functionality
        });

        let main_window_weak = main_window.as_weak();
        main_window.on_toggle_view(move || {
            info!("Toggle view callback triggered");
            // TODO: Implement view toggle
        });

        let main_window_weak = main_window.as_weak();
        main_window.on_select_media(move |media_id| {
            info!("Select media callback triggered for: {}", media_id);
            // TODO: Implement media selection
        });

        // Show the window
        main_window.show()?;

        info!("Running Slint event loop");

        // Run the event loop
        slint::run_event_loop()?;

        info!("Slint event loop finished");

        self.is_running = false;
        Ok(0)
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down Slint frontend");
        self.is_running = false;
        slint::quit_event_loop();
        Ok(())
    }
}
