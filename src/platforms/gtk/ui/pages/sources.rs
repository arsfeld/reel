use gtk4::{glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info};

use crate::models::{AuthProvider, Source};
use crate::platforms::gtk::ui::viewmodels::sources_view_model::SourcesViewModel;
use crate::services::AuthManager;
use crate::state::AppState;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct SourcesPage {
        pub scrolled_window: gtk4::ScrolledWindow,
        pub main_box: gtk4::Box,
        pub content_box: gtk4::Box,
        pub state: RefCell<Option<Arc<AppState>>>,
        pub auth_manager: RefCell<Option<Arc<AuthManager>>>,
        pub provider_sections: RefCell<HashMap<String, ProviderSection>>,
        pub view_model: RefCell<Option<Arc<SourcesViewModel>>>,
    }

    pub struct ProviderSection {
        pub container: adw::PreferencesGroup,
        pub sources_list: gtk4::ListBox,
        pub provider: AuthProvider,
        pub sources: Vec<Source>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SourcesPage {
        const NAME: &'static str = "SourcesPage";
        type Type = super::SourcesPage;
        type ParentType = gtk4::Box;
    }

    impl ObjectImpl for SourcesPage {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            obj.set_orientation(gtk4::Orientation::Vertical);
            obj.set_vexpand(true);
            obj.set_hexpand(true);

            // Setup scrolled window
            self.scrolled_window
                .set_hscrollbar_policy(gtk4::PolicyType::Never);
            self.scrolled_window
                .set_vscrollbar_policy(gtk4::PolicyType::Automatic);
            self.scrolled_window.set_vexpand(true);
            self.scrolled_window.set_hexpand(true);

            // Setup content box
            self.content_box
                .set_orientation(gtk4::Orientation::Vertical);
            self.content_box.set_spacing(24);
            self.content_box.set_margin_top(24);
            self.content_box.set_margin_bottom(24);
            self.content_box.set_margin_start(24);
            self.content_box.set_margin_end(24);

            self.scrolled_window.set_child(Some(&self.content_box));

            // Add scrolled window to main box
            obj.append(&self.scrolled_window);
        }
    }

    impl WidgetImpl for SourcesPage {}
    impl BoxImpl for SourcesPage {}
}

glib::wrapper! {
    pub struct SourcesPage(ObjectSubclass<imp::SourcesPage>)
        @extends gtk4::Widget, gtk4::Box,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Orientable;
}

impl SourcesPage {
    pub fn new<F>(state: Arc<AppState>, setup_header: F) -> Self
    where
        F: Fn(&gtk4::Label, &gtk4::Button) + 'static,
    {
        let page: Self = glib::Object::builder().build();

        // Create auth manager with config
        let auth_manager = Arc::new(AuthManager::new(
            state.config.clone(),
            state.event_bus.clone(),
        ));

        // Initialize SourcesViewModel
        let data_service = state.data_service.clone();
        let view_model = Arc::new(SourcesViewModel::new(data_service));
        page.imp().view_model.replace(Some(view_model.clone()));

        // Initialize ViewModel with EventBus
        glib::spawn_future_local({
            let vm = view_model.clone();
            let event_bus = state.event_bus.clone();
            async move {
                use crate::platforms::gtk::ui::viewmodels::ViewModel;
                vm.initialize(event_bus).await;
            }
        });

        // Setup ViewModel bindings
        page.setup_viewmodel_bindings(view_model);

        page.imp().state.replace(Some(state.clone()));
        page.imp().auth_manager.replace(Some(auth_manager.clone()));

        // Setup header with title and add button
        let title_label = gtk4::Label::new(Some("Sources & Accounts"));
        let add_button = gtk4::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("Add Source")
            .css_classes(vec!["suggested-action"])
            .build();

        // Connect add button to show dialog
        let page_weak = page.downgrade();
        add_button.connect_clicked(move |_| {
            if let Some(page) = page_weak.upgrade() {
                page.show_add_source_dialog();
            }
        });

        // Call the header setup callback
        setup_header(&title_label, &add_button);

        // Load existing providers
        page.load_providers();

        page
    }

