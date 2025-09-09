use gtk4::{gio, glib, glib::clone, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{debug, error, info};

use super::filters::{SortOrder, WatchStatus};
use super::pages;
use crate::config::Config;
use crate::state::AppState;
use tokio::sync::RwLock;

// Wrapper enum to handle both library view types
#[derive(Clone, Debug)]
enum LibraryViewWrapper {
    Standard(crate::platforms::gtk::ui::pages::LibraryView),
    Virtual(crate::platforms::gtk::ui::pages::LibraryVirtualView),
}

impl LibraryViewWrapper {
    fn set_on_media_selected<F>(&self, callback: F)
    where
        F: Fn(&crate::models::MediaItem) + 'static,
    {
        match self {
            LibraryViewWrapper::Standard(view) => view.set_on_media_selected(callback),
            LibraryViewWrapper::Virtual(view) => view.set_on_media_selected(callback),
        }
    }

    async fn load_library(&self, backend_id: String, library: crate::models::Library) {
        match self {
            LibraryViewWrapper::Standard(view) => view.load_library(backend_id, library).await,
            LibraryViewWrapper::Virtual(view) => view.load_library(backend_id, library).await,
        }
    }

    fn update_watch_status_filter(&self, status: WatchStatus) {
        match self {
            LibraryViewWrapper::Standard(view) => view.update_watch_status_filter(status),
            LibraryViewWrapper::Virtual(view) => view.update_watch_status_filter(status),
        }
    }

    fn update_sort_order(&self, order: SortOrder) {
        match self {
            LibraryViewWrapper::Standard(view) => view.update_sort_order(order),
            LibraryViewWrapper::Virtual(view) => view.update_sort_order(order),
        }
    }

    fn search(&self, query: String) {
        match self {
            LibraryViewWrapper::Standard(view) => view.search(query),
            LibraryViewWrapper::Virtual(view) => view.search(query),
        }
    }

    fn refresh(&self) {
        match self {
            LibraryViewWrapper::Standard(view) => view.refresh(),
            LibraryViewWrapper::Virtual(view) => view.refresh(),
        }
    }

    fn upcast(&self) -> gtk4::Widget {
        match self {
            LibraryViewWrapper::Standard(view) => view.clone().upcast(),
            LibraryViewWrapper::Virtual(view) => view.clone().upcast(),
        }
    }

    fn downgrade(&self) -> LibraryViewWrapperWeak {
        match self {
            LibraryViewWrapper::Standard(view) => {
                LibraryViewWrapperWeak::Standard(view.downgrade())
            }
            LibraryViewWrapper::Virtual(view) => LibraryViewWrapperWeak::Virtual(view.downgrade()),
        }
    }
}

// Weak reference version of the wrapper
#[derive(Clone, Debug)]
enum LibraryViewWrapperWeak {
    Standard(gtk4::glib::WeakRef<crate::platforms::gtk::ui::pages::LibraryView>),
    Virtual(gtk4::glib::WeakRef<crate::platforms::gtk::ui::pages::LibraryVirtualView>),
}

impl LibraryViewWrapperWeak {
    fn upgrade(&self) -> Option<LibraryViewWrapper> {
        match self {
            LibraryViewWrapperWeak::Standard(weak) => {
                weak.upgrade().map(LibraryViewWrapper::Standard)
            }
            LibraryViewWrapperWeak::Virtual(weak) => {
                weak.upgrade().map(LibraryViewWrapper::Virtual)
            }
        }
    }
}

mod imp {
    use super::*;

    use gtk4::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/dev/arsfeld/Reel/window.ui")]
    pub struct ReelMainWindow {
        #[template_child]
        pub welcome_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub connect_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub home_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub home_list: TemplateChild<gtk4::ListBox>,
        #[template_child]
        pub sources_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub sources_container: TemplateChild<gtk4::Box>,
        #[template_child]
        pub status_container: TemplateChild<gtk4::Box>,
        #[template_child]
        pub status_icon: TemplateChild<gtk4::Image>,
        #[template_child]
        pub status_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub sync_spinner: TemplateChild<gtk4::Spinner>,
        #[template_child]
        pub empty_state: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub content_page: TemplateChild<adw::NavigationPage>,
        #[template_child]
        pub content_toolbar: TemplateChild<adw::ToolbarView>,
        #[template_child]
        pub content_header: TemplateChild<adw::HeaderBar>,

        pub state: RefCell<Option<Arc<AppState>>>,
        pub config: RefCell<Option<Arc<RwLock<Config>>>>,
        pub content_stack: RefCell<Option<gtk4::Stack>>,
        pub home_page: RefCell<Option<crate::platforms::gtk::ui::pages::HomePage>>,
        pub sources_page: RefCell<Option<crate::platforms::gtk::ui::pages::SourcesPage>>,
        pub library_view: RefCell<Option<LibraryViewWrapper>>,
        pub player_page: RefCell<Option<crate::platforms::gtk::ui::pages::PlayerPage>>,
        pub show_details_page: RefCell<Option<crate::platforms::gtk::ui::pages::ShowDetailsPage>>,
        pub movie_details_page: RefCell<Option<crate::platforms::gtk::ui::pages::MovieDetailsPage>>,
        pub back_button: RefCell<Option<gtk4::Button>>,
        pub saved_window_size: RefCell<(i32, i32)>,
        pub filter_controls: RefCell<Option<gtk4::Box>>,
        pub edit_mode: RefCell<bool>,
        pub library_visibility: RefCell<std::collections::HashMap<String, bool>>,
        pub all_libraries: RefCell<Vec<(crate::models::Library, usize)>>,
        pub navigation_stack: RefCell<Vec<String>>, // Track navigation history
        pub header_add_button: RefCell<Option<gtk4::Button>>, // Track add button in header
        pub sidebar_viewmodel:
            RefCell<Option<Arc<crate::platforms::gtk::ui::viewmodels::SidebarViewModel>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ReelMainWindow {
        const NAME: &'static str = "ReelMainWindow";
        type Type = super::ReelMainWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[gtk4::template_callbacks]
    impl ReelMainWindow {
        #[template_callback]
        fn on_connect_clicked(&self, _button: &gtk4::Button) {
            info!("Connect button clicked");
            self.obj().show_auth_dialog();
        }

        #[template_callback]
        fn on_refresh_clicked(&self, _button: &gtk4::Button) {
            info!("Refresh button clicked");
            // We can't spawn async directly from template callback,
            // so we'll just log for now. The actual handler is in constructed()
        }
    }

    impl ObjectImpl for ReelMainWindow {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();

            // Connect signals
            self.connect_button.connect_clicked(clone!(
                #[weak]
                obj,
                move |_| {
                    obj.show_auth_dialog();
                }
            ));

            // Status is now more subtle, no need for clickable row

            // No longer need refresh_button and edit_libraries_button connections here
            // They will be handled per-source group

            // Connect to home list row activation
            self.home_list.connect_row_activated(clone!(
                #[weak]
                obj,
                move |_, row| {
                    if let Some(action_row) = row.downcast_ref::<adw::ActionRow>() {
                        info!("Home selected");
                        obj.show_home_page_for_source(None);
                    }
                }
            ));

            // Connect to sources button click
            self.sources_button.connect_clicked(clone!(
                #[weak]
                obj,
                move |_| {
                    info!("Sources button clicked");
                    obj.show_sources_page();
                }
            ));

            // Library list row activation is now handled per-source group

            // Sources button is now always visible
        }
    }

    impl WidgetImpl for ReelMainWindow {}
    impl WindowImpl for ReelMainWindow {}
    impl ApplicationWindowImpl for ReelMainWindow {}
    impl adw::subclass::application_window::AdwApplicationWindowImpl for ReelMainWindow {}
}

glib::wrapper! {
    pub struct ReelMainWindow(ObjectSubclass<imp::ReelMainWindow>)
        @extends gtk4::Widget, gtk4::Window, gtk4::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk4::Accessible, gtk4::Buildable,
                    gtk4::ConstraintTarget, gtk4::Native, gtk4::Root, gtk4::ShortcutManager;
}

impl ReelMainWindow {
    pub fn new(app: &adw::Application, state: Arc<AppState>, config: Arc<RwLock<Config>>) -> Self {
        let window: Self = glib::Object::builder().property("application", app).build();

        // Store state and config
        window.imp().state.replace(Some(state.clone()));
        window.imp().config.replace(Some(config));

        // Initialize SidebarViewModel
        let sidebar_vm = Arc::new(
            crate::platforms::gtk::ui::viewmodels::SidebarViewModel::new(
                state.data_service.clone(),
            ),
        );
        window
            .imp()
            .sidebar_viewmodel
            .replace(Some(sidebar_vm.clone()));

        // Initialize the ViewModel with the event bus
        let event_bus = state.event_bus.clone();
        let sidebar_vm_clone = sidebar_vm.clone();
        let window_weak_for_vm = window.downgrade();
        glib::spawn_future_local(async move {
            use super::viewmodels::ViewModel;
            sidebar_vm_clone.initialize(event_bus).await;

            // Set up subscription to sources property
            if let Some(window) = window_weak_for_vm.upgrade() {
                window.setup_sidebar_subscriptions();

                // Initial load should happen after ViewModel is initialized and subscriptions are set up
                tracing::info!("SidebarViewModel initialized, triggering initial data load");
                sidebar_vm_clone.refresh().await;
            }
        });

        // Setup actions
        window.setup_actions(app);

        // Apply theme
        let window_weak = window.downgrade();
        glib::spawn_future_local(async move {
            if let Some(window) = window_weak.upgrade() {
                window.apply_theme().await;
            }
        });

        // Check for existing backends and load them (SidebarViewModel will handle cached data display)
        window.check_and_load_backends(state);

        // Subscribe to user changes
        window.setup_state_subscriptions();

        window
    }

    fn setup_actions(&self, app: &adw::Application) {
        // Preferences action
        let preferences_action = gio::SimpleAction::new("preferences", None);
        preferences_action.connect_activate(clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                info!("Opening preferences");
                window.show_preferences();
            }
        ));
        app.add_action(&preferences_action);

        // About action
        let about_action = gio::SimpleAction::new("about", None);
        about_action.connect_activate(clone!(
            #[weak(rename_to = window)]
            self,
            move |_, _| {
                window.show_about();
            }
        ));
        app.add_action(&about_action);

        // Keyboard shortcuts
        app.set_accels_for_action("app.preferences", &["<primary>comma"]);
        app.set_accels_for_action("window.close", &["<primary>w"]);
    }

    async fn apply_theme(&self) {
        if let Some(config_arc) = self.imp().config.borrow().as_ref() {
            let style_manager = adw::StyleManager::default();

            let theme = {
                let config = config_arc.read().await;
                config.general.theme.clone()
            };

            match theme.as_str() {
                "light" => style_manager.set_color_scheme(adw::ColorScheme::ForceLight),
                "dark" => style_manager.set_color_scheme(adw::ColorScheme::ForceDark),
                _ => style_manager.set_color_scheme(adw::ColorScheme::PreferDark),
            }
        }
    }

    fn check_and_load_backends(&self, state: Arc<AppState>) {
        let window_weak = self.downgrade();

        glib::spawn_future_local(async move {
            // Use SourceCoordinator to initialize all sources
            let source_coordinator = state.get_source_coordinator();
            // First, load saved providers from config
            if let Err(e) = source_coordinator.get_auth_manager().load_providers().await {
                error!("Failed to load providers: {}", e);
            }

            // Then, migrate any legacy backends
            if let Err(e) = source_coordinator.migrate_legacy_backends().await {
                error!("Failed to migrate legacy backends: {}", e);
            }

            // Initialize all sources
            match source_coordinator.initialize_all_sources().await {
                Ok(source_statuses) => {
                    let connected_count = source_statuses
                        .iter()
                        .filter(|s| {
                            matches!(
                                s.connection_status,
                                crate::services::source_coordinator::ConnectionStatus::Connected
                            )
                        })
                        .count();

                    info!(
                        "Initialized {} sources, {} connected",
                        source_statuses.len(),
                        connected_count
                    );
                }
                Err(e) => {
                    error!("Failed to initialize sources: {}", e);
                }
            }

            // Handle the results of backend initialization
            if let Some(window) = window_weak.upgrade() {
                // Update backend selector (now just hides it)
                window.update_backend_selector().await;

                // Check how many backends are connected
                let all_backends = state.source_coordinator.get_all_backends().await;
                let connected_count = all_backends.len();

                if connected_count > 0 {
                    info!("Successfully initialized {} backends", connected_count);
                    // Status is now managed by SidebarViewModel - no manual updates needed

                    // Refresh all libraries
                    window.refresh_all_libraries().await;

                    // Start background sync for all sources
                    let coordinator_clone = source_coordinator.clone();
                    let window_weak2 = window.downgrade();
                    glib::spawn_future_local(async move {
                        if let Some(_window) = window_weak2.upgrade() {
                            info!("Starting background sync for all sources...");
                            if let Err(e) = coordinator_clone.sync_all_visible_sources().await {
                                error!("Failed to sync sources: {}", e);
                            }
                        }
                    });
                } else {
                    info!("No backends were successfully initialized");

                    // Check if we have any providers that need authentication
                    let providers = source_coordinator
                        .get_auth_manager()
                        .get_all_providers()
                        .await;

                    if !providers.is_empty() {
                        info!(
                            "Found {} providers but no valid credentials",
                            providers.len()
                        );

                        // Status is now managed by SidebarViewModel - no manual updates needed
                    }
                }
            }
        });
    }

    fn setup_state_subscriptions(&self) {
        // Listen for state changes
        // For now, we'll handle updates manually when auth completes
    }

    fn setup_sidebar_subscriptions(&self) {
        if let Some(sidebar_vm) = self.imp().sidebar_viewmodel.borrow().as_ref() {
            // Subscribe to sources changes
            let mut sources_subscriber = sidebar_vm.sources().subscribe();
            let window_weak = self.downgrade();

            glib::spawn_future_local(async move {
                while sources_subscriber.wait_for_change().await {
                    if let Some(window) = window_weak.upgrade() {
                        window.update_sidebar_from_viewmodel();
                    }
                }
            });

            // Subscribe to status text changes
            let mut status_subscriber = sidebar_vm.status_text().subscribe();
            let window_weak = self.downgrade();

            glib::spawn_future_local(async move {
                while status_subscriber.wait_for_change().await {
                    if let Some(window) = window_weak.upgrade()
                        && let Some(vm) = window.imp().sidebar_viewmodel.borrow().as_ref()
                    {
                        let text = vm.status_text().get_sync();
                        window.imp().status_label.set_text(&text);
                    }
                }
            });

            // Subscribe to status icon changes
            let mut icon_subscriber = sidebar_vm.status_icon().subscribe();
            let window_weak = self.downgrade();

            glib::spawn_future_local(async move {
                while icon_subscriber.wait_for_change().await {
                    if let Some(window) = window_weak.upgrade()
                        && let Some(vm) = window.imp().sidebar_viewmodel.borrow().as_ref()
                    {
                        let icon = vm.status_icon().get_sync();
                        window.imp().status_icon.set_icon_name(Some(&icon));
                    }
                }
            });

            // Subscribe to spinner visibility
            let mut spinner_subscriber = sidebar_vm.show_spinner().subscribe();
            let window_weak = self.downgrade();

            glib::spawn_future_local(async move {
                while spinner_subscriber.wait_for_change().await {
                    if let Some(window) = window_weak.upgrade()
                        && let Some(vm) = window.imp().sidebar_viewmodel.borrow().as_ref()
                    {
                        let show = vm.show_spinner().get_sync();
                        window.imp().sync_spinner.set_visible(show);
                        window.imp().sync_spinner.set_spinning(show);
                    }
                }
            });

            // Subscribe to connection status
            let mut connected_subscriber = sidebar_vm.is_connected().subscribe();
            let window_weak = self.downgrade();

            glib::spawn_future_local(async move {
                while connected_subscriber.wait_for_change().await {
                    if let Some(window) = window_weak.upgrade()
                        && let Some(vm) = window.imp().sidebar_viewmodel.borrow().as_ref()
                    {
                        let connected = vm.is_connected().get_sync();
                        let imp = window.imp();

                        if connected {
                            imp.welcome_page.set_visible(false);
                            imp.home_group.set_visible(true);
                            imp.sources_container.set_visible(true);
                            imp.status_container.set_visible(true);
                        } else {
                            // Only show welcome if we truly have no data
                            let sources = vm.sources().get_sync();
                            if sources.is_empty() {
                                imp.welcome_page.set_visible(true);
                                imp.home_group.set_visible(false);
                                imp.sources_container.set_visible(false);
                            }
                        }
                    }
                }
            });
        }
    }

    fn update_sidebar_from_viewmodel(&self) {
        if let Some(sidebar_vm) = self.imp().sidebar_viewmodel.borrow().as_ref() {
            // Get sources synchronously - no need for async since it's already in memory
            let sources = sidebar_vm.sources().get_sync();

            // Convert ViewModel data to the format expected by update_all_backends_libraries
            let mut backends_libraries = Vec::new();

            for source in sources {
                let libraries: Vec<(crate::models::Library, usize)> = source
                    .libraries
                    .iter()
                    .map(|lib| {
                        let library = crate::models::Library {
                            id: lib.id.clone(),
                            title: lib.title.clone(),
                            library_type: match lib.library_type.as_str() {
                                "movies" => crate::models::LibraryType::Movies,
                                "shows" => crate::models::LibraryType::Shows,
                                "music" => crate::models::LibraryType::Music,
                                "photos" => crate::models::LibraryType::Photos,
                                _ => crate::models::LibraryType::Mixed,
                            },
                            icon: lib.icon.clone(),
                        };
                        (library, lib.item_count as usize)
                    })
                    .collect();

                if !libraries.is_empty() {
                    backends_libraries.push((source.id.clone(), source.name.clone(), libraries));
                }
            }

            // Update the UI directly - no async needed
            self.update_all_backends_libraries_with_names(backends_libraries);
        }
    }

    pub fn show_auth_dialog(&self) {
        info!("Showing authentication dialog");

        // Get state from the window
        let state = self.imp().state.borrow().as_ref().unwrap().clone();

        // Create and show auth dialog
        let dialog = crate::platforms::gtk::ui::AuthDialog::new(state.clone());
        dialog.present(Some(self));

        // Start authentication automatically
        dialog.start_authentication();

        // Set up a callback for when the dialog closes
        let window_weak = self.downgrade();
        let state_clone = state.clone();
        dialog.connect_closed(move |_| {
            if let Some(window) = window_weak.upgrade() {
                // Check if we now have an authenticated backend
                let state_for_async = state_clone.clone();
                glib::spawn_future_local(async move {
                    let source_coordinator = state_for_async.get_source_coordinator();
                    let all_backends = source_coordinator.get_all_backends().await;
                    if let Some((backend_id, backend)) = all_backends.into_iter().next()
                        && backend.is_initialized().await
                    {
                        info!("Backend initialized after auth dialog closed");

                        // Backend status is now managed by SidebarViewModel
                        window.update_backend_selector().await;

                        // FIRST: Load cached data immediately via SidebarViewModel
                        info!(
                            "Refreshing SidebarViewModel for newly authenticated backend: {}",
                            backend_id
                        );
                        if let Some(sidebar_vm) = window.imp().sidebar_viewmodel.borrow().as_ref() {
                            use super::viewmodels::ViewModel;
                            sidebar_vm.refresh().await;
                        }

                        // THEN: Start background sync
                        let backend_clone = backend.clone();
                        let state_clone = state_for_async.clone();
                        let window_weak = window.downgrade();
                        glib::spawn_future_local(async move {
                            if let Some(window) = window_weak.upgrade() {
                                info!("Starting background sync after auth...");
                                // Sync progress is now managed by SidebarViewModel

                                // Start sync with backend ID
                                // Use the same backend_id from outer scope
                                if true {
                                    window
                                        .sync_and_update_libraries(
                                            &backend_id,
                                            backend_clone,
                                            state_clone,
                                        )
                                        .await;
                                }

                                // Sync progress is now managed by SidebarViewModel
                            }
                        });
                    }
                });
            }
        });
    }

    fn show_preferences(&self) {
        info!("Showing preferences");

        if let Some(config) = self.imp().config.borrow().as_ref()
            && let Some(state) = self.imp().state.borrow().as_ref()
        {
            let prefs_window = crate::platforms::gtk::ui::PreferencesWindow::new(
                self,
                config.clone(),
                state.event_bus.clone(),
            );
            prefs_window.present();
        }
    }

    fn show_about(&self) {
        let about = adw::AboutWindow::builder()
            .application_name("Reel")
            .application_icon("dev.arsfeld.Reel")
            .developer_name("Alexandre Rosenfeld")
            .version(env!("CARGO_PKG_VERSION"))
            .license_type(gtk4::License::Gpl30)
            .website("https://github.com/arsfeld/reel")
            .issue_url("https://github.com/arsfeld/reel/issues")
            .build();

        about.set_transient_for(Some(self));
        about.present();
    }

    pub fn update_all_backends_libraries_with_names(
        &self,
        backends_libraries: Vec<(String, String, Vec<(crate::models::Library, usize)>)>,
    ) {
        let imp = self.imp();

        // Clear existing source containers
        while let Some(child) = imp.sources_container.first_child() {
            imp.sources_container.remove(&child);
        }

        // Clear existing home rows
        while let Some(child) = imp.home_list.first_child() {
            imp.home_list.remove(&child);
        }

        // Add unified Home row for all sources
        let home_row = adw::ActionRow::builder()
            .title("Home")
            .subtitle("Recently added from all sources")
            .activatable(true)
            .build();

        let home_icon = gtk4::Image::from_icon_name("user-home-symbolic");
        home_row.add_prefix(&home_icon);

        let home_arrow = gtk4::Image::from_icon_name("go-next-symbolic");
        home_row.add_suffix(&home_arrow);

        home_row.set_widget_name("__home__");
        imp.home_list.append(&home_row);
        imp.home_group.set_visible(true);

        // Collect all libraries for edit mode
        let mut all_libraries = Vec::new();

        // Create a separate PreferencesGroup for each backend
        for (backend_id, source_name, libraries) in backends_libraries.iter() {
            if libraries.is_empty() {
                continue;
            }

            // Create a preferences group for this source
            let source_group = adw::PreferencesGroup::builder().title(source_name).build();

            // Add edit/refresh buttons in the header suffix
            let header_buttons = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .spacing(6)
                .build();

            let edit_button = gtk4::Button::builder()
                .icon_name("document-edit-symbolic")
                .valign(gtk4::Align::Center)
                .tooltip_text("Edit Libraries")
                .css_classes(vec!["flat"])
                .build();

            let refresh_button = gtk4::Button::builder()
                .icon_name("view-refresh-symbolic")
                .valign(gtk4::Align::Center)
                .tooltip_text("Refresh")
                .css_classes(vec!["flat"])
                .build();

            header_buttons.append(&edit_button);
            header_buttons.append(&refresh_button);
            source_group.set_header_suffix(Some(&header_buttons));

            // Connect refresh button for this specific backend
            let backend_id_clone = backend_id.clone();
            let window_weak = self.downgrade();
            refresh_button.connect_clicked(move |_| {
                if let Some(window) = window_weak.upgrade() {
                    let backend_id = backend_id_clone.clone();
                    glib::spawn_future_local(async move {
                        window.sync_single_backend(&backend_id).await;
                    });
                }
            });

            // Create list box for libraries
            let libraries_list = gtk4::ListBox::builder()
                .selection_mode(gtk4::SelectionMode::None)
                .css_classes(vec!["boxed-list"])
                .build();

            // Add libraries for this backend
            for (library, item_count) in libraries {
                all_libraries.push((library.clone(), *item_count));

                let visibility_map = imp.library_visibility.borrow();
                let is_visible = visibility_map.get(&library.id).copied().unwrap_or(true);

                if is_visible {
                    let row = adw::ActionRow::builder()
                        .title(&library.title)
                        .subtitle(format!("{} items", item_count))
                        .activatable(true)
                        .build();

                    // Add icon based on library type
                    let icon_name = match library.library_type {
                        crate::models::LibraryType::Movies => "video-x-generic-symbolic",
                        crate::models::LibraryType::Shows => "video-display-symbolic",
                        crate::models::LibraryType::Music => "audio-x-generic-symbolic",
                        crate::models::LibraryType::Photos => "image-x-generic-symbolic",
                        _ => "folder-symbolic",
                    };

                    let prefix_icon = gtk4::Image::from_icon_name(icon_name);
                    row.add_prefix(&prefix_icon);

                    let arrow = gtk4::Image::from_icon_name("go-next-symbolic");
                    row.add_suffix(&arrow);

                    // Store backend_id:library_id in widget name for navigation
                    row.set_widget_name(&format!("{}:{}", backend_id, library.id));

                    libraries_list.append(&row);
                }
            }

            // Connect row activation for this list
            let window_weak = self.downgrade();
            libraries_list.connect_row_activated(move |_, row| {
                if let Some(action_row) = row.downcast_ref::<adw::ActionRow>()
                    && let Some(window) = window_weak.upgrade()
                {
                    let library_id = action_row.widget_name().to_string();
                    window.navigate_to_library(&library_id);
                }
            });

            source_group.add(&libraries_list);
            imp.sources_container.append(&source_group);
        }

        // Store all libraries for edit mode
        imp.all_libraries.replace(all_libraries);

        // Show sources container if we have any backends
        imp.sources_container
            .set_visible(!backends_libraries.is_empty());
    }

    fn show_sources_page(&self) {
        info!("show_sources_page called");
        let imp = self.imp();

        // Prepare for navigation
        self.prepare_navigation();

        // Get or create content stack
        let content_stack = self.ensure_content_stack();

        // Create sources page if it doesn't exist
        if content_stack.child_by_name("sources").is_none()
            && let Some(state) = &*imp.state.borrow()
        {
            // Create sources page with header setup callback
            let header_ref = imp.content_header.clone();
            let add_button_ref = imp.header_add_button.clone();
            let sources_page =
                pages::SourcesPage::new(state.clone(), move |title_label, add_button| {
                    // Set the header title
                    header_ref.set_title_widget(Some(title_label));

                    // Add the button to header and store reference
                    header_ref.pack_end(add_button);
                    add_button_ref.replace(Some(add_button.clone()));
                });

            content_stack.add_named(&sources_page, Some("sources"));
            imp.sources_page.replace(Some(sources_page));
        }

        // Show the sources page
        content_stack.set_visible_child_name("sources");
    }

    fn clear_header_end_widgets(&self) {
        let imp = self.imp();

        // Remove the add button if it exists
        if let Some(button) = imp.header_add_button.borrow().as_ref() {
            imp.content_header.remove(button);
        }
        imp.header_add_button.replace(None);
    }

    fn show_home_page_for_source(&self, source_id: Option<String>) {
        let imp = self.imp();

        // Prepare for navigation
        self.prepare_navigation();

        // Get or create content stack
        let content_stack = self.ensure_content_stack();

        // Create home page if it doesn't exist
        if content_stack.child_by_name("home").is_none() {
            if let Some(state) = &*imp.state.borrow() {
                // Create home page with header setup and navigation callbacks
                let header_ref = imp.content_header.clone();
                let window_weak = self.downgrade();

                let home_page = pages::HomePage::new(
                    state.clone(),
                    source_id.clone(), // Pass the source filter
                    move |title_widget| {
                        // Set the header title widget (can be label or more complex widget)
                        header_ref.set_title_widget(Some(title_widget));
                    },
                    move |nav_request| {
                        if let Some(window) = window_weak.upgrade() {
                            glib::spawn_future_local(async move {
                                window.navigate_to(nav_request).await;
                            });
                        }
                    },
                );

                content_stack.add_named(&home_page, Some("home"));
                imp.home_page.replace(Some(home_page));
            }
        } else {
            // Refresh the home page if it already exists and restore its header
            if let Some(home_page) = &*imp.home_page.borrow() {
                // Re-setup the header with source selector since it was cleared during navigation
                let header_ref = imp.content_header.clone();
                home_page.setup_header_with_selector(move |title_widget| {
                    header_ref.set_title_widget(Some(title_widget));
                });
                home_page.refresh();
            }
        }

        // Show the home page
        content_stack.set_visible_child_name("home");
    }

    pub async fn trigger_sync(&self, state: Arc<AppState>) {
        info!("Manually triggering sync for all backends...");

        // Sync all backends
        self.sync_all_backends(state).await;
    }

    pub async fn sync_and_update_libraries(
        &self,
        backend_id: &str,
        backend: Arc<dyn crate::backends::MediaBackend>,
        state: Arc<AppState>,
    ) {
        info!("Starting library sync for backend: {}", backend_id);

        // Get sync manager from state
        let sync_manager = state.sync_manager.clone();

        // Perform sync
        match sync_manager.sync_backend(backend_id, backend).await {
            Ok(result) => {
                info!(
                    "Sync completed for {}: {} items synced",
                    backend_id, result.items_synced
                );
            }
            Err(e) => {
                error!("Sync failed for {}: {}", backend_id, e);
            }
        }

        // After syncing one backend, refresh all libraries display
        self.refresh_all_libraries().await;

        // Check if we should navigate to home page
        let imp = self.imp();
        let should_show_home = if let Some(stack) = imp.content_stack.borrow().as_ref() {
            // Check if we have any actual content pages
            let has_content = stack.child_by_name("library").is_some()
                || stack.child_by_name("movie_details").is_some()
                || stack.child_by_name("show_details").is_some()
                || stack.child_by_name("player").is_some();

            !has_content
        } else {
            true
        };

        if should_show_home {
            self.show_home_page_for_source(None);
        } else {
            // Just refresh the home page data if it exists
            if let Some(home_page) = &*imp.home_page.borrow() {
                home_page.refresh();
            }
        }
    }

    pub async fn sync_all_backends(&self, state: Arc<AppState>) {
        info!("Starting sync for all backends...");

        let all_backends = state.source_coordinator.get_all_backends().await;

        // Sync progress is now managed by SidebarViewModel

        // Sync each backend sequentially
        for (backend_id, backend) in all_backends {
            info!("Syncing backend: {}", backend_id);

            let sync_manager = state.sync_manager.clone();
            match sync_manager.sync_backend(&backend_id, backend).await {
                Ok(result) => {
                    info!(
                        "Backend {} synced: {} items",
                        backend_id, result.items_synced
                    );
                }
                Err(e) => {
                    error!("Failed to sync backend {}: {}", backend_id, e);
                }
            }
        }

        // Sync progress is now managed by SidebarViewModel

        // Refresh all libraries display
        self.refresh_all_libraries().await;

        // Navigate to home if appropriate
        let imp = self.imp();
        let should_show_home = if let Some(stack) = imp.content_stack.borrow().as_ref() {
            stack.child_by_name("library").is_none()
                && stack.child_by_name("movie_details").is_none()
                && stack.child_by_name("show_details").is_none()
                && stack.child_by_name("player").is_none()
        } else {
            true
        };

        if should_show_home {
            self.show_home_page_for_source(None);
        } else if let Some(home_page) = &*imp.home_page.borrow() {
            home_page.refresh();
        }
    }

    pub async fn sync_single_backend(&self, backend_id: &str) {
        info!("Starting sync for backend: {}", backend_id);

        let state = self.imp().state.borrow().as_ref().unwrap().clone();

        if let Some(backend) = state.source_coordinator.get_backend(backend_id).await {
            // Sync progress is now managed by SidebarViewModel

            let sync_manager = state.sync_manager.clone();
            match sync_manager.sync_backend(backend_id, backend).await {
                Ok(result) => {
                    info!(
                        "Backend {} synced: {} items",
                        backend_id, result.items_synced
                    );
                }
                Err(e) => {
                    error!("Failed to sync backend {}: {}", backend_id, e);
                }
            }

            // Sync progress is now managed by SidebarViewModel

            // Refresh all libraries display
            self.refresh_all_libraries().await;
        } else {
            error!("Backend {} not found", backend_id);
        }
    }

    pub fn navigate_to_library(&self, library_id: &str) {
        info!("Navigating to library: {}", library_id);

        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let library_id = library_id.to_string();
        let window_weak = self.downgrade();

        glib::spawn_future_local(async move {
            if let Some(window) = window_weak.upgrade() {
                // Check if library_id contains backend_id
                let (backend_id, actual_library_id) = if library_id.contains(':') {
                    let parts: Vec<&str> = library_id.splitn(2, ':').collect();
                    (parts[0].to_string(), parts[1].to_string())
                } else {
                    // Library ID must include backend_id
                    error!(
                        "Library ID '{}' does not include backend_id separator ':'",
                        library_id
                    );
                    return;
                };

                let sync_manager = state.sync_manager.clone();

                // Get the library from cache
                match sync_manager.get_cached_libraries(&backend_id).await {
                    Ok(libraries) => {
                        if let Some(library) = libraries.iter().find(|l| l.id == actual_library_id)
                        {
                            window.show_library_view(backend_id, library.clone()).await;
                        } else {
                            error!("Library not found: {}", actual_library_id);
                        }
                    }
                    Err(e) => {
                        error!("Failed to get libraries: {}", e);
                    }
                }
            }
        });
    }

    async fn move_backend_up(&self, backend_id: &str) {
        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        state
            .source_coordinator
            .move_backend_up(backend_id)
            .await
            .ok();

        // Refresh the display
        self.refresh_all_libraries().await;
    }

    async fn move_backend_down(&self, backend_id: &str) {
        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        state
            .source_coordinator
            .move_backend_down(backend_id)
            .await
            .ok();

        // Refresh the display
        self.refresh_all_libraries().await;
    }

    async fn refresh_all_libraries(&self) {
        // Only refresh the SidebarViewModel - it will handle updating the UI via reactive properties
        if let Some(sidebar_vm) = self.imp().sidebar_viewmodel.borrow().as_ref() {
            use super::viewmodels::ViewModel;
            sidebar_vm.refresh().await;
            tracing::info!("Refreshed SidebarViewModel - UI will update via reactive properties");
        } else {
            tracing::warn!("SidebarViewModel not found during refresh_all_libraries");
        }
    }

    pub async fn show_movie_details(&self, movie: crate::models::Movie, state: Arc<AppState>) {
        let imp = self.imp();
        let start_time = std::time::Instant::now();

        // Prepare navigation and get stack
        self.prepare_navigation();
        let content_stack = self.ensure_content_stack();

        // Set transition for details pages
        content_stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);
        content_stack.set_transition_duration(300);

        // Create or get movie details page
        let movie_details_page = if let Some(page) = imp.movie_details_page.borrow().as_ref() {
            // Page exists, make sure it's in the stack
            if content_stack.child_by_name("movie_details").is_none() {
                content_stack.add_named(page, Some("movie_details"));
            }
            page.clone()
        } else {
            // Create new page
            let page = crate::platforms::gtk::ui::pages::MovieDetailsPage::new(state.clone());

            // Set callback for when play is clicked
            let window_weak = self.downgrade();
            page.set_on_play_clicked(move |movie| {
                if let Some(window) = window_weak.upgrade() {
                    let movie_item = crate::models::MediaItem::Movie(movie.clone());
                    glib::spawn_future_local(async move {
                        use super::navigation::NavigationRequest;
                        window
                            .navigate_to(NavigationRequest::ShowPlayer(movie_item))
                            .await;
                    });
                }
            });

            // Store and add to stack
            imp.movie_details_page.replace(Some(page.clone()));
            content_stack.add_named(&page, Some("movie_details"));
            page
        };

        // Update the content page title
        imp.content_page.set_title(&movie.title);

        // Setup back button immediately for better UX
        self.setup_back_button("Back to Library");

        // Start loading the movie data
        movie_details_page.load_movie(movie.clone());

        // Defer the transition until data starts loading (immediate transition for perceived performance)
        // This gives the best of both worlds: immediate response + smooth transition
        glib::timeout_add_local_once(std::time::Duration::from_millis(10), {
            let content_stack = content_stack.clone();
            move || {
                content_stack.set_visible_child_name("movie_details");
            }
        });

        // Performance monitoring
        let elapsed = start_time.elapsed();
        if elapsed > std::time::Duration::from_millis(16) {
            tracing::warn!(
                "show_movie_details took {:?} (exceeds frame budget)",
                elapsed
            );
        }
    }

    pub async fn show_show_details(&self, show: crate::models::Show, state: Arc<AppState>) {
        let imp = self.imp();
        let start_time = std::time::Instant::now();

        // Prepare navigation and get stack
        self.prepare_navigation();
        let content_stack = self.ensure_content_stack();

        // Set transition for details pages
        content_stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);
        content_stack.set_transition_duration(300);

        // Create or get show details page
        let show_details_page = if let Some(page) = imp.show_details_page.borrow().as_ref() {
            // Page exists, make sure it's in the stack
            if content_stack.child_by_name("show_details").is_none() {
                content_stack.add_named(page.widget(), Some("show_details"));
            }
            page.clone()
        } else {
            // Create new page
            let page = crate::platforms::gtk::ui::pages::ShowDetailsPage::new(state.clone());

            // Set callback for when episode is selected
            let window_weak = self.downgrade();
            page.set_on_episode_selected(move |episode| {
                if let Some(window) = window_weak.upgrade() {
                    let episode_item = crate::models::MediaItem::Episode(episode.clone());
                    glib::spawn_future_local(async move {
                        use super::navigation::NavigationRequest;
                        window
                            .navigate_to(NavigationRequest::ShowPlayer(episode_item))
                            .await;
                    });
                }
            });

            // Store and add to stack
            imp.show_details_page.replace(Some(page.clone()));
            content_stack.add_named(page.widget(), Some("show_details"));
            page
        };

        // Update the content page title
        imp.content_page.set_title(&show.title);

        // Setup back button immediately for better UX
        self.setup_back_button("Back to Library");

        // Start loading the show data
        show_details_page.load_show(show.clone());

        // Defer the transition until data starts loading (immediate transition for perceived performance)
        // This gives the best of both worlds: immediate response + smooth transition
        glib::timeout_add_local_once(std::time::Duration::from_millis(10), {
            let content_stack = content_stack.clone();
            move || {
                content_stack.set_visible_child_name("show_details");
            }
        });

        // Performance monitoring
        let elapsed = start_time.elapsed();
        if elapsed > std::time::Duration::from_millis(16) {
            tracing::warn!(
                "show_show_details took {:?} (exceeds frame budget)",
                elapsed
            );
        }
    }

    pub async fn show_player(&self, media_item: &crate::models::MediaItem, state: Arc<AppState>) {
        let start_time = std::time::Instant::now();
        info!(
            "MainWindow::show_player() - Called for media: {}",
            media_item.title()
        );
        debug!(
            "MainWindow::show_player() - Media type: {:?}, ID: {}",
            std::mem::discriminant(media_item),
            media_item.id()
        );

        let imp = self.imp();

        // Prepare navigation and get stack
        self.prepare_navigation();
        let content_stack = self.ensure_content_stack();
        content_stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);
        content_stack.set_transition_duration(300);

        // Create or reuse player page
        // Check if we need to recreate the player page due to backend change
        let current_backend = {
            let config = state.config.read().await;
            config.playback.player_backend.clone()
        };

        let needs_recreation = if let Some(existing_page) = self.imp().player_page.borrow().as_ref()
        {
            // Check if the backend has changed
            let page_backend = existing_page.get_backend_type().await;
            page_backend != current_backend
        } else {
            true // No existing page, need to create
        };

        let player_page = if needs_recreation {
            info!("Creating new PlayerPage with backend: {}", current_backend);

            // Cleanup old player page if it exists
            if let Some(old_page) = self.imp().player_page.borrow().as_ref() {
                old_page.cleanup().await;
            }

            // Remove old player page widget from stack
            if let Some(old_widget) = content_stack.child_by_name("player") {
                content_stack.remove(&old_widget);
            }

            // Create new player page with current backend
            let page = crate::platforms::gtk::ui::pages::PlayerPage::new(state.clone());
            self.imp().player_page.replace(Some(page.clone()));
            content_stack.add_named(page.widget(), Some("player"));
            page
        } else {
            // Reuse existing page
            let page = self.imp().player_page.borrow().as_ref().unwrap().clone();
            if content_stack.child_by_name("player").is_none() {
                content_stack.add_named(page.widget(), Some("player"));
            }
            page
        };

        // Update the content page title first
        self.imp().content_page.set_title(media_item.title());
        debug!("MainWindow::show_player() - Updated content page title");

        // Load the media (but don't block navigation on failure)
        debug!("MainWindow::show_player() - Loading media into player");
        if let Err(e) = player_page.load_media(media_item, state).await {
            error!("MainWindow::show_player() - Failed to load media: {}", e);
            // Show error dialog but still navigate to player page
            let dialog =
                adw::AlertDialog::new(Some("Failed to Load Media"), Some(&format!("Error: {}", e)));
            dialog.add_response("ok", "OK");
            dialog.set_default_response(Some("ok"));
            dialog.present(Some(self));
        }
        // Clear any existing back buttons from the main header
        if let Some(old_button) = imp.back_button.borrow().as_ref() {
            imp.content_header.remove(old_button);
        }
        imp.back_button.replace(None);

        // Also clear any title widget that might have controls
        imp.content_header.set_title_widget(None::<&gtk4::Widget>);

        // Hide the main header bar completely to avoid duplicate back buttons
        imp.content_header.set_visible(false);
        imp.content_toolbar
            .set_top_bar_style(adw::ToolbarStyle::Flat); // Make toolbar flat/hidden

        // Configure back/close actions on the player page OSD buttons
        let window_weak_close = self.downgrade();
        player_page.set_on_close_clicked(move || {
            if let Some(window) = window_weak_close.upgrade() {
                window.close();
            }
        });

        let window_weak = self.downgrade();
        player_page.set_on_back_clicked(move || {
            if let Some(window) = window_weak.upgrade() {
                // Stop the player
                if let Some(player_page) = window.imp().player_page.borrow().as_ref() {
                    let player_page = player_page.clone();
                    glib::spawn_future_local(async move {
                        player_page.stop().await;
                    });
                }

                // Show header bar again and restore toolbar style
                window.imp().content_header.set_visible(true);
                window
                    .imp()
                    .content_toolbar
                    .set_top_bar_style(adw::ToolbarStyle::Raised);

                // Restore window size
                let (width, height) = *window.imp().saved_window_size.borrow();
                window.set_default_size(width, height);

                // Restore sidebar
                if let Some(content) = window.content()
                    && let Some(split_view) = content.downcast_ref::<adw::NavigationSplitView>()
                {
                    split_view.set_collapsed(false);
                    split_view.set_show_content(true);
                }

                // Navigate back to the previous page from navigation stack
                let previous_page = window.imp().navigation_stack.borrow_mut().pop();
                if let Some(page_name) = previous_page {
                    if let Some(stack) = window.imp().content_stack.borrow().as_ref()
                        && stack.child_by_name(&page_name).is_some()
                    {
                        stack.set_visible_child_name(&page_name);
                    }
                } else {
                    // Fallback if no navigation history
                    if let Some(stack) = window.imp().content_stack.borrow().as_ref() {
                        if stack.child_by_name("home").is_some() {
                            stack.set_visible_child_name("home");
                        } else if stack.child_by_name("library").is_some() {
                            stack.set_visible_child_name("library");
                        }
                    }
                }
            }
        });

        // Save current window size before changing it
        let (current_width, current_height) = self.default_size();
        imp.saved_window_size
            .replace((current_width, current_height));

        // Push current page to navigation stack before switching to player
        if let Some(current_page) = content_stack.visible_child_name() {
            imp.navigation_stack
                .borrow_mut()
                .push(current_page.to_string());
            info!(
                "MainWindow::show_player() - Pushed '{}' to navigation stack",
                current_page
            );
        }

        // Defer the transition for smoother performance
        info!("MainWindow::show_player() - Switching stack to 'player' page");
        glib::timeout_add_local_once(std::time::Duration::from_millis(10), {
            let content_stack = content_stack.clone();
            move || {
                content_stack.set_visible_child_name("player");
                info!("MainWindow::show_player() - Navigation to player complete");
            }
        });

        // Performance monitoring
        let elapsed = start_time.elapsed();
        if elapsed > std::time::Duration::from_millis(16) {
            tracing::warn!("show_player took {:?} (exceeds frame budget)", elapsed);
        }

        // Navigate to the content pane and collapse the sidebar for immersive playback
        if let Some(content) = self.content()
            && let Some(split_view) = content.downcast_ref::<adw::NavigationSplitView>()
        {
            // Ensure content is visible first
            split_view.set_show_content(true);
            // Then collapse the sidebar for immersive video playback
            split_view.set_collapsed(true);
        }

        // Try to resize window to match video aspect ratio after a short delay
        // (to give GStreamer time to negotiate the video format)
        let window_weak = self.downgrade();
        let player_page_clone = player_page.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
            let window_weak = window_weak.clone();
            let player_page = player_page_clone.clone();
            glib::spawn_future_local(async move {
                if let Some(window) = window_weak.upgrade()
                    && let Some((width, height)) = player_page.get_video_dimensions().await
                {
                    // Calculate aspect ratio
                    let aspect_ratio = width as f64 / height as f64;

                    // Calculate new width based on aspect ratio
                    // Use a reasonable height (e.g., 720p)
                    let target_height = 720.min(height).max(480);
                    let target_width = (target_height as f64 * aspect_ratio) as i32;

                    // Set the new window size
                    window.set_default_size(target_width, target_height);

                    info!(
                        "Resized window to {}x{} (aspect ratio: {:.2})",
                        target_width, target_height, aspect_ratio
                    );
                }
            });
            glib::ControlFlow::Break
        });
    }

    async fn show_library_view(&self, backend_id: String, library: crate::models::Library) {
        let imp = self.imp();
        let start_time = std::time::Instant::now();

        // Prepare navigation and get stack
        self.prepare_navigation();
        let content_stack = self.ensure_content_stack();

        // Set transition for library pages
        content_stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);
        content_stack.set_transition_duration(300);

        // Create or get library view
        let library_view = {
            // Check if we already have a library view
            let existing_view = imp.library_view.borrow().as_ref().cloned();

            if let Some(view) = existing_view {
                view
            } else {
                // Create new library view - use virtual scrolling for production
                let state = imp.state.borrow().as_ref().unwrap().clone();

                // Always use virtual scrolling for production-ready performance
                let view = if crate::constants::USE_VIRTUAL_SCROLLING {
                    LibraryViewWrapper::Virtual(
                        crate::platforms::gtk::ui::pages::LibraryVirtualView::new(state.clone()),
                    )
                } else {
                    LibraryViewWrapper::Standard(
                        crate::platforms::gtk::ui::pages::LibraryView::new(state.clone()),
                    )
                };

                // Set the media selected callback to handle different media types
                let window_weak = self.downgrade();
                view.set_on_media_selected(move |media_item| {
                    info!("Library - Media selected: {}", media_item.title());
                    if let Some(window) = window_weak.upgrade() {
                        let media_item = media_item.clone();
                        glib::spawn_future_local(async move {
                            use super::navigation::NavigationRequest;
                            use crate::models::MediaItem;
                            let nav_request = match &media_item {
                                MediaItem::Movie(movie) => {
                                    NavigationRequest::ShowMovieDetails(movie.clone())
                                }
                                MediaItem::Show(show) => {
                                    NavigationRequest::ShowShowDetails(show.clone())
                                }
                                MediaItem::Episode(_) => NavigationRequest::ShowPlayer(media_item),
                                _ => {
                                    info!("Library - Unsupported media type");
                                    return;
                                }
                            };
                            window.navigate_to(nav_request).await;
                        });
                    }
                });

                // Store the view and add to stack
                imp.library_view.replace(Some(view.clone()));
                content_stack.add_named(&view.upcast(), Some("library"));
                view
            }
        };

        // Update the content page title
        imp.content_page.set_title(&library.title);

        // Setup back button
        self.setup_back_button("Back to Libraries");

        // Update header bar title
        imp.content_header.set_title_widget(Some(
            &gtk4::Label::builder()
                .label(&library.title)
                .single_line_mode(true)
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .build(),
        ));

        // Create filter controls for the header bar
        // Remove any existing filter controls to avoid duplicates when navigating
        if let Some(prev_controls) = imp.filter_controls.borrow().as_ref() {
            imp.content_header.remove(prev_controls);
        }
        let filter_controls = self.create_filter_controls(&library_view);
        imp.content_header.pack_end(&filter_controls);
        imp.filter_controls.replace(Some(filter_controls));

        // Start loading the library data
        library_view.load_library(backend_id, library).await;

        // Defer the transition for smoother performance
        // The await above ensures data starts loading before transition
        glib::timeout_add_local_once(std::time::Duration::from_millis(10), {
            let content_stack = content_stack.clone();
            move || {
                content_stack.set_visible_child_name("library");
            }
        });

        // Performance monitoring
        let elapsed = start_time.elapsed();
        if elapsed > std::time::Duration::from_millis(16) {
            tracing::warn!(
                "show_library_view took {:?} (exceeds frame budget)",
                elapsed
            );
        }

        // Get the split view from the window content and show content pane on mobile
        if let Some(content) = self.content()
            && let Some(split_view) = content.downcast_ref::<adw::NavigationSplitView>()
        {
            split_view.set_show_content(true);
        }
    }

    pub fn show_libraries_view(&self) {
        info!("Navigating back to libraries");

        let imp = self.imp();

        // Prepare navigation (clears headers)
        self.prepare_navigation();

        // Show empty state in content area
        if let Some(stack) = imp.content_stack.borrow().as_ref() {
            stack.set_visible_child_name("empty");
        }

        // Reset content page title
        imp.content_page.set_title("Content");

        // Clear back button completely
        if let Some(back_button) = imp.back_button.borrow().as_ref() {
            imp.content_header.remove(back_button);
        }
        imp.back_button.replace(None);

        // Reset header bar title
        imp.content_header.set_title_widget(gtk4::Widget::NONE);

        // Show sidebar in mobile view
        if let Some(content) = self.content()
            && let Some(split_view) = content.downcast_ref::<adw::NavigationSplitView>()
        {
            split_view.set_show_content(false);
        }
    }

    fn create_filter_controls(&self, library_view: &LibraryViewWrapper) -> gtk4::Box {
        let controls_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .build();

        // Create watch status dropdown
        let watch_status_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(4)
            .build();

        let watch_label = gtk4::Label::builder().label("Show:").build();
        watch_label.add_css_class("dim-label");

        let watch_model = gtk4::StringList::new(&["All", "Unwatched", "Watched", "In Progress"]);

        let watch_dropdown = gtk4::DropDown::builder()
            .model(&watch_model)
            .selected(0)
            .tooltip_text("Filter by watch status")
            .build();

        watch_status_box.append(&watch_label);
        watch_status_box.append(&watch_dropdown);

        // Create sort order dropdown
        let sort_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(4)
            .build();

        let sort_label = gtk4::Label::builder().label("Sort:").build();
        sort_label.add_css_class("dim-label");

        let sort_model = gtk4::StringList::new(&[
            "Title (A-Z)",
            "Title (Z-A)",
            "Year (Oldest)",
            "Year (Newest)",
            "Rating (Low-High)",
            "Rating (High-Low)",
            "Date Added (Oldest)",
            "Date Added (Newest)",
        ]);

        let sort_dropdown = gtk4::DropDown::builder()
            .model(&sort_model)
            .selected(0)
            .tooltip_text("Sort order")
            .build();

        sort_box.append(&sort_label);
        sort_box.append(&sort_dropdown);

        // Connect watch status filter handler
        let library_view_weak = library_view.downgrade();
        watch_dropdown.connect_selected_notify(move |dropdown| {
            if let Some(view) = library_view_weak.upgrade() {
                let status = match dropdown.selected() {
                    0 => WatchStatus::All,
                    1 => WatchStatus::Unwatched,
                    2 => WatchStatus::Watched,
                    3 => WatchStatus::InProgress,
                    _ => WatchStatus::All,
                };
                view.update_watch_status_filter(status);
            }
        });

        // Connect sort order handler
        let library_view_weak = library_view.downgrade();
        sort_dropdown.connect_selected_notify(move |dropdown| {
            if let Some(view) = library_view_weak.upgrade() {
                let order = match dropdown.selected() {
                    0 => SortOrder::TitleAsc,
                    1 => SortOrder::TitleDesc,
                    2 => SortOrder::YearAsc,
                    3 => SortOrder::YearDesc,
                    4 => SortOrder::RatingAsc,
                    5 => SortOrder::RatingDesc,
                    6 => SortOrder::DateAddedAsc,
                    7 => SortOrder::DateAddedDesc,
                    _ => SortOrder::TitleAsc,
                };
                view.update_sort_order(order);
            }
        });

        // Add controls to the box
        controls_box.append(&watch_status_box);
        controls_box.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
        controls_box.append(&sort_box);

        // Future: Add more filter buttons here (genre, year range, etc.)
        // Example:
        // let filter_button = gtk4::Button::builder()
        //     .icon_name("view-filter-symbolic")
        //     .tooltip_text("More filters")
        //     .build();
        // controls_box.append(&filter_button);

        controls_box
    }

    fn toggle_edit_mode(&self, _button: &gtk4::Button) {
        // Edit mode is now handled per-source group
        // This method might be removed or repurposed
    }

    async fn load_library_visibility(&self) {
        // Load from existing config
        if let Some(config_arc) = self.imp().config.borrow().as_ref() {
            let visibility = {
                let config = config_arc.read().await;
                config.get_all_library_visibility()
            };
            *self.imp().library_visibility.borrow_mut() = visibility;
        }
    }

    async fn save_library_visibility(&self) {
        // Save to config using proper methods
        let visibility = self.imp().library_visibility.borrow().clone();

        if let Some(config_arc) = self.imp().config.borrow().as_ref() {
            let mut config = config_arc.write().await;
            if let Err(e) = config.set_all_library_visibility(visibility) {
                error!("Failed to save library visibility: {}", e);
            }
        }
    }

    pub async fn update_backend_selector(&self) {
        // Sources button is always visible at the bottom - no selector needed
    }

    // Generic navigation handler
    pub async fn navigate_to(
        &self,
        request: crate::platforms::gtk::ui::navigation::NavigationRequest,
    ) {
        use super::navigation::NavigationRequest;

        let state = self.imp().state.borrow().as_ref().unwrap().clone();

        match request {
            NavigationRequest::ShowHome(source_id) => {
                self.show_home_page_for_source(source_id);
            }
            NavigationRequest::ShowSources => {
                self.show_sources_page();
            }
            NavigationRequest::ShowMovieDetails(movie) => {
                self.show_movie_details(movie, state).await;
            }
            NavigationRequest::ShowShowDetails(show) => {
                self.show_show_details(show, state).await;
            }
            NavigationRequest::ShowPlayer(media_item) => {
                self.show_player(&media_item, state).await;
            }
            NavigationRequest::ShowLibrary(backend_id, library) => {
                self.show_library_view(backend_id, library).await;
            }
            NavigationRequest::GoBack => {
                // Navigate back in history
                if let Some(stack) = self.imp().content_stack.borrow().as_ref() {
                    // Check if we're currently on the player page and clean it up
                    if let Some(current_page) = stack.visible_child_name() {
                        if current_page == "player" {
                            // Stop the player before navigating away
                            if let Some(player_page) = self.imp().player_page.borrow().as_ref() {
                                let player_page = player_page.clone();
                                let window_self = self.clone();
                                glib::spawn_future_local(async move {
                                    player_page.stop().await;

                                    // Restore UI state
                                    window_self.imp().content_header.set_visible(true);
                                    window_self
                                        .imp()
                                        .content_toolbar
                                        .set_top_bar_style(adw::ToolbarStyle::Raised);

                                    // Restore window size
                                    let (width, height) =
                                        *window_self.imp().saved_window_size.borrow();
                                    window_self.set_default_size(width, height);

                                    // Restore sidebar
                                    if let Some(content) = window_self.content()
                                        && let Some(split_view) =
                                            content.downcast_ref::<adw::NavigationSplitView>()
                                    {
                                        split_view.set_collapsed(false);
                                        split_view.set_show_content(true);
                                    }
                                });
                            }
                        }
                    }

                    // Pop from navigation stack if available
                    if let Some(previous_page) = self.imp().navigation_stack.borrow_mut().pop() {
                        stack.set_visible_child_name(&previous_page);
                    } else {
                        // Default to home
                        self.show_home_page_for_source(None);
                    }
                }
            }
        }
    }

    // Helper to ensure content stack exists
    fn ensure_content_stack(&self) -> gtk4::Stack {
        let imp = self.imp();

        if imp.content_stack.borrow().is_none() {
            let stack = gtk4::Stack::new();
            stack.add_named(&*imp.empty_state, Some("empty"));
            imp.content_toolbar.set_content(Some(&stack));
            imp.content_stack.replace(Some(stack.clone()));
            stack
        } else {
            imp.content_stack.borrow().as_ref().unwrap().clone()
        }
    }

    // Helper to prepare for page navigation
    fn prepare_navigation(&self) {
        let imp = self.imp();

        // Clear header end widgets
        self.clear_header_end_widgets();

        // Remove any filter controls
        if let Some(filter_controls) = imp.filter_controls.borrow().as_ref() {
            imp.content_header.remove(filter_controls);
        }
        imp.filter_controls.replace(None);
    }

    // Helper to setup back button
    fn setup_back_button(&self, tooltip: &str) {
        let imp = self.imp();

        // Remove any existing back button
        if let Some(old_button) = imp.back_button.borrow().as_ref() {
            imp.content_header.remove(old_button);
        }

        // Create a new back button
        let back_button = gtk4::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text(tooltip)
            .build();

        let window_weak = self.downgrade();
        back_button.connect_clicked(move |_| {
            if let Some(window) = window_weak.upgrade() {
                glib::spawn_future_local(async move {
                    use super::navigation::NavigationRequest;
                    window.navigate_to(NavigationRequest::GoBack).await;
                });
            }
        });

        imp.content_header.pack_start(&back_button);
        imp.back_button.replace(Some(back_button));
    }

    // Backend switching removed - each view must track its own backend_id
    // The UI should be refactored to either:
    // 1. Show content from all backends simultaneously
    // 2. Have a backend selector that filters the view
    // 3. Pass backend_id through the navigation hierarchy
}
