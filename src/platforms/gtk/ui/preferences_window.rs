use gtk4::{glib, glib::clone, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{error, info};

use crate::config::Config;
use crate::events::{
    event_bus::EventBus,
    types::{DatabaseEvent, EventPayload, EventType},
};
use tokio::sync::RwLock;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct PreferencesWindow {
        pub config: RefCell<Option<Arc<RwLock<Config>>>>,
        pub event_bus: RefCell<Option<Arc<EventBus>>>,
    }

    impl std::fmt::Debug for PreferencesWindow {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("PreferencesWindow")
                .field("config", &"Arc<RwLock<Config>>")
                .field("event_bus", &"Arc<EventBus>")
                .finish()
        }
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
    pub fn new(
        parent: &impl IsA<gtk4::Window>,
        config: Arc<RwLock<Config>>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        let window: Self = glib::Object::builder()
            .property("title", "Preferences")
            .property("modal", true)
            .property("transient-for", parent)
            .property("default-width", 600)
            .property("default-height", 500)
            .build();

        window.imp().config.replace(Some(config));
        window.imp().event_bus.replace(Some(event_bus));
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

        // Create Playback group
        let playback_group = adw::PreferencesGroup::builder().title("Playback").build();

        let player_backend_row = adw::ComboRow::builder()
            .title("Player Backend")
            .subtitle("Choose between GStreamer and MPV for media playback")
            .model(&gtk4::StringList::new(&["GStreamer", "MPV"]))
            .build();

        // Set current selections based on shared config
        if let Some(config_arc) = self.imp().config.borrow().as_ref() {
            // Read config synchronously using block_on to avoid timing issues
            let config = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(config_arc.read())
            });

            // Set theme selection
            let theme_index = match config.general.theme.as_str() {
                "light" => 1,
                "dark" => 2,
                _ => 0, // Default to System/auto
            };
            theme_row.set_selected(theme_index);

            // Set player backend selection (case-insensitive comparison)
            let backend = config.playback.player_backend.to_lowercase();
            info!(
                "Current player backend in config: '{}' (original: '{}')",
                backend, config.playback.player_backend
            );

            let selected_index = if backend == "gstreamer" {
                0
            } else {
                1 // Default to MPV for "mpv" or any other value (since MPV is the default)
            };
            info!("Setting player backend combo to index: {}", selected_index);
            player_backend_row.set_selected(selected_index);
        }

        playback_group.add(&player_backend_row);
        general_page.add(&playback_group);

        // Add page to window
        self.add(&general_page);

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

        // Player backend row handler
        player_backend_row.connect_selected_notify(clone!(
            #[weak(rename_to = window)]
            self,
            move |row| {
                let selected = row.selected();
                let backend = match selected {
                    1 => "mpv",
                    _ => "gstreamer",
                };
                window.apply_player_backend(backend);
            }
        ));
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
        if let Some(config_arc) = self.imp().config.borrow().as_ref() {
            let config_arc = config_arc.clone();
            let event_bus = self.imp().event_bus.borrow().as_ref().cloned();
            let theme = theme.to_string();
            glib::spawn_future_local(async move {
                let mut config = config_arc.write().await;
                let old_theme = config.general.theme.clone();
                config.general.theme = theme.clone();
                if let Err(e) = config.save() {
                    error!("Failed to save theme preference: {}", e);
                } else if old_theme != theme {
                    // Emit UserPreferencesChanged event
                    if let Some(bus) = event_bus {
                        let event = DatabaseEvent::new(
                            EventType::UserPreferencesChanged,
                            EventPayload::User {
                                user_id: "local_user".to_string(),
                                action: format!("theme_changed_to_{}", theme),
                            },
                        );

                        if let Err(e) = bus.publish(event).await {
                            tracing::warn!("Failed to publish UserPreferencesChanged event: {}", e);
                        }
                    }
                }
            });
        }
    }

    fn apply_player_backend(&self, backend: &str) {
        info!("Applying player backend: {}", backend);

        // Save player backend preference
        if let Some(config_arc) = self.imp().config.borrow().as_ref() {
            let config_arc = config_arc.clone();
            let event_bus = self.imp().event_bus.borrow().as_ref().cloned();
            let backend_str = backend.to_string();
            let window_weak = self.downgrade();

            glib::spawn_future_local(async move {
                let mut config = config_arc.write().await;
                let old_backend = config.playback.player_backend.clone();
                config.playback.player_backend = backend_str.clone();

                if let Err(e) = config.save() {
                    error!("Failed to save player backend preference: {}", e);
                    return;
                }

                // Only notify if the backend actually changed
                if old_backend != backend_str {
                    // Emit UserPreferencesChanged event
                    if let Some(bus) = event_bus {
                        let event = DatabaseEvent::new(
                            EventType::UserPreferencesChanged,
                            EventPayload::User {
                                user_id: "local_user".to_string(),
                                action: format!("player_backend_changed_to_{}", backend_str),
                            },
                        );

                        if let Err(e) = bus.publish(event).await {
                            tracing::warn!("Failed to publish UserPreferencesChanged event: {}", e);
                        }
                    }
                    info!(
                        "Player backend changed from '{}' to '{}'",
                        old_backend, backend_str
                    );

                    // Show a toast notification
                    if let Some(window) = window_weak.upgrade()
                        && let Some(parent) = window
                            .transient_for()
                            .and_then(|w| w.downcast::<adw::ApplicationWindow>().ok())
                    {
                        let toast = adw::Toast::builder()
                            .title(format!(
                                "Player backend changed to {}",
                                if backend_str == "mpv" {
                                    "MPV"
                                } else {
                                    "GStreamer"
                                }
                            ))
                            .timeout(3)
                            .build();

                        // Find the toast overlay in the main window
                        if let Some(content) = parent
                            .content()
                            .and_then(|w| w.downcast::<adw::ToastOverlay>().ok())
                        {
                            content.add_toast(toast);
                        } else {
                            info!("Player backend will be used for next playback");
                        }
                    }
                }
            });
        }
    }
}