    fn setup_viewmodel_bindings(&self, view_model: Arc<SourcesViewModel>) {
        let weak_self = self.downgrade();

        // Subscribe to sources changes
        let mut sources_subscriber = view_model.sources().subscribe();
        glib::spawn_future_local(async move {
            while sources_subscriber.wait_for_change().await {
                if let Some(page) = weak_self.upgrade() {
                    // Refresh the sources display when ViewModel updates
                    page.load_providers();
                }
            }
        });

        // Subscribe to sync status
        let weak_self_sync = self.downgrade();
        let mut sync_subscriber = view_model.sources().subscribe();
        glib::spawn_future_local(async move {
            while sync_subscriber.wait_for_change().await {
                if let Some(page) = weak_self_sync.upgrade()
                    && let Some(vm) = &*page.imp().view_model.borrow()
                {
                    // Could update UI to show sync progress
                    let sources = vm.sources().get().await;
                    info!("Sources: {:?}", sources);
                }
            }
        });
    }

    fn load_providers(&self) {
        let auth_manager = self.imp().auth_manager.borrow().as_ref().unwrap().clone();
        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let page_weak = self.downgrade();

        // First, load cached providers synchronously for instant display
        glib::spawn_future_local(async move {
            if let Some(page) = page_weak.upgrade() {
                // Load persisted providers from config (this is fast, from disk)
                if let Err(e) = auth_manager.load_providers().await {
                    error!("Failed to load auth providers: {}", e);
                }

                // Get all providers (from memory, instant)
                let providers = auth_manager.get_all_providers().await;

                // Clear existing content first
                let imp = page.imp();
                while let Some(child) = imp.content_box.first_child() {
                    imp.content_box.remove(&child);
                }

                if providers.is_empty() {
                    page.show_empty_state();
                } else {
                    // Load cached UI immediately
                    for provider in &providers {
                        page.add_provider_section_cached(provider.clone()).await;
                    }

                    // Then refresh in background
                    page.refresh_providers_background(providers).await;
                }

                // Migrate any legacy backends in background if needed
                let auth_manager_clone = auth_manager.clone();
                tokio::spawn(async move {
                    if let Err(e) = auth_manager_clone.migrate_legacy_backends().await {
                        error!("Failed to migrate legacy backends: {}", e);
                    }
                });
            }
        });
    }

    async fn refresh_providers_background(&self, providers: Vec<AuthProvider>) {
        let auth_manager = self.imp().auth_manager.borrow().as_ref().unwrap().clone();
        let page_weak = self.downgrade();

        // Refresh each provider's sources in background
        for provider in providers {
            if let AuthProvider::PlexAccount { id, .. } = &provider {
                let auth_manager_clone = auth_manager.clone();
                let provider_id = id.clone();
                let page_weak_clone = page_weak.clone();

                glib::spawn_future_local(async move {
                    // This will fetch fresh data and update cache
                    match auth_manager_clone.discover_plex_sources(&provider_id).await {
                        Ok(sources) => {
                            info!(
                                "Refreshed {} sources for provider {}",
                                sources.len(),
                                provider_id
                            );

                            // Update UI if needed
                            if let Some(page) = page_weak_clone.upgrade() {
                                page.update_provider_sources(&provider_id, sources).await;
                            }
                        }
                        Err(e) => {
                            error!(
                                "Failed to refresh sources for provider {}: {}",
                                provider_id, e
                            );
                        }
                    }
                });
            }
        }
    }

