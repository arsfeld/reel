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
        // Force dark theme - no user preference
        let style_manager = adw::StyleManager::default();
        style_manager.set_color_scheme(adw::ColorScheme::ForceDark);

        // Load details page CSS
        let details_css = include_str!("styles/details.css");

        // Set global CSS matching GTK library styles exactly
        let base_css = r#"
            /* Typography Classes from GTK */
            .title-1 {
                font-size: 32pt;
                font-weight: 800;
                line-height: 1.1;
                text-shadow: 0 2px 8px rgba(0, 0, 0, 0.8),
                             0 1px 3px rgba(0, 0, 0, 0.9);
            }
            .title-2 {
                font-size: 24pt;
                font-weight: 700;
                line-height: 1.2;
                color: rgba(255, 255, 255, 0.7);
                font-size: 14px;
                font-weight: 600;
                text-transform: uppercase;
                letter-spacing: 0.5px;
            }
            .title-3 {
                font-size: 18pt;
                font-weight: 600;
                line-height: 1.2;
            }
            .title-4 {
                color: white;
                font-weight: 600;
                text-shadow: 0 1px 2px rgba(0, 0, 0, 0.7);
                font-size: 0.85em;
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
            .caption.bold {
                font-weight: bold;
            }
            .subtitle {
                color: rgba(255, 255, 255, 0.85);
                font-size: 0.75em;
                text-shadow: 0 1px 2px rgba(0, 0, 0, 0.7);
            }
            .dim-label {
                opacity: 0.7;
                color: rgba(255, 255, 255, 0.7);
                font-size: 14px;
            }

            /* Card Base Styles */
            .card {
                border-radius: 8px;
                background-color: @card_bg_color;
                box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12);
            }

            /* Movie Poster Card Styles - GTK Library Exact Match */
            .poster-card {
                border-radius: 6px;
                transition: all 200ms cubic-bezier(0.4, 0, 0.2, 1);
                width: 180px;
                height: 270px;
                min-width: 180px;
                min-height: 270px;
                box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3),
                           0 4px 16px rgba(0, 0, 0, 0.15),
                           inset 0 1px 0 rgba(255, 255, 255, 0.05),
                           inset 0 -1px 0 rgba(0, 0, 0, 0.2);
            }

            .poster-card:hover {
                transform: scale(1.05) translateY(-3px);
                box-shadow: 0 8px 25px rgba(0, 0, 0, 0.4),
                           0 12px 35px rgba(0, 0, 0, 0.2),
                           inset 0 1px 0 rgba(255, 255, 255, 0.08),
                           inset 0 -1px 0 rgba(0, 0, 0, 0.3);
            }

            /* Poster Overlay Container */
            .poster-overlay {
                border-radius: 8px;
                box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15),
                            0 1px 3px rgba(0, 0, 0, 0.1);
                background: linear-gradient(to bottom,
                            rgba(255, 255, 255, 0.02) 0%,
                            transparent 50%,
                            rgba(0, 0, 0, 0.03) 100%);
                width: 180px;
                height: 270px;
                min-width: 180px;
                min-height: 270px;
            }

            /* Rounded Poster Image */
            .rounded-poster {
                border-radius: 6px;
                overflow: hidden;
            }

            /* Info Gradient at Bottom of Poster */
            .poster-info-gradient {
                background: linear-gradient(to top,
                            rgba(0, 0, 0, 0.95) 0%,
                            rgba(0, 0, 0, 0.7) 50%,
                            rgba(0, 0, 0, 0) 100%);
                padding: 4px 4px;
                padding-bottom: 4px;
                border-bottom-left-radius: 6px;
                border-bottom-right-radius: 6px;
                min-height: 30px;
                box-shadow: inset 0 -1px 2px rgba(0, 0, 0, 0.4);
            }

            /* Text on Poster */
            .poster-info-gradient label.title-4 {
                color: white;
                font-weight: 600;
                text-shadow: 0 1px 2px rgba(0, 0, 0, 0.7);
                font-size: 0.85em;
            }

            .poster-info-gradient label.subtitle {
                color: rgba(255, 255, 255, 0.85);
                font-size: 0.75em;
                text-shadow: 0 1px 2px rgba(0, 0, 0, 0.7);
            }

            /* Loading Spinner Styling */
            .poster-card spinner {
                background: rgba(0, 0, 0, 0.5);
                border-radius: 50%;
                padding: 3px;
            }

            /* Poster Background Gradient */
            .poster-card picture {
                background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
                min-width: 75px;
                min-height: 112px;
                box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.3),
                           inset 0 -1px 1px rgba(255, 255, 255, 0.02);
            }

            /* Skeleton loading placeholder */
            .poster-skeleton {
                background: linear-gradient(90deg,
                    rgba(255, 255, 255, 0.05) 0%,
                    rgba(255, 255, 255, 0.1) 50%,
                    rgba(255, 255, 255, 0.05) 100%);
                background-size: 200% 100%;
                animation: skeleton-shimmer 1.5s ease-in-out infinite;
                box-shadow: inset 0 1px 3px rgba(0, 0, 0, 0.2);
            }

            @keyframes skeleton-shimmer {
                0% { background-position: -200% 0; }
                100% { background-position: 200% 0; }
            }

            /* Smooth image fade-in */
            .poster-fade-in {
                animation: poster-fade 0.3s ease-in;
            }

            @keyframes poster-fade {
                from { opacity: 0; }
                to { opacity: 1; }
            }

            /* Flow Box Child Sizing */
            flowboxchild {
                width: 180px;
                height: 270px;
                min-width: 180px;
                min-height: 270px;
                max-width: 180px;
                max-height: 270px;
            }

            /* Badge Styles */
            .badge {
                background-color: alpha(@accent_color, 0.8);
                color: @accent_fg_color;
                border-radius: 4px;
                padding: 2px 6px;
                font-weight: bold;
            }

            .badge.small {
                font-size: 0.8em;
            }

            /* Watched Overlay */
            .watched-overlay {
                background: linear-gradient(to bottom, transparent, alpha(black, 0.6));
                border-radius: 0 0 8px 8px;
            }

            /* Unwatched Indicator */
            .unwatched-indicator {
                filter: drop-shadow(0 2px 6px alpha(black, 0.5));
            }

            .unwatched-glow-dot {
                background: radial-gradient(circle, #3584e4, #1c71d8);
                border-radius: 50%;
                border: 2px solid rgba(255, 255, 255, 0.9);
                box-shadow: 0 0 12px #3584e4,
                            0 0 24px #3584e4,
                            0 0 36px #1c71d8,
                            inset 0 0 10px rgba(255, 255, 255, 0.4);
            }

            /* Media Progress Bar */
            .media-progress {
                min-height: 3px;
                border-radius: 0 0 8px 8px;
            }

            /* Media Card Info Box */
            .media-card-info {
                padding: 0;
                margin: 0;
            }

            /* Flow Box in Library */
            flowbox {
                background: transparent;
            }

            /* Search Entry Styles */
            searchentry {
                border-radius: 6px;
                padding: 6px;
            }

            /* Dropdown Styles */
            dropdown {
                min-width: 120px;
            }

            dropdown button {
                padding: 4px 8px;
            }

            /* Header Bar Separator */
            headerbar separator {
                margin: 0 8px;
                opacity: 0.3;
            }

            /* Status Pages */
            statuspage > box {
                margin: 24px;
            }


            /* Navigation Sidebar */
            .navigation-sidebar {
                background: transparent;
                padding: 6px;
            }
            .navigation-sidebar listbox row:has(separator) {
                min-height: 0;
                padding: 0;
                background: transparent;
            }
            .navigation-sidebar listbox row separator {
                opacity: 0.15;
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

            /* Episode Cards */
            .episode-card {
                border-radius: 10px;
                background: transparent;
                transition: transform 200ms cubic-bezier(0.4, 0, 0.2, 1);
                overflow: visible;
                border: none;
                box-shadow: none;
            }
            .episode-card:hover {
                transform: translateY(-2px);
            }

            /* Pills and Badges */
            .pill {
                border-radius: 999px;
                padding: 6px 12px;
                font-weight: 500;
            }
            .pill.suggested-action {
                background: @accent_bg_color;
                color: @accent_fg_color;
                border: none;
                transition: all 200ms ease-in-out;
            }
            .pill.suggested-action:hover {
                background: shade(@accent_bg_color, 1.1);
                transform: translateY(-1px);
            }
            .pill:not(.suggested-action) {
                background: @card_bg_color;
                border: 1px solid alpha(@borders, 0.3);
                transition: all 200ms ease-in-out;
            }
            .pill:not(.suggested-action):hover {
                background: shade(@card_bg_color, 1.1);
                transform: translateY(-1px);
            }

            /* Loading States */
            @keyframes pulse {
                0% { opacity: 0.6; }
                50% { opacity: 1.0; }
                100% { opacity: 0.6; }
            }
            .loading {
                animation: pulse 1.5s ease-in-out infinite;
            }

            /* Player Styles */
            .video-area {
                background-color: black;
            }
            .fullscreen .video-area {
                background-color: black;
                padding: 0;
            }

            /* Hero Section Gradient Overlay */
            .hero-gradient {
                background: linear-gradient(to bottom,
                            transparent 0%,
                            transparent 30%,
                            rgba(0, 0, 0, 0.1) 40%,
                            rgba(0, 0, 0, 0.3) 50%,
                            rgba(0, 0, 0, 0.5) 60%,
                            rgba(0, 0, 0, 0.7) 70%,
                            rgba(0, 0, 0, 0.85) 80%,
                            rgba(0, 0, 0, 0.95) 90%,
                            rgba(0, 0, 0, 0.98) 100%);
                min-height: 100%;
            }

            /* Enhanced Poster Shadow for Details Page */
            .poster-shadow {
                box-shadow: 0 10px 40px rgba(0, 0, 0, 0.8),
                           0 5px 20px rgba(0, 0, 0, 0.6),
                           0 2px 8px rgba(0, 0, 0, 0.4),
                           inset 0 1px 0 rgba(255, 255, 255, 0.05);
            }

            /* Metadata Pills Styling */
            .metadata-pill {
                background: rgba(255, 255, 255, 0.1);
                backdrop-filter: blur(10px);
                border: 1px solid rgba(255, 255, 255, 0.1);
                border-radius: 999px;
                transition: all 200ms ease-in-out;
            }

            .metadata-pill:hover {
                background: rgba(255, 255, 255, 0.15);
                transform: translateY(-1px);
            }
            "#;

        // Combine base CSS with details CSS
        let combined_css = format!("{}{}", base_css, details_css);
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
