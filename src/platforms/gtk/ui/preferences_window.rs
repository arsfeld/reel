use gtk4::{glib, glib::clone, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::core::viewmodels::preferences_view_model::{PlayerBackend, ThemeOption};
use crate::core::viewmodels::{PreferencesViewModel, ViewModel};
use crate::events::event_bus::EventBus;
use tokio::sync::RwLock;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct PreferencesWindow {
        pub view_model: RefCell<Option<Arc<PreferencesViewModel>>>,
        // Theme toggle buttons
        pub theme_system_toggle: RefCell<Option<gtk4::ToggleButton>>,
        pub theme_light_toggle: RefCell<Option<gtk4::ToggleButton>>,
        pub theme_dark_toggle: RefCell<Option<gtk4::ToggleButton>>,
        // Player backend toggle buttons
        pub backend_gstreamer_toggle: RefCell<Option<gtk4::ToggleButton>>,
        pub backend_mpv_toggle: RefCell<Option<gtk4::ToggleButton>>,
    }

    impl std::fmt::Debug for PreferencesWindow {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("PreferencesWindow")
                .field("view_model", &"Option<Arc<PreferencesViewModel>>")
                .field("theme_toggles", &"ToggleButton group")
                .field("backend_toggles", &"ToggleButton group")
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
        info!("🔧 PreferencesWindow: Creating new preferences dialog");
        debug!("🔧 PreferencesWindow: Parent window provided");

        let window: Self = glib::Object::builder()
            .property("title", "Preferences")
            .property("modal", true)
            .property("transient-for", parent)
            .property("default-width", 600)
            .property("default-height", 500)
            .build();

        // Create ViewModel
        debug!("🔧 PreferencesWindow: Creating PreferencesViewModel");
        let view_model = Arc::new(PreferencesViewModel::new(config));
        window.imp().view_model.replace(Some(view_model.clone()));
        info!("✅ PreferencesWindow: ViewModel created and stored");

        // Initialize ViewModel
        let view_model_clone = view_model.clone();
        let window_weak = window.downgrade();
        debug!("🔧 PreferencesWindow: Starting ViewModel initialization task");
        glib::spawn_future_local(async move {
            view_model_clone.initialize(event_bus).await;
            info!("✅ PreferencesWindow: ViewModel initialized successfully");

            // Setup reactive subscriptions after initialization
            if let Some(window) = window_weak.upgrade() {
                debug!("🔧 PreferencesWindow: Setting up reactive subscriptions");
                window.setup_reactive_subscriptions();

                // Synchronize UI with ViewModel state after initialization
                window.sync_ui_with_viewmodel().await;

                info!("✅ PreferencesWindow: Reactive subscriptions established");
            } else {
                warn!(
                    "⚠️ PreferencesWindow: Window was destroyed before subscriptions could be set up"
                );
            }
        });

        info!("✅ PreferencesWindow: Preferences dialog created successfully");
        window
    }

    fn setup_ui(&self) {
        let imp = self.imp();

        // Create General page
        let general_page = adw::PreferencesPage::builder()
            .title("General")
            .icon_name("applications-system-symbolic")
            .build();

        // Theme selection group
        let appearance_group = adw::PreferencesGroup::builder().title("Appearance").build();

        // Create theme toggle buttons with proper styling and grouping
        let theme_system_toggle = gtk4::ToggleButton::builder()
            .label("System")
            .valign(gtk4::Align::Center)
            .vexpand(false)
            .build();
        let theme_light_toggle = gtk4::ToggleButton::builder()
            .label("Light")
            .valign(gtk4::Align::Center)
            .vexpand(false)
            .group(&theme_system_toggle)
            .build();
        let theme_dark_toggle = gtk4::ToggleButton::builder()
            .label("Dark")
            .valign(gtk4::Align::Center)
            .vexpand(false)
            .group(&theme_system_toggle)
            .build();

        // Create a linked box for grouped appearance
        let theme_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(0) // No spacing for linked appearance
            .css_classes(vec!["linked".to_string()]) // GTK linked style class
            .valign(gtk4::Align::Center) // Center vertically
            .vexpand(false) // Don't expand vertically
            .build();
        theme_box.append(&theme_system_toggle);
        theme_box.append(&theme_light_toggle);
        theme_box.append(&theme_dark_toggle);

        let theme_row = adw::ActionRow::builder()
            .title("Theme")
            .subtitle("Choose the application theme")
            .build();
        theme_row.add_suffix(&theme_box);
        theme_row.set_activatable_widget(Some(&theme_system_toggle));

        appearance_group.add(&theme_row);
        general_page.add(&appearance_group);

        // Player backend selection group
        let playback_group = adw::PreferencesGroup::builder().title("Playback").build();

        // Create player backend toggle buttons with proper styling and grouping
        let backend_gstreamer_toggle = gtk4::ToggleButton::builder()
            .label("GStreamer")
            .valign(gtk4::Align::Center)
            .vexpand(false)
            .build();
        let backend_mpv_toggle = gtk4::ToggleButton::builder()
            .label("MPV (Recommended)")
            .valign(gtk4::Align::Center)
            .vexpand(false)
            .group(&backend_gstreamer_toggle)
            .build();

        // Create a linked box for grouped appearance
        let backend_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(0) // No spacing for linked appearance
            .css_classes(vec!["linked".to_string()]) // GTK linked style class
            .valign(gtk4::Align::Center) // Center vertically
            .vexpand(false) // Don't expand vertically
            .build();
        backend_box.append(&backend_gstreamer_toggle);
        backend_box.append(&backend_mpv_toggle);

        let backend_row = adw::ActionRow::builder()
            .title("Player Backend")
            .subtitle("MPV provides better subtitle rendering")
            .build();
        backend_row.add_suffix(&backend_box);
        backend_row.set_activatable_widget(Some(&backend_gstreamer_toggle));

        playback_group.add(&backend_row);
        general_page.add(&playback_group);

        // Store references for reactive updates
        imp.theme_system_toggle
            .replace(Some(theme_system_toggle.clone()));
        imp.theme_light_toggle
            .replace(Some(theme_light_toggle.clone()));
        imp.theme_dark_toggle
            .replace(Some(theme_dark_toggle.clone()));
        imp.backend_gstreamer_toggle
            .replace(Some(backend_gstreamer_toggle.clone()));
        imp.backend_mpv_toggle
            .replace(Some(backend_mpv_toggle.clone()));

        // Log initial UI state before reactive handlers are set up
        debug!("🎮 PreferencesWindow: Initial toggle button states:");
        debug!(
            "  Theme - System:{}, Light:{}, Dark:{}",
            theme_system_toggle.is_active(),
            theme_light_toggle.is_active(),
            theme_dark_toggle.is_active()
        );
        debug!(
            "  Backend - GStreamer:{}, MPV:{}",
            backend_gstreamer_toggle.is_active(),
            backend_mpv_toggle.is_active()
        );

        // Add page to window
        self.add(&general_page);

        // Set up reactive handlers
        debug!("🎮 PreferencesWindow: About to setup reactive handlers for toggle buttons");
        self.setup_reactive_handlers();
        info!("✅ PreferencesWindow: Toggle button handlers established");
    }

    fn setup_reactive_handlers(&self) {
        let imp = self.imp();

        if let Some(view_model) = imp.view_model.borrow().as_ref() {
            info!(
                "🎮 PreferencesWindow: Setting up toggle button interaction handlers with ViewModel"
            );

            // Theme toggle button handlers
            self.connect_theme_toggle_handlers(view_model.clone());

            // Player backend toggle button handlers
            self.connect_backend_toggle_handlers(view_model.clone());

            info!("✅ PreferencesWindow: All toggle button handlers connected");
        }
    }

    fn connect_theme_toggle_handlers(&self, view_model: Arc<PreferencesViewModel>) {
        let imp = self.imp();

        // System theme handler
        if let Some(system_toggle) = imp.theme_system_toggle.borrow().as_ref() {
            system_toggle.connect_toggled(clone!(
                #[weak(rename_to = window)]
                self,
                #[strong]
                view_model,
                move |toggle| {
                    if toggle.is_active() {
                        info!("🎨 PreferencesWindow: USER SELECTED SYSTEM THEME");
                        let theme = ThemeOption::System;

                        let vm = view_model.clone();
                        glib::spawn_future_local(async move {
                            vm.set_theme(theme).await;
                        });
                    }
                }
            ));
        }

        // Light theme handler
        if let Some(light_toggle) = imp.theme_light_toggle.borrow().as_ref() {
            light_toggle.connect_toggled(clone!(
                #[weak(rename_to = window)]
                self,
                #[strong]
                view_model,
                move |toggle| {
                    if toggle.is_active() {
                        info!("🎨 PreferencesWindow: USER SELECTED LIGHT THEME");
                        let theme = ThemeOption::Light;

                        let vm = view_model.clone();
                        glib::spawn_future_local(async move {
                            vm.set_theme(theme).await;
                        });
                    }
                }
            ));
        }

        // Dark theme handler
        if let Some(dark_toggle) = imp.theme_dark_toggle.borrow().as_ref() {
            dark_toggle.connect_toggled(clone!(
                #[weak(rename_to = window)]
                self,
                #[strong]
                view_model,
                move |toggle| {
                    if toggle.is_active() {
                        info!("🎨 PreferencesWindow: USER SELECTED DARK THEME");
                        let theme = ThemeOption::Dark;

                        let vm = view_model.clone();
                        glib::spawn_future_local(async move {
                            vm.set_theme(theme).await;
                        });
                    }
                }
            ));
        }

        debug!("✅ PreferencesWindow: Theme toggle button handlers connected");
    }

    fn connect_backend_toggle_handlers(&self, view_model: Arc<PreferencesViewModel>) {
        let imp = self.imp();

        // GStreamer backend handler
        if let Some(gst_toggle) = imp.backend_gstreamer_toggle.borrow().as_ref() {
            gst_toggle.connect_toggled(clone!(
                #[weak(rename_to = window)]
                self,
                #[strong]
                view_model,
                move |toggle| {
                    if toggle.is_active() {
                        info!("🎛️ PreferencesWindow: USER SELECTED GSTREAMER BACKEND");
                        let backend = PlayerBackend::GStreamer;

                        let vm = view_model.clone();
                        let window_weak = window.downgrade();
                        glib::spawn_future_local(async move {
                            vm.set_player_backend(backend.clone()).await;

                            // Show toast notification
                            if let Some(window) = window_weak.upgrade() {
                                window.show_backend_changed_toast(&backend);
                            }
                        });
                    }
                }
            ));
        }

        // MPV backend handler
        if let Some(mpv_toggle) = imp.backend_mpv_toggle.borrow().as_ref() {
            mpv_toggle.connect_toggled(clone!(
                #[weak(rename_to = window)]
                self,
                #[strong]
                view_model,
                move |toggle| {
                    if toggle.is_active() {
                        info!("🎛️ PreferencesWindow: USER SELECTED MPV BACKEND");
                        let backend = PlayerBackend::Mpv;

                        let vm = view_model.clone();
                        let window_weak = window.downgrade();
                        glib::spawn_future_local(async move {
                            vm.set_player_backend(backend.clone()).await;

                            // Show toast notification
                            if let Some(window) = window_weak.upgrade() {
                                window.show_backend_changed_toast(&backend);
                            }
                        });
                    }
                }
            ));
        }

        debug!("✅ PreferencesWindow: Backend toggle button handlers connected");
    }

    fn setup_reactive_subscriptions(&self) {
        let imp = self.imp();

        if let Some(view_model) = imp.view_model.borrow().as_ref() {
            debug!("🔄 PreferencesWindow: Setting up reactive subscriptions");

            // Subscribe to theme changes
            let mut theme_subscriber = view_model.theme().subscribe();
            let window_weak = self.downgrade();
            glib::spawn_future_local(async move {
                while theme_subscriber.wait_for_change().await {
                    if let Some(window) = window_weak.upgrade() {
                        if let Some(vm) = window.imp().view_model.borrow().as_ref() {
                            let theme = vm.theme().get().await;
                            window.update_theme_toggle_buttons(&theme).await;
                        }
                    } else {
                        break;
                    }
                }
            });

            // Subscribe to player backend changes
            let mut backend_subscriber = view_model.player_backend().subscribe();
            let window_weak = self.downgrade();
            glib::spawn_future_local(async move {
                while backend_subscriber.wait_for_change().await {
                    if let Some(window) = window_weak.upgrade() {
                        if let Some(vm) = window.imp().view_model.borrow().as_ref() {
                            let backend = vm.player_backend().get().await;
                            window.update_backend_toggle_buttons(&backend).await;
                        }
                    } else {
                        break;
                    }
                }
            });

            info!("✅ PreferencesWindow: Reactive subscriptions established");
        } else {
            warn!("⚠️ PreferencesWindow: No ViewModel available for reactive subscriptions");
        }
    }

    async fn sync_ui_with_viewmodel(&self) {
        let imp = self.imp();

        if let Some(view_model) = imp.view_model.borrow().as_ref() {
            let theme = view_model.theme().get().await;
            let backend = view_model.player_backend().get().await;

            self.update_theme_toggle_buttons(&theme).await;
            self.update_backend_toggle_buttons(&backend).await;
        }
    }

    async fn update_theme_toggle_buttons(&self, theme: &ThemeOption) {
        let imp = self.imp();

        if let Some(system_toggle) = imp.theme_system_toggle.borrow().as_ref() {
            system_toggle.set_active(matches!(theme, ThemeOption::System));
        }
        if let Some(light_toggle) = imp.theme_light_toggle.borrow().as_ref() {
            light_toggle.set_active(matches!(theme, ThemeOption::Light));
        }
        if let Some(dark_toggle) = imp.theme_dark_toggle.borrow().as_ref() {
            dark_toggle.set_active(matches!(theme, ThemeOption::Dark));
        }
    }

    async fn update_backend_toggle_buttons(&self, backend: &PlayerBackend) {
        let imp = self.imp();

        if let Some(gst_toggle) = imp.backend_gstreamer_toggle.borrow().as_ref() {
            gst_toggle.set_active(matches!(backend, PlayerBackend::GStreamer));
        }
        if let Some(mpv_toggle) = imp.backend_mpv_toggle.borrow().as_ref() {
            mpv_toggle.set_active(matches!(backend, PlayerBackend::Mpv));
        }
    }

    fn show_backend_changed_toast(&self, backend: &PlayerBackend) {
        debug!("🍞 PreferencesWindow: Preparing to show toast notification for backend change");

        if let Some(parent) = self
            .transient_for()
            .and_then(|w| w.downcast::<adw::ApplicationWindow>().ok())
        {
            debug!("🍞 PreferencesWindow: Found parent ApplicationWindow for toast");
            let toast_message = format!("Player backend changed to {}", backend.display_name());
            info!("🍞 PreferencesWindow: Toast message: '{}'", toast_message);

            let toast = adw::Toast::builder()
                .title(toast_message)
                .timeout(3)
                .build();

            // Find the toast overlay in the main window
            if let Some(content) = parent
                .content()
                .and_then(|w| w.downcast::<adw::ToastOverlay>().ok())
            {
                debug!("🍞 PreferencesWindow: Found ToastOverlay, showing toast");
                content.add_toast(toast);
                info!("✅ PreferencesWindow: Toast notification displayed successfully");
            } else {
                warn!("⚠️ PreferencesWindow: No ToastOverlay found in parent window");
                info!(
                    "🎬 PreferencesWindow: Player backend ({}) will be used for next playback",
                    backend.display_name()
                );
            }
        } else {
            warn!("⚠️ PreferencesWindow: No parent ApplicationWindow found for toast notification");
            info!(
                "🎬 PreferencesWindow: Player backend ({}) will be used for next playback",
                backend.display_name()
            );
        }
    }
}