    async fn add_provider_section_cached(&self, provider: AuthProvider) {
        let imp = self.imp();

        // Create preferences group for this provider
        let group = adw::PreferencesGroup::new();

        // Set title based on provider type
        let title = match &provider {
            AuthProvider::PlexAccount { username, .. } => {
                format!("Plex · {}", username)
            }
            AuthProvider::JellyfinAuth {
                server_url,
                username,
                ..
            } => {
                format!("Jellyfin · {} @ {}", username, server_url)
            }
            AuthProvider::NetworkCredentials { display_name, .. } => {
                format!("Network · {}", display_name)
            }
            AuthProvider::LocalFiles { .. } => "Local Files".to_string(),
        };

        group.set_title(&title);

        // Add header suffix buttons
        let header_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .build();

        let settings_button = gtk4::Button::builder()
            .icon_name("emblem-system-symbolic")
            .tooltip_text("Settings")
            .build();
        settings_button.add_css_class("flat");

        let remove_button = gtk4::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Remove")
            .build();
        remove_button.add_css_class("flat");

        header_box.append(&settings_button);
        header_box.append(&remove_button);
        group.set_header_suffix(Some(&header_box));

        // Create list for sources
        let sources_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .build();
        sources_list.add_css_class("boxed-list");

        // Load cached sources for this provider
        if let AuthProvider::PlexAccount { id, .. } = &provider {
            let auth_manager = self.imp().auth_manager.borrow().as_ref().unwrap().clone();

            // Try to get cached sources first (instant)
            if let Some(sources) = auth_manager.get_cached_sources(id).await {
                info!(
                    "Loading {} cached sources for provider {}",
                    sources.len(),
                    id
                );
                for source in &sources {
                    let row = self.create_source_row(source);
                    sources_list.append(&row);
                }

                // Store the section
                let section = imp::ProviderSection {
                    container: group.clone(),
                    sources_list: sources_list.clone(),
                    provider: provider.clone(),
                    sources: sources.clone(),
                };
                imp.provider_sections
                    .borrow_mut()
                    .insert(provider.id().to_string(), section);
            } else {
                // No cached sources, show loading indicator
                let loading_row = adw::ActionRow::builder()
                    .title("Loading servers...")
                    .build();
                sources_list.append(&loading_row);
            }
        }

        group.add(&sources_list);
        imp.content_box.append(&group);

        // Connect handlers
        let provider_id = provider.id().to_string();
        let page_weak = self.downgrade();

        settings_button.connect_clicked(glib::clone!(
            #[strong]
            provider_id,
            move |_| {
                if let Some(page) = page_weak.upgrade() {
                    page.show_provider_settings(&provider_id);
                }
            }
        ));

        let page_weak = self.downgrade();
        remove_button.connect_clicked(glib::clone!(
            #[strong]
            provider_id,
            move |_| {
                if let Some(page) = page_weak.upgrade() {
                    page.confirm_remove_provider(&provider_id);
                }
            }
        ));
    }

    async fn update_provider_sources(&self, provider_id: &str, sources: Vec<Source>) {
        let imp = self.imp();

        // Update the provider section if it exists
        if let Some(section) = imp.provider_sections.borrow_mut().get_mut(provider_id) {
            // Clear existing rows
            while let Some(child) = section.sources_list.first_child() {
                section.sources_list.remove(&child);
            }

            // Add updated sources
            for source in &sources {
                let row = self.create_source_row(source);
                section.sources_list.append(&row);
            }

            // Update stored sources
            section.sources = sources;
        }
    }

