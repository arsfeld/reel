use gtk::glib;
use gtk::prelude::*;
use libadwaita as adw;
use relm4::gtk;

/// Sync status states for UI representation
#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    /// No sync in progress
    Idle,
    /// Sync in progress
    Syncing { count: usize },
    /// Sync completed successfully
    Synced { count: usize },
    /// Sync failed
    Failed { error: String },
}

/// Create a sync status indicator widget
/// Returns a gtk::Box containing an icon and optional label
pub fn create_sync_status_indicator(status: &SyncStatus, show_label: bool) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    container.add_css_class("sync-status-indicator");

    match status {
        SyncStatus::Idle => {
            // Don't show anything when idle
            container.set_visible(false);
        }
        SyncStatus::Syncing { count } => {
            container.add_css_class("syncing");

            // Spinner icon
            let spinner = gtk::Spinner::new();
            spinner.set_spinning(true);
            spinner.set_size_request(16, 16);
            container.append(&spinner);

            if show_label {
                let label = gtk::Label::new(Some(&format!("Syncing {}...", count)));
                label.add_css_class("dim-label");
                label.add_css_class("caption");
                container.append(&label);
            }
        }
        SyncStatus::Synced { count } => {
            container.add_css_class("synced");

            // Checkmark icon
            let icon = gtk::Image::from_icon_name("emblem-ok-symbolic");
            icon.set_icon_size(gtk::IconSize::Inherit);
            icon.set_pixel_size(16);
            icon.add_css_class("success");
            container.append(&icon);

            if show_label {
                let label = gtk::Label::new(Some(&format!("{} synced", count)));
                label.add_css_class("dim-label");
                label.add_css_class("caption");
                label.add_css_class("success");
                container.append(&label);
            }

            // Auto-hide after 3 seconds
            glib::timeout_add_seconds_local_once(3, {
                let container = container.clone();
                move || {
                    container.set_visible(false);
                }
            });
        }
        SyncStatus::Failed { error } => {
            container.add_css_class("failed");

            // Warning icon
            let icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
            icon.set_icon_size(gtk::IconSize::Inherit);
            icon.set_pixel_size(16);
            icon.add_css_class("error");
            container.append(&icon);

            if show_label {
                let label = gtk::Label::new(Some(&format!("Sync failed: {}", error)));
                label.add_css_class("dim-label");
                label.add_css_class("caption");
                label.add_css_class("error");
                label.set_max_width_chars(30);
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                container.append(&label);
            }
        }
    }

    container
}

/// Create a simple status icon (no label)
pub fn create_sync_status_icon(status: &SyncStatus) -> gtk::Widget {
    match status {
        SyncStatus::Idle => {
            let placeholder = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            placeholder.set_visible(false);
            placeholder.upcast()
        }
        SyncStatus::Syncing { .. } => {
            let spinner = gtk::Spinner::new();
            spinner.set_spinning(true);
            spinner.set_size_request(16, 16);
            spinner.set_tooltip_text(Some("Syncing..."));
            spinner.upcast()
        }
        SyncStatus::Synced { .. } => {
            let icon = gtk::Image::from_icon_name("emblem-ok-symbolic");
            icon.set_icon_size(gtk::IconSize::Inherit);
            icon.set_pixel_size(16);
            icon.add_css_class("success");
            icon.set_tooltip_text(Some("Synced"));

            // Auto-hide after 3 seconds
            glib::timeout_add_seconds_local_once(3, {
                let icon = icon.clone();
                move || {
                    icon.set_visible(false);
                }
            });

            icon.upcast()
        }
        SyncStatus::Failed { error } => {
            let icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
            icon.set_icon_size(gtk::IconSize::Inherit);
            icon.set_pixel_size(16);
            icon.add_css_class("error");
            icon.set_tooltip_text(Some(&format!("Sync failed: {}", error)));
            icon.upcast()
        }
    }
}

/// Helper to determine if a media item's sync failed
pub fn is_item_sync_failed(
    media_item_id: &str,
    failed_items: &[(String, String)],
) -> Option<String> {
    failed_items
        .iter()
        .find(|(id, _)| id == media_item_id)
        .map(|(_, error)| error.clone())
}
