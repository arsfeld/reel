use super::components::MainWindow;
use crate::db::{Database, DatabaseConnection};
use gtk::gio;
use libadwaita as adw;
use libadwaita::prelude::*;
use relm4::gtk;
use relm4::prelude::*;
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
        // Set global CSS
        relm4::set_global_css(
            r#"
            /* Typography Classes */
            .title-1 {
                font-size: 32pt;
                font-weight: 800;
                line-height: 1.1;
            }
            .title-2 {
                font-size: 24pt;
                font-weight: 700;
                line-height: 1.2;
            }
            .title-3 {
                font-size: 18pt;
                font-weight: 600;
                line-height: 1.2;
            }
            .heading {
                font-size: 15pt;
                font-weight: 600;
                line-height: 1.3;
            }
            .body {
                font-size: 11pt;
                line-height: 1.4;
            }
            .caption {
                font-size: 9pt;
                line-height: 1.3;
            }
            .dim-label {
                opacity: 0.55;
            }

            /* Media Cards */
            .media-card {
                background: var(--card-bg-color);
                border-radius: 8px;
                box-shadow: 0 1px 3px rgba(0,0,0,0.12);
                padding: 12px;
                margin: 6px;
                transition: all 0.2s ease;
            }
            .media-card:hover {
                transform: translateY(-2px);
                box-shadow: 0 4px 12px rgba(0,0,0,0.15);
            }

            /* Progress indicators */
            .progress-bar {
                background: var(--accent-color);
                border-radius: 2px;
                min-height: 4px;
            }

            /* Player styles */
            .video-area {
                background-color: black;
            }
            .fullscreen .video-area {
                background-color: black;
                padding: 0;
            }

            /* OSD Controls */
            .osd {
                background: linear-gradient(to top, rgba(0,0,0,0.8), transparent);
                border-radius: 12px;
                padding: 18px;
                margin: 12px;
            }
            .osd.pill {
                background: rgba(0,0,0,0.75);
                border-radius: 999px;
                padding: 12px 24px;
            }
            .osd scale {
                min-width: 200px;
            }
            .osd button {
                background: rgba(255,255,255,0.1);
                border: none;
                color: white;
                min-width: 48px;
                min-height: 48px;
            }
            .osd button:hover {
                background: rgba(255,255,255,0.2);
            }

            /* Navigation styles */
            .navigation-sidebar {
                background: transparent;
                padding: 6px;
            }
            .navigation-sidebar row {
                border-radius: 6px;
                padding: 8px;
                margin: 2px 0;
            }
            .navigation-sidebar row:selected {
                background: var(--accent-bg-color);
                color: var(--accent-fg-color);
            }
            .navigation-sidebar row:hover:not(:selected) {
                background: alpha(currentColor, 0.07);
            }

            /* Card overlays */
            .poster-overlay {
                background: linear-gradient(to top, rgba(0,0,0,0.8), transparent);
                padding: 12px;
            }

            /* Episode cards */
            .episode-card {
                border-radius: 8px;
                background: var(--card-bg-color);
                padding: 8px;
                transition: all 0.2s ease;
            }
            .episode-card:hover {
                transform: scale(1.02);
                box-shadow: 0 4px 12px rgba(0,0,0,0.15);
            }

            /* Pills and badges */
            .pill {
                border-radius: 999px;
                padding: 6px 12px;
                font-weight: 500;
            }

            /* Loading states */
            @keyframes pulse {
                0% { opacity: 0.6; }
                50% { opacity: 1.0; }
                100% { opacity: 0.6; }
            }
            .loading {
                animation: pulse 1.5s ease-in-out infinite;
            }
            "#,
        );

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