    fn show_empty_state(&self) {
        let imp = self.imp();

        // Clear existing content
        while let Some(child) = imp.content_box.first_child() {
            imp.content_box.remove(&child);
        }

        // Create exciting empty state with big buttons
        let empty_state = adw::StatusPage::builder()
            .icon_name("applications-multimedia-symbolic")
            .title("Connect Your Media")
            .description("Choose your media server to start streaming")
            .build();

        // Create button box
        let button_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(16)
            .halign(gtk4::Align::Center)
            .margin_top(24)
            .build();

        // Create Plex button
        let plex_button = gtk4::Button::builder().build();

        let plex_content = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(24)
            .margin_end(24)
            .build();

        let plex_icon = gtk4::Image::builder()
            .icon_name("folder-videos-symbolic")
            .pixel_size(32)
            .build();

        let plex_label_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(4)
            .build();

        let plex_label = gtk4::Label::builder()
            .label("Connect to Plex")
            .css_classes(vec!["title-2"])
            .halign(gtk4::Align::Start)
            .build();

        let plex_sublabel = gtk4::Label::builder()
            .label("Stream from your Plex Media Server")
            .css_classes(vec!["dim-label"])
            .halign(gtk4::Align::Start)
            .build();

        plex_label_box.append(&plex_label);
        plex_label_box.append(&plex_sublabel);
        plex_content.append(&plex_icon);
        plex_content.append(&plex_label_box);
        plex_button.set_child(Some(&plex_content));
        plex_button.add_css_class("suggested-action");
        plex_button.add_css_class("pill");

        // Create Jellyfin button
        let jellyfin_button = gtk4::Button::builder().build();

        let jellyfin_content = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(24)
            .margin_end(24)
            .build();

        let jellyfin_icon = gtk4::Image::builder()
            .icon_name("folder-music-symbolic")
            .pixel_size(32)
            .build();

        let jellyfin_label_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(4)
            .build();

        let jellyfin_label = gtk4::Label::builder()
            .label("Connect to Jellyfin")
            .css_classes(vec!["title-2"])
            .halign(gtk4::Align::Start)
            .build();

        let jellyfin_sublabel = gtk4::Label::builder()
            .label("Stream from your Jellyfin Server")
            .css_classes(vec!["dim-label"])
            .halign(gtk4::Align::Start)
            .build();

        jellyfin_label_box.append(&jellyfin_label);
        jellyfin_label_box.append(&jellyfin_sublabel);
        jellyfin_content.append(&jellyfin_icon);
        jellyfin_content.append(&jellyfin_label_box);
        jellyfin_button.set_child(Some(&jellyfin_content));
        jellyfin_button.add_css_class("pill");

        // Connect button handlers
        let page_weak = self.downgrade();
        plex_button.connect_clicked(move |_| {
            if let Some(page) = page_weak.upgrade() {
                page.add_plex_account();
            }
        });

        let page_weak = self.downgrade();
        jellyfin_button.connect_clicked(move |_| {
            if let Some(page) = page_weak.upgrade() {
                page.add_jellyfin_server();
            }
        });

        button_box.append(&plex_button);
        button_box.append(&jellyfin_button);

        empty_state.set_child(Some(&button_box));
        imp.content_box.append(&empty_state);
    }

    async fn add_provider_section(&self, provider: AuthProvider) {
        let imp = self.imp();

        // Create preferences group for this provider
        let group = adw::PreferencesGroup::new();

        // Set title based on provider type
        let title = match &provider {
            AuthProvider::PlexAccount { username, .. } => {
                format!("Plex · {}", username)
            }
            AuthProvider::JellyfinAuth {
                server_url,
                username,
                ..
            } => {
                format!("Jellyfin · {} @ {}", username, server_url)
            }
            AuthProvider::NetworkCredentials { display_name, .. } => {
                format!("Network · {}", display_name)
            }
            AuthProvider::LocalFiles { .. } => "Local Files".to_string(),
        };

        group.set_title(&title);

        // Add header suffix buttons
        let header_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .build();

        let settings_button = gtk4::Button::builder()
            .icon_name("emblem-system-symbolic")
            .tooltip_text("Settings")
            .build();
        settings_button.add_css_class("flat");

        let remove_button = gtk4::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Remove")
            .build();
        remove_button.add_css_class("flat");

        header_box.append(&settings_button);
        header_box.append(&remove_button);
        group.set_header_suffix(Some(&header_box));

        // Create list for sources
        let sources_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .build();
        sources_list.add_css_class("boxed-list");

        // Load sources for this provider
        if let AuthProvider::PlexAccount { .. } = &provider {
            let auth_manager = self.imp().auth_manager.borrow().as_ref().unwrap().clone();
            match auth_manager.discover_plex_sources(provider.id()).await {
                Ok(sources) => {
                    for source in &sources {
                        let row = self.create_source_row(source);
                        sources_list.append(&row);
                    }

                    // Store the section
                    let section = imp::ProviderSection {
                        container: group.clone(),
                        sources_list: sources_list.clone(),
                        provider: provider.clone(),
                        sources: sources.clone(),
                    };
                    imp.provider_sections
                        .borrow_mut()
                        .insert(provider.id().to_string(), section);
                }
                Err(e) => {
                    error!("Failed to discover Plex sources: {}", e);
                    let error_row = adw::ActionRow::builder()
                        .title("Failed to load servers")
                        .subtitle(e.to_string())
                        .build();
                    sources_list.append(&error_row);
                }
            }
        }

        group.add(&sources_list);
        imp.content_box.append(&group);

        // Connect handlers
        let provider_id = provider.id().to_string();
        let page_weak = self.downgrade();

        settings_button.connect_clicked(glib::clone!(
            #[strong]
            provider_id,
            move |_| {
                if let Some(page) = page_weak.upgrade() {
                    page.show_provider_settings(&provider_id);
                }
            }
        ));

        let page_weak = self.downgrade();
        remove_button.connect_clicked(glib::clone!(
            #[strong]
            provider_id,
            move |_| {
                if let Some(page) = page_weak.upgrade() {
                    page.confirm_remove_provider(&provider_id);
                }
            }
        ));
    }

