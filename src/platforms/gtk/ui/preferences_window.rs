use gtk4::{glib, glib::clone, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{error, info};

use crate::config::Config;
use crate::core::viewmodels::{PreferencesViewModel, ViewModel};
use crate::events::EventBus;
use tokio::sync::RwLock;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct PreferencesWindow {
        pub view_model: RefCell<Option<Arc<PreferencesViewModel>>>,
    }

    impl std::fmt::Debug for PreferencesWindow {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("PreferencesWindow")
                .field("view_model", &"Option<Arc<PreferencesViewModel>>")
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

        let view_model = Arc::new(PreferencesViewModel::new(config));

        // Initialize the ViewModel
        let vm_clone = view_model.clone();
        glib::spawn_future_local(async move {
            vm_clone.initialize(event_bus).await;
        });

        window.imp().view_model.replace(Some(view_model));
        window
    }

    fn setup_ui(&self) {
        let Some(view_model) = self.imp().view_model.borrow().as_ref().map(|vm| vm.clone()) else {
            error!("PreferencesWindow: ViewModel not initialized");
            return;
        };

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

        let video_output_row = adw::ComboRow::builder()
            .title("Video Output")
            .subtitle("Embedded rendering (libmpv) or external HDR window (gpu-next)")
            .model(&gtk4::StringList::new(&["Embedded", "External HDR"]))
            .build();

        playback_group.add(&player_backend_row);
        playback_group.add(&video_output_row);
        general_page.add(&playback_group);

        // Add page to window
        self.add(&general_page);

        // Set up reactive binding for theme
        self.bind_theme_property(view_model.clone(), theme_row.clone());

        // Set up reactive binding for player backend
        self.bind_player_backend_property(view_model.clone(), player_backend_row.clone());

        // Set up reactive binding for video output
        self.bind_video_output_property(view_model.clone(), video_output_row.clone());

        // Set up event handlers
        theme_row.connect_selected_notify(clone!(
            #[weak]
            view_model,
            move |row| {
                let selected = row.selected();
                let theme =
                    crate::core::viewmodels::preferences_view_model::Theme::from_index(selected);

                let vm = view_model.clone();
                glib::spawn_future_local(async move {
                    if let Err(e) = vm.set_theme(theme).await {
                        error!("Failed to set theme: {}", e);
                    }
                });
            }
        ));

        player_backend_row.connect_selected_notify(clone!(
            #[weak]
            view_model,
            move |row| {
                let selected = row.selected();
                let backend =
                    crate::core::viewmodels::preferences_view_model::PlayerBackend::from_index(
                        selected,
                    );

                let vm = view_model.clone();
                glib::spawn_future_local(async move {
                    if let Err(e) = vm.set_player_backend(backend).await {
                        error!("Failed to set player backend: {}", e);
                    }
                });
            }
        ));

        video_output_row.connect_selected_notify(clone!(
            #[weak]
            view_model,
            move |row| {
                let selected = row.selected();
                let video_output =
                    crate::core::viewmodels::preferences_view_model::VideoOutput::from_index(
                        selected,
                    );

                let vm = view_model.clone();
                glib::spawn_future_local(async move {
                    if let Err(e) = vm.set_video_output(video_output).await {
                        error!("Failed to set video output: {}", e);
                    }
                });
            }
        ));

        // Show toast notifications for changes
        self.setup_toast_notifications(view_model);
    }

    fn bind_theme_property(&self, view_model: Arc<PreferencesViewModel>, theme_row: adw::ComboRow) {
        // Set initial value
        let vm_clone = view_model.clone();
        let theme_row_clone = theme_row.clone();
        glib::spawn_future_local(async move {
            let theme = vm_clone.get_theme().await;
            theme_row_clone.set_selected(theme.to_index());
        });

        // Subscribe to changes
        let mut theme_subscriber = view_model.subscribe_theme();
        glib::spawn_future_local(async move {
            while theme_subscriber.wait_for_change().await {
                let theme = view_model.get_theme().await;
                theme_row.set_selected(theme.to_index());
            }
        });
    }

    fn bind_player_backend_property(
        &self,
        view_model: Arc<PreferencesViewModel>,
        backend_row: adw::ComboRow,
    ) {
        // Set initial value
        let vm_clone = view_model.clone();
        let backend_row_clone = backend_row.clone();
        glib::spawn_future_local(async move {
            let backend = vm_clone.get_player_backend().await;
            backend_row_clone.set_selected(backend.to_index());
        });

        // Subscribe to changes
        let mut backend_subscriber = view_model.subscribe_player_backend();
        glib::spawn_future_local(async move {
            while backend_subscriber.wait_for_change().await {
                let backend = view_model.get_player_backend().await;
                backend_row.set_selected(backend.to_index());
            }
        });
    }

    fn bind_video_output_property(
        &self,
        view_model: Arc<PreferencesViewModel>,
        output_row: adw::ComboRow,
    ) {
        // Set initial value
        let vm_clone = view_model.clone();
        let output_row_clone = output_row.clone();
        glib::spawn_future_local(async move {
            let output = vm_clone.get_video_output().await;
            output_row_clone.set_selected(output.to_index());
        });

        // Subscribe to changes
        let mut output_subscriber = view_model.subscribe_video_output();
        glib::spawn_future_local(async move {
            while output_subscriber.wait_for_change().await {
                let output = view_model.get_video_output().await;
                output_row.set_selected(output.to_index());
            }
        });
    }

    fn setup_toast_notifications(&self, view_model: Arc<PreferencesViewModel>) {
        let window_weak = self.downgrade();

        // Subscribe to theme changes
        let view_model_theme = view_model.clone();
        let window_weak_theme = window_weak.clone();
        let mut theme_subscriber = view_model.subscribe_theme();
        glib::spawn_future_local(async move {
            while theme_subscriber.wait_for_change().await {
                let theme = view_model_theme.get_theme().await;

                // Apply theme to style manager
                let style_manager = adw::StyleManager::default();
                match theme {
                    crate::core::viewmodels::preferences_view_model::Theme::Light => {
                        style_manager.set_color_scheme(adw::ColorScheme::ForceLight);
                    }
                    crate::core::viewmodels::preferences_view_model::Theme::Dark => {
                        style_manager.set_color_scheme(adw::ColorScheme::ForceDark);
                    }
                    crate::core::viewmodels::preferences_view_model::Theme::System => {
                        style_manager.set_color_scheme(adw::ColorScheme::PreferDark);
                    }
                }

                if let Some(window) = window_weak_theme.upgrade() {
                    window.show_toast(&format!("Theme changed to {}", theme.display_name()));
                }
            }
        });

        // Subscribe to player backend changes
        let view_model_backend = view_model.clone();
        let window_weak_backend = window_weak.clone();
        let mut backend_subscriber = view_model.subscribe_player_backend();
        glib::spawn_future_local(async move {
            while backend_subscriber.wait_for_change().await {
                let backend = view_model_backend.get_player_backend().await;
                if let Some(window) = window_weak_backend.upgrade() {
                    window.show_toast(&format!(
                        "Player backend changed to {}",
                        backend.display_name()
                    ));
                }
            }
        });

        // Subscribe to video output changes
        let view_model_output = view_model.clone();
        let window_weak_output = window_weak.clone();
        let mut output_subscriber = view_model.subscribe_video_output();
        glib::spawn_future_local(async move {
            while output_subscriber.wait_for_change().await {
                let output = view_model_output.get_video_output().await;
                if let Some(window) = window_weak_output.upgrade() {
                    window.show_toast(&format!(
                        "Video output changed to {}",
                        output.display_name()
                    ));
                }
            }
        });
    }

    fn show_toast(&self, message: &str) {
        if let Some(parent) = self
            .transient_for()
            .and_then(|w| w.downcast::<adw::ApplicationWindow>().ok())
        {
            let toast = adw::Toast::builder().title(message).timeout(3).build();

            // Find the toast overlay in the main window
            if let Some(content) = parent
                .content()
                .and_then(|w| w.downcast::<adw::ToastOverlay>().ok())
            {
                content.add_toast(toast);
            } else {
                info!("{}", message);
            }
        }
    }
}
