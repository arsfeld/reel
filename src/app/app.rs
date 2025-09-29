use crate::db::Database;
use crate::ui::MainWindow;
use libadwaita as adw;
use std::sync::Arc;
use tokio::runtime::Runtime;

pub struct ReelApp {
    runtime: Arc<Runtime>,
}

impl ReelApp {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self { runtime }
    }

    pub fn run(self) -> anyhow::Result<()> {
        // Initialize ConfigService early before UI components
        tracing::info!("Initializing ConfigService at application startup");
        let config_service = crate::services::config_service::config_service();

        // Load initial config synchronously to ensure it's ready
        let initial_config = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(config_service.get_config());
        tracing::info!(
            "ConfigService initialized with player backend: {}",
            initial_config.playback.player_backend
        );

        // Initialize cache service after config is ready
        tracing::info!("Initializing file cache service");
        // Use the existing runtime from self to ensure spawned tasks persist
        self.runtime.block_on(async {
            if let Err(e) = crate::services::cache_service::initialize_cache_service().await {
                tracing::warn!("Failed to initialize cache service: {}", e);
                tracing::warn!("Application will continue without file caching");
            }
        });

        // Force dark theme - no user preference
        let style_manager = adw::StyleManager::default();
        style_manager.set_color_scheme(adw::ColorScheme::ForceDark);

        // Load CSS files (no macOS-specific CSS)
        let base_css = include_str!("../styles/base.css");
        let details_css = include_str!("../styles/details.css");
        let sidebar_css = include_str!("../styles/sidebar.css");

        tracing::info!("Loading standard CSS styles");
        let combined_css = format!("{}{}{}", base_css, details_css, sidebar_css);
        relm4::set_global_css(&combined_css);

        // Initialize database in a blocking context first
        let db = tokio::runtime::Runtime::new().unwrap().block_on(async {
            let database = Database::new()
                .await
                .expect("Failed to initialize database");

            // Run database migrations
            database
                .migrate()
                .await
                .expect("Failed to run database migrations");

            database.get_connection()
        });

        // Create the Relm4 application and run it
        let app = relm4::RelmApp::new("com.github.reel");
        let main_window_init = (db, self.runtime.clone());
        app.with_args(vec![])
            .run_async::<MainWindow>(main_window_init);

        Ok(())
    }
}