    fn create_source_row(&self, source: &Source) -> adw::ActionRow {
        let row = adw::ActionRow::builder()
            .title(&source.name)
            .activatable(false)
            .build();

        // Add icon
        let icon = gtk4::Image::from_icon_name(source.source_icon());
        row.add_prefix(&icon);

        // Add status indicator
        let status_icon = if source.is_online() {
            gtk4::Image::from_icon_name("emblem-ok-symbolic")
        } else {
            gtk4::Image::from_icon_name("network-offline-symbolic")
        };

        // Add subtitle based on source type and library counts
        let subtitle = match &source.source_type {
            crate::models::SourceType::PlexServer { owned, .. } => {
                let ownership = if *owned { "Owned" } else { "Shared" };
                if source.is_online() {
                    format!("{} server • {} libraries", ownership, source.library_count)
                } else {
                    format!("{} server • Offline", ownership)
                }
            }
            crate::models::SourceType::JellyfinServer => {
                if source.is_online() {
                    format!("Jellyfin • {} libraries", source.library_count)
                } else {
                    "Jellyfin • Offline".to_string()
                }
            }
            _ => {
                if source.is_online() {
                    "Online".to_string()
                } else {
                    "Offline".to_string()
                }
            }
        };
        row.set_subtitle(&subtitle);

        // Add action buttons
        let action_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .build();

        let sync_button = gtk4::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Sync")
            .build();
        sync_button.add_css_class("flat");

        let settings_button = gtk4::Button::builder()
            .icon_name("emblem-system-symbolic")
            .tooltip_text("Settings")
            .build();
        settings_button.add_css_class("flat");

        action_box.append(&status_icon);
        action_box.append(&sync_button);
        action_box.append(&settings_button);

        row.add_suffix(&action_box);

        row
    }

    pub fn show_add_source_dialog(&self) {
        info!("Showing add source dialog");

        // Create a dialog to select source type
        let dialog = adw::MessageDialog::builder()
            .title("Add Source")
            .body("Choose the type of media source to add")
            .modal(true)
            .transient_for(&self.root().and_downcast::<gtk4::Window>().unwrap())
            .build();

        dialog.add_response("cancel", "Cancel");
        dialog.add_response("plex", "Plex Account");
        dialog.add_response("jellyfin", "Jellyfin Server");
        dialog.add_response("network", "Network Share");
        dialog.add_response("local", "Local Files");

        dialog.set_response_appearance("plex", adw::ResponseAppearance::Suggested);

        let page_weak = self.downgrade();
        dialog.connect_response(None, move |_, response| {
            if let Some(page) = page_weak.upgrade() {
                match response {
                    "plex" => page.add_plex_account(),
                    "jellyfin" => page.add_jellyfin_server(),
                    "network" => page.add_network_share(),
                    "local" => page.add_local_files(),
                    _ => {}
                }
            }
        });

        dialog.present();
    }

    pub fn add_plex_account(&self) {
        info!("Adding Plex account");

        // Use the existing auth dialog
        if let Some(state) = self.imp().state.borrow().as_ref() {
            let auth_dialog = crate::platforms::gtk::ui::AuthDialog::new(state.clone());
            if let Some(window) = self.root().and_downcast::<gtk4::Window>() {
                auth_dialog.present(Some(&window));
                auth_dialog.start_auth();

                // Refresh when dialog closes
                let page_weak = self.downgrade();
                auth_dialog.connect_closed(move |_| {
                    if let Some(page) = page_weak.upgrade() {
                        page.load_providers();
                    }
                });
            }
        }
    }

