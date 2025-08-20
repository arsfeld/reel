use gtk4::{gio, glib, glib::clone, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{error, info};

use crate::backends::{
    BackendManager,
    traits::{BackendInfo, BackendType},
};
use crate::state::AppState;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct PreferencesWindow {
        pub state: RefCell<Option<Arc<AppState>>>,
        pub backends_group: RefCell<Option<adw::PreferencesGroup>>,
        pub backends_list: RefCell<Option<gtk4::ListBox>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PreferencesWindow {
        const NAME: &'static str = "ReelPreferencesWindow";
        type Type = super::PreferencesWindow;
        type ParentType = adw::PreferencesWindow;
    }

    impl ObjectImpl for PreferencesWindow {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().setup_ui();
        }
    }

    impl WidgetImpl for PreferencesWindow {}
    impl WindowImpl for PreferencesWindow {}
    impl adw::subclass::window::AdwWindowImpl for PreferencesWindow {}
    impl adw::subclass::preferences_window::PreferencesWindowImpl for PreferencesWindow {}
}

glib::wrapper! {
    pub struct PreferencesWindow(ObjectSubclass<imp::PreferencesWindow>)
        @extends gtk4::Widget, gtk4::Window, adw::Window, adw::PreferencesWindow,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
}

impl PreferencesWindow {
    pub fn new(parent: &impl IsA<gtk4::Window>, state: Arc<AppState>) -> Self {
        let window: Self = glib::Object::builder()
            .property("title", "Preferences")
            .property("modal", true)
            .property("transient-for", parent)
            .property("default-width", 600)
            .property("default-height", 500)
            .build();

        window.imp().state.replace(Some(state));
        window.load_backends();

        window
    }

    fn setup_ui(&self) {
        let imp = self.imp();

        // Create General page
        let general_page = adw::PreferencesPage::builder()
            .title("General")
            .icon_name("applications-system-symbolic")
            .build();

        let appearance_group = adw::PreferencesGroup::builder().title("Appearance").build();

        let theme_row = adw::ComboRow::builder()
            .title("Theme")
            .model(&gtk4::StringList::new(&["System", "Light", "Dark"]))
            .build();

        appearance_group.add(&theme_row);
        general_page.add(&appearance_group);

        // Create Backends page
        let backends_page = adw::PreferencesPage::builder()
            .title("Backends")
            .icon_name("network-server-symbolic")
            .build();

        let backends_group = adw::PreferencesGroup::builder()
            .title("Media Sources")
            .description("Manage your media server connections")
            .build();

        // Add button in header
        let add_button = gtk4::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("Add Backend")
            .css_classes(vec!["flat"])
            .build();

        backends_group.set_header_suffix(Some(&add_button));

        // Create list box for backends
        let backends_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(vec!["boxed-list"])
            .build();

        backends_group.add(&backends_list);
        backends_page.add(&backends_group);

        // Store references
        imp.backends_group.replace(Some(backends_group));
        imp.backends_list.replace(Some(backends_list.clone()));

        // Add pages to window
        self.add(&general_page);
        self.add(&backends_page);

        // Connect signals
        add_button.connect_clicked(clone!(
            #[weak(rename_to = window)]
            self,
            move |_| {
                window.show_add_backend_dialog();
            }
        ));

        // Theme row handler
        theme_row.connect_selected_notify(clone!(
            #[weak(rename_to = window)]
            self,
            move |row| {
                let selected = row.selected();
                let theme = match selected {
                    0 => "auto",
                    1 => "light",
                    2 => "dark",
                    _ => "auto",
                };
                window.apply_theme(theme);
            }
        ));
    }

    fn load_backends(&self) {
        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let window_weak = self.downgrade();

        glib::spawn_future_local(async move {
            if let Some(window) = window_weak.upgrade() {
                let backend_manager = state.backend_manager.read().await;
                window.update_backend_list(&backend_manager);
            }
        });
    }

