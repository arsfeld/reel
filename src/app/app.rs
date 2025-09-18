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
        // Force dark theme - no user preference
        let style_manager = adw::StyleManager::default();
        style_manager.set_color_scheme(adw::ColorScheme::ForceDark);

        // Load CSS files
        let base_css = include_str!("../styles/base.css");
        let details_css = include_str!("../styles/details.css");
        let sidebar_css = include_str!("../styles/sidebar.css");

        // Combine base CSS with details CSS and sidebar CSS
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
        app.with_args(vec![]).run_async::<MainWindow>(db);

        Ok(())
    }
}