    pub fn add_jellyfin_server(&self) {
        info!("Adding Jellyfin server");

        if let Some(state) = self.imp().state.borrow().as_ref() {
            let auth_dialog = crate::platforms::gtk::ui::AuthDialog::new(state.clone());
            auth_dialog.set_backend_type(crate::platforms::gtk::ui::BackendType::Jellyfin);

            if let Some(window) = self.root().and_downcast::<gtk4::Window>() {
                auth_dialog.present(Some(&window));

                let page_weak = self.downgrade();
                auth_dialog.connect_closed(move |_| {
                    if let Some(page) = page_weak.upgrade() {
                        page.load_providers();
                    }
                });
            }
        }
    }

    fn add_network_share(&self) {
        info!("Adding network share");
        // TODO: Implement network share dialog
    }

    fn add_local_files(&self) {
        info!("Adding local files");

        let dialog = gtk4::FileDialog::builder()
            .title("Choose Media Directory")
            .modal(true)
            .build();

        let page_weak = self.downgrade();
        if let Some(window) = self.root().and_downcast::<gtk4::Window>() {
            dialog.select_folder(Some(&window), gtk4::gio::Cancellable::NONE, move |result| {
                if let Ok(folder) = result
                    && let Some(page) = page_weak.upgrade()
                {
                    info!("Selected folder: {:?}", folder.path());
                    // TODO: Add local folder as source
                    page.load_providers();
                }
            });
        }
    }

    fn show_provider_settings(&self, provider_id: &str) {
        info!("Showing settings for provider: {}", provider_id);
        // TODO: Implement provider settings dialog
    }

    fn confirm_remove_provider(&self, provider_id: &str) {
        let dialog = adw::MessageDialog::builder()
            .title("Remove Source")
            .body("Are you sure you want to remove this source? This will remove all associated servers and cached data.")
            .modal(true)
            .transient_for(&self.root().and_downcast::<gtk4::Window>().unwrap())
            .build();

        dialog.add_response("cancel", "Cancel");
        dialog.add_response("remove", "Remove");
        dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);

        let provider_id = provider_id.to_string();
        let page_weak = self.downgrade();
        dialog.connect_response(None, move |_, response| {
            if response == "remove"
                && let Some(page) = page_weak.upgrade()
            {
                page.remove_provider(&provider_id);
            }
        });

        dialog.present();
    }

    fn remove_provider(&self, provider_id: &str) {
        info!("Removing provider: {}", provider_id);

        let auth_manager = self.imp().auth_manager.borrow().as_ref().unwrap().clone();
        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let provider_id = provider_id.to_string();
        let page_weak = self.downgrade();

        glib::spawn_future_local(async move {
            // First, remove all sources associated with this provider
            let source_coordinator = state.get_source_coordinator();
            // Get all sources for this provider
            if let Some(sources) = auth_manager.get_cached_sources(&provider_id).await {
                for source in sources {
                    // Remove each source and its backend
                    if let Err(e) = source_coordinator.remove_source(&source.id).await {
                        error!("Failed to remove source {}: {}", source.id, e);
                    }
                }
            }

            // Then remove the provider itself
            match auth_manager.remove_provider(&provider_id).await {
                Ok(()) => {
                    info!("Provider removed successfully");
                    if let Some(page) = page_weak.upgrade() {
                        // Clear the provider section immediately from UI
                        page.imp()
                            .provider_sections
                            .borrow_mut()
                            .remove(&provider_id);
                        // Then reload to refresh the UI
                        page.load_providers();
                    }
                }
                Err(e) => {
                    error!("Failed to remove provider: {}", e);
                    // Show error dialog
                    if let Some(page) = page_weak.upgrade()
                        && let Some(window) = page.root().and_downcast::<gtk4::Window>()
                    {
                        let dialog = adw::AlertDialog::new(
                            Some("Failed to Remove Source"),
                            Some(&format!("Error: {}", e)),
                        );
                        dialog.add_response("ok", "OK");
                        dialog.set_default_response(Some("ok"));
                        dialog.present(Some(&window));
                    }
                }
            }
        });
    }
}