    fn update_backend_list(&self, backend_manager: &BackendManager) {
        let imp = self.imp();

        if let Some(list) = imp.backends_list.borrow().as_ref() {
            // Clear existing items
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }

            // Get all registered backends
            let backends = backend_manager.list_backends();

            if backends.is_empty() {
                // Show empty state
                let empty_row = adw::ActionRow::builder()
                    .title("No backends configured")
                    .subtitle("Add a backend to start browsing your media")
                    .sensitive(false)
                    .build();

                list.append(&empty_row);
            } else {
                // Add a row for each backend
                for (backend_id, backend_info) in backends {
                    let row = self.create_backend_row(&backend_id, &backend_info);
                    list.append(&row);
                }
            }
        }
    }

    fn create_backend_row(&self, backend_id: &str, info: &BackendInfo) -> adw::ActionRow {
        let row = adw::ActionRow::builder()
            .title(&info.display_name)
            .activatable(false)
            .build();

        // Add subtitle with server info
        if let Some(server_name) = &info.server_name {
            row.set_subtitle(&format!("{} - {}", info.backend_type, server_name));
        } else {
            row.set_subtitle(&format!("{}", info.backend_type));
        }

        // Add type icon
        let icon_name = match info.backend_type {
            BackendType::Plex => "network-server-symbolic",
            BackendType::Jellyfin => "network-workgroup-symbolic",
            BackendType::Local => "folder-symbolic",
            BackendType::Generic => "application-x-executable-symbolic",
        };

        let icon = gtk4::Image::from_icon_name(icon_name);
        row.add_prefix(&icon);

        // Add connection status indicator
        let status_icon = gtk4::Image::from_icon_name(if info.is_local {
            "network-wired-symbolic"
        } else if info.is_relay {
            "network-cellular-symbolic"
        } else {
            "network-wireless-symbolic"
        });
        status_icon.set_tooltip_text(Some(if info.is_local {
            "Local connection"
        } else if info.is_relay {
            "Relay connection"
        } else {
            "Remote connection"
        }));
        row.add_suffix(&status_icon);

        // Add remove button
        let remove_button = gtk4::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Remove Backend")
            .css_classes(vec!["flat", "circular"])
            .valign(gtk4::Align::Center)
            .build();

        let backend_id = backend_id.to_string();
        let window_weak = self.downgrade();
        remove_button.connect_clicked(move |_| {
            if let Some(window) = window_weak.upgrade() {
                window.confirm_remove_backend(&backend_id);
            }
        });

        row.add_suffix(&remove_button);

        row
    }

    fn show_add_backend_dialog(&self) {
        let dialog = adw::MessageDialog::builder()
            .title("Add Backend")
            .body("Choose the type of media source to add")
            .modal(true)
            .transient_for(self)
            .build();

        dialog.add_response("cancel", "Cancel");
        dialog.add_response("plex", "Plex");
        dialog.add_response("jellyfin", "Jellyfin");
        dialog.add_response("local", "Local Files");

        dialog.set_response_appearance("plex", adw::ResponseAppearance::Suggested);

        let window_weak = self.downgrade();
        dialog.connect_response(None, move |_, response| {
            if let Some(window) = window_weak.upgrade() {
                match response {
                    "plex" => window.add_plex_backend(),
                    "jellyfin" => window.add_jellyfin_backend(),
                    "local" => window.add_local_backend(),
                    _ => {}
                }
            }
        });

        dialog.present();
    }

    fn add_plex_backend(&self) {
        info!("Adding Plex backend");

        // Show the auth dialog for Plex
        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let auth_dialog = crate::ui::AuthDialog::new(state);
        auth_dialog.present(Some(self));
        auth_dialog.start_auth();

        // Refresh the list when dialog closes
        let window_weak = self.downgrade();
        auth_dialog.connect_closed(move |_| {
            if let Some(window) = window_weak.upgrade() {
                window.load_backends();
            }
        });
    }

    fn add_jellyfin_backend(&self) {
        info!("Adding Jellyfin backend");

        // Create a simple dialog for Jellyfin server URL and credentials
        let dialog = adw::MessageDialog::builder()
            .title("Add Jellyfin Server")
            .body("Jellyfin backend support is coming soon")
            .modal(true)
            .transient_for(self)
            .build();

        dialog.add_response("ok", "OK");
        dialog.present();
    }

    fn add_local_backend(&self) {
        info!("Adding Local Files backend");

        // Create a file chooser for selecting media directories
        let dialog = gtk4::FileDialog::builder()
            .title("Choose Media Directory")
            .modal(true)
            .build();

        let window_weak = self.downgrade();
        dialog.select_folder(Some(self), gio::Cancellable::NONE, move |result| {
            if let Ok(folder) = result
                && let Some(window) = window_weak.upgrade()
                && let Some(path) = folder.path()
            {
                info!("Selected folder: {:?}", path);
                // TODO: Implement local backend registration
                window.load_backends();
            }
        });
    }

    fn confirm_remove_backend(&self, backend_id: &str) {
        let dialog = adw::MessageDialog::builder()
            .title("Remove Backend")
            .body("Are you sure you want to remove this backend?\n\nThis will remove all cached data for this backend.".to_string())
            .modal(true)
            .transient_for(self)
            .build();

        dialog.add_response("cancel", "Cancel");
        dialog.add_response("remove", "Remove");
        dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);

        let backend_id = backend_id.to_string();
        let window_weak = self.downgrade();
        dialog.connect_response(None, move |_, response| {
            if response == "remove"
                && let Some(window) = window_weak.upgrade()
            {
                window.remove_backend(&backend_id);
            }
        });

        dialog.present();
    }

    fn remove_backend(&self, backend_id: &str) {
        info!("Removing backend: {}", backend_id);

        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let backend_id = backend_id.to_string();
        let window_weak = self.downgrade();

        glib::spawn_future_local(async move {
            let mut backend_manager = state.backend_manager.write().await;

            // Remove the backend
            backend_manager.unregister_backend(&backend_id);

            // Clear cached data for this backend
            let cache = state.cache_manager.clone();
            if let Err(e) = cache.clear_backend_cache(&backend_id).await {
                error!("Failed to clear cache for backend {}: {}", backend_id, e);
            }

            // If this was the last active backend, clear it from config
            let config = state.config.clone();
            if config.get_last_active_backend() == Some(backend_id.clone()) {
                let mut config = state.config.as_ref().clone();
                let _ = config.set_last_active_backend("");
            }

            drop(backend_manager); // Release write lock

            // Reload the backend list
            if let Some(window) = window_weak.upgrade() {
                window.load_backends();
            }
        });
    }

    fn apply_theme(&self, theme: &str) {
        info!("Applying theme: {}", theme);

        let style_manager = adw::StyleManager::default();
        match theme {
            "light" => style_manager.set_color_scheme(adw::ColorScheme::ForceLight),
            "dark" => style_manager.set_color_scheme(adw::ColorScheme::ForceDark),
            _ => style_manager.set_color_scheme(adw::ColorScheme::PreferDark),
        }

        // Save theme preference
        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let mut config = state.config.as_ref().clone();
        config.general.theme = theme.to_string();
        if let Err(e) = config.save() {
            error!("Failed to save theme preference: {}", e);
        }
    }
}
