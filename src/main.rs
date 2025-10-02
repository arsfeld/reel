use anyhow::Result;

mod app;
mod backends;
mod cache;
mod config;
mod constants;
mod core;
mod db;
mod mapper;
mod models;
mod player;
mod services;
// State module removed in Relm4 migration - components manage their own state
// mod state;
mod ui;
mod utils;
mod workers;

#[cfg(test)]
mod test_utils;

fn main() -> Result<()> {
    use app::AppPlatform;
    use std::sync::Arc;
    use tracing::info;

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("reel=debug")
        .init();

    info!("Starting Reel application");

    // Initialize GTK and Adwaita first
    gtk4::init()?;
    libadwaita::init()?;

    // Configure icon theme on macOS
    // GTK4 on macOS doesn't automatically use XDG_DATA_DIRS for icon discovery,
    // so we need to explicitly add icon search paths to the IconTheme
    #[cfg(target_os = "macos")]
    {
        use gtk4::prelude::*;
        use std::path::PathBuf;

        info!("Configuring icon theme for macOS");

        // Get the default icon theme for the display
        if let Some(display) = gtk4::gdk::Display::default() {
            let icon_theme = gtk4::IconTheme::for_display(&display);

            // Parse XDG_DATA_DIRS and add icon paths explicitly
            if let Ok(xdg_data_dirs) = std::env::var("XDG_DATA_DIRS") {
                info!("XDG_DATA_DIRS found, adding icon search paths");
                let mut paths_added = 0;
                for dir in xdg_data_dirs.split(':') {
                    if dir.is_empty() {
                        continue;
                    }
                    let icons_path = PathBuf::from(dir).join("icons");
                    if icons_path.exists() {
                        info!("  Adding icon search path: {:?}", icons_path);
                        icon_theme.add_search_path(&icons_path);
                        paths_added += 1;
                    }
                }
                info!("Total icon search paths added: {}", paths_added);
            } else {
                info!("XDG_DATA_DIRS not set, icon loading may not work correctly");
            }

            // Set the icon theme name
            info!("Setting icon theme to: WhiteSur-dark");
            icon_theme.set_theme_name(Some("WhiteSur-dark"));

            // Log current search paths for debugging
            let search_paths = icon_theme.search_path();
            info!("Icon search paths configured: {} paths", search_paths.len());
            for (i, path) in search_paths.iter().take(10).enumerate() {
                if let Some(path_str) = path.to_str() {
                    info!("  [{}] {}", i, path_str);
                }
            }

            // Test icon lookup to verify icons can be found
            let test_icons = vec![
                "go-previous-symbolic",
                "media-playback-start-symbolic",
                "folder-symbolic",
                "sidebar-show-symbolic",
                "user-home-symbolic",
                "dialog-error-symbolic",
            ];
            info!("Testing icon lookups:");
            for icon_name in test_icons {
                if icon_theme.has_icon(icon_name) {
                    info!("  ✓ Found: {}", icon_name);
                    // Try to lookup the icon and get its path
                    let icon_paintable = icon_theme.lookup_icon(
                        icon_name,
                        &[],
                        16,
                        1,
                        gtk4::TextDirection::Ltr,
                        gtk4::IconLookupFlags::empty(),
                    );
                    if let Some(file) = icon_paintable.file() {
                        if let Some(path) = file.path() {
                            info!("    Path: {:?}", path);
                        }
                    }
                } else {
                    info!("  ✗ NOT FOUND: {}", icon_name);
                }
            }
        } else {
            info!("Failed to get default display for icon theme configuration");
        }

        // Also set via GtkSettings for compatibility
        if let Some(settings) = gtk4::Settings::default() {
            settings.set_property("gtk-icon-theme-name", "WhiteSur-dark");
            let theme: String = settings.property("gtk-icon-theme-name");
            info!("GtkSettings icon theme set to: {}", theme);
        }
    }

    // Initialize GStreamer (if available)
    #[cfg(feature = "gstreamer")]
    gstreamer::init()?;

    // Initialize Tokio runtime for async operations
    let runtime = Arc::new(tokio::runtime::Runtime::new()?);

    // Run the appropriate platform implementation
    AppPlatform::run_relm4(runtime)?;

    Ok(())
}
