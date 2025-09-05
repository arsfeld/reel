use gtk4::{glib, prelude::*, subclass::prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;
use tracing::{error, info};

use crate::platforms::gtk::ui::viewmodels::sources_view_model::SourcesViewModel;
use crate::state::AppState;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct SourcesPage {
        pub scrolled_window: gtk4::ScrolledWindow,
        pub main_box: gtk4::Box,
        pub content_box: gtk4::Box,
        pub state: RefCell<Option<Arc<AppState>>>,
        pub view_model: RefCell<Option<Arc<SourcesViewModel>>>,
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

        // No longer need AuthManager in UI layer - sources managed by ViewModel

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
        page.setup_viewmodel_bindings(view_model.clone());

        page.imp().state.replace(Some(state.clone()));

        // Setup header with title and add dropdown button
        let title_label = gtk4::Label::new(Some("Servers & Accounts"));

        // Create button that shows dropdown on click
        let add_button_widget = gtk4::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("Add Account")
            .css_classes(vec!["suggested-action"])
            .build();

        // Create the popover and attach it to the button
        add_button_widget.set_has_frame(true);

        // Connect click to show popover
        let page_weak = page.downgrade();
        add_button_widget.connect_clicked(move |button| {
            // Create and show popover on click
            let popover = gtk4::Popover::new();
            let menu_box = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Vertical)
                .spacing(6)
                .margin_top(6)
                .margin_bottom(6)
                .margin_start(6)
                .margin_end(6)
                .build();

            // Create Plex button with proper layout
            let plex_button = gtk4::Button::builder()
                .css_classes(vec!["flat"])
                .hexpand(true)
                .build();

            let plex_box = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .spacing(8)
                .margin_top(6)
                .margin_bottom(6)
                .margin_start(12)
                .margin_end(12)
                .build();

            let plex_icon = gtk4::Image::from_icon_name("network-server-symbolic");
            let plex_label = gtk4::Label::new(Some("Add Plex Account"));
            plex_label.set_halign(gtk4::Align::Start);

            plex_box.append(&plex_icon);
            plex_box.append(&plex_label);
            plex_button.set_child(Some(&plex_box));

            // Create Jellyfin button with proper layout
            let jellyfin_button = gtk4::Button::builder()
                .css_classes(vec!["flat"])
                .hexpand(true)
                .build();

            let jellyfin_box = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .spacing(8)
                .margin_top(6)
                .margin_bottom(6)
                .margin_start(12)
                .margin_end(12)
                .build();

            let jellyfin_icon = gtk4::Image::from_icon_name("network-workgroup-symbolic");
            let jellyfin_label = gtk4::Label::new(Some("Add Jellyfin Server"));
            jellyfin_label.set_halign(gtk4::Align::Start);

            jellyfin_box.append(&jellyfin_icon);
            jellyfin_box.append(&jellyfin_label);
            jellyfin_button.set_child(Some(&jellyfin_box));

            // Connect handlers
            if let Some(page) = page_weak.upgrade() {
                let page_clone = page.clone();
                let popover_weak = popover.downgrade();
                plex_button.connect_clicked(move |_| {
                    if let Some(popover) = popover_weak.upgrade() {
                        popover.popdown();
                    }
                    page_clone.add_plex_account();
                });

                let page_clone = page.clone();
                let popover_weak = popover.downgrade();
                jellyfin_button.connect_clicked(move |_| {
                    if let Some(popover) = popover_weak.upgrade() {
                        popover.popdown();
                    }
                    page_clone.add_jellyfin_server();
                });
            }

            menu_box.append(&plex_button);
            menu_box.append(&jellyfin_button);
            popover.set_child(Some(&menu_box));
            popover.set_parent(button);
            popover.popup();
        });

        // Call the header setup callback
        setup_header(&title_label, &add_button_widget);

        // Load sources through ViewModel (reactive)
        glib::spawn_future_local({
            let vm = view_model.clone();
            let state_clone = state.clone();
            async move {
                if let Err(e) = vm.load_sources().await {
                    error!("Failed to load sources: {}", e);
                }

                // Fix any sources missing auth_provider_id by refreshing from auth providers
                // This ensures old sources get their auth_provider_id set properly
                info!("Refreshing sources from auth providers to fix missing auth_provider_id");
                if let Err(e) = state_clone.source_coordinator.refresh_all_backends().await {
                    error!("Failed to refresh sources from auth providers: {}", e);
                } else {
                    // Reload sources after refresh to pick up the fixed auth_provider_id values
                    if let Err(e) = vm.load_sources().await {
                        error!("Failed to reload sources after refresh: {}", e);
                    }
                }
            }
        });

        page
    }

    fn setup_viewmodel_bindings(&self, view_model: Arc<SourcesViewModel>) {
        let weak_self = self.downgrade();

        // Subscribe to sources changes - this drives the entire UI
        let mut sources_subscriber = view_model.sources().subscribe();
        glib::spawn_future_local(async move {
            while sources_subscriber.wait_for_change().await {
                if let Some(page) = weak_self.upgrade()
                    && let Some(vm) = &*page.imp().view_model.borrow()
                {
                    // Rebuild UI based on reactive ViewModel state
                    let sources = vm.sources().get().await;
                    page.rebuild_sources_ui(sources).await;
                }
            }
        });

        // Subscribe to loading state
        let weak_self_loading = self.downgrade();
        let mut loading_subscriber = view_model.is_loading().subscribe();
        glib::spawn_future_local(async move {
            while loading_subscriber.wait_for_change().await {
                if let Some(page) = weak_self_loading.upgrade()
                    && let Some(vm) = &*page.imp().view_model.borrow()
                {
                    let is_loading = vm.is_loading().get().await;
                    info!("Sources loading state: {}", is_loading);
                    // UI could show loading spinner if needed
                }
            }
        });
    }

    /// Rebuild the entire sources UI based on reactive ViewModel state
    async fn rebuild_sources_ui(
        &self,
        source_infos: Vec<crate::core::viewmodels::sources_view_model::SourceInfo>,
    ) {
        let imp = self.imp();

        // Clear existing content
        while let Some(child) = imp.content_box.first_child() {
            imp.content_box.remove(&child);
        }

        if source_infos.is_empty() {
            self.show_empty_state();
            return;
        }

        // Group sources by provider type for organized display
        let mut plex_sources = Vec::new();
        let mut jellyfin_sources = Vec::new();
        let mut local_sources = Vec::new();

        for source_info in source_infos {
            match source_info.source.source_type.as_str() {
                "plex" => plex_sources.push(source_info),
                "jellyfin" => jellyfin_sources.push(source_info),
                "local" => local_sources.push(source_info),
                "network" => local_sources.push(source_info),
                "unknown" => {
                    // Skip unknown sources - they indicate data corruption or incomplete setup
                    info!(
                        "Skipping unknown source type for source '{}'",
                        source_info.source.name
                    );
                }
                _ => {
                    info!(
                        "Unexpected source type '{}' for source '{}', skipping",
                        source_info.source.source_type, source_info.source.name
                    );
                }
            }
        }

        // Create sections for each provider type that has sources
        if !plex_sources.is_empty() {
            self.create_provider_section("Plex Servers", plex_sources)
                .await;
        }
        if !jellyfin_sources.is_empty() {
            self.create_provider_section("Jellyfin Servers", jellyfin_sources)
                .await;
        }
        if !local_sources.is_empty() {
            self.create_provider_section("Local Files", local_sources)
                .await;
        }
    }

    /// Create a provider section with grouped sources
    async fn create_provider_section(
        &self,
        title: &str,
        source_infos: Vec<crate::core::viewmodels::sources_view_model::SourceInfo>,
    ) {
        let imp = self.imp();

        // Create preferences group for this provider type
        let group = adw::PreferencesGroup::new();
        group.set_title(title);

        // Create list for sources
        let sources_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .build();
        sources_list.add_css_class("boxed-list");

        // Add each source as a row
        for source_info in source_infos {
            let row = self.create_source_row(&source_info);
            sources_list.append(&row);
        }

        group.add(&sources_list);
        imp.content_box.append(&group);
    }

    // Legacy method - sources are now updated reactively through ViewModel
    // This method is kept for compatibility but should not be used

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

    fn create_source_row(
        &self,
        source_info: &crate::core::viewmodels::sources_view_model::SourceInfo,
    ) -> adw::ActionRow {
        use crate::core::viewmodels::sources_view_model::{ConnectionStatus, SyncStage};

        let source = &source_info.source;
        let friendly_name = source_info.friendly_name();
        let row = adw::ActionRow::builder()
            .title(&friendly_name)
            .activatable(false)
            .build();

        // Add source icon based on source type
        let icon_name = match source.source_type.as_str() {
            "plex" => "network-server-symbolic",
            "jellyfin" => "network-workgroup-symbolic",
            "local" => "folder-symbolic",
            "network" => "folder-remote-symbolic",
            _ => "folder-symbolic", // fallback
        };
        let source_icon = gtk4::Image::from_icon_name(icon_name);
        row.add_prefix(&source_icon);

        // Create subtitle with connection and sync status
        let subtitle = if source_info.sync_progress.is_syncing {
            // Show sync progress details
            let stage_text = match &source_info.sync_progress.current_stage {
                SyncStage::Idle => "Idle".to_string(),
                SyncStage::ConnectingToServer => "Connecting to server...".to_string(),
                SyncStage::DiscoveringLibraries => "Discovering libraries...".to_string(),
                SyncStage::LoadingMovies { library_name } => {
                    format!("Loading movies from {}", library_name)
                }
                SyncStage::LoadingTVShows { library_name } => {
                    format!("Loading TV shows from {}", library_name)
                }
                SyncStage::LoadingEpisodes {
                    show_name,
                    season,
                    current,
                    total,
                } => {
                    format!(
                        "Loading episodes from {} S{:02} ({}/{})",
                        show_name, season, current, total
                    )
                }
                SyncStage::LoadingMusic { library_name } => {
                    format!("Loading music from {}", library_name)
                }
                SyncStage::ProcessingMetadata => "Processing metadata...".to_string(),
                SyncStage::Complete => "Sync complete".to_string(),
                SyncStage::Failed { error } => format!("Sync failed: {}", error),
            };

            let progress_percent =
                (source_info.sync_progress.overall_progress.clamp(0.0, 1.0) * 100.0) as u32;
            format!("{} ({}%)", stage_text, progress_percent)
        } else {
            // Show normal status
            let connection_text = match &source_info.connection_status {
                ConnectionStatus::Connected => "Connected",
                ConnectionStatus::Connecting => "Connecting...",
                ConnectionStatus::Disconnected => "Offline",
                ConnectionStatus::Error(err) => &format!("Error: {}", err),
            };

            let library_count = source_info.libraries.len();
            match source.source_type.as_str() {
                "plex" => {
                    format!(
                        "Plex server • {} • {} libraries",
                        connection_text, library_count
                    )
                }
                "jellyfin" => {
                    format!(
                        "Jellyfin • {} • {} libraries",
                        connection_text, library_count
                    )
                }
                "local" => {
                    format!("Local folder • {} libraries", library_count)
                }
                "network" => {
                    format!("Network share • {} libraries", library_count)
                }
                _ => {
                    format!("{} • {} libraries", connection_text, library_count)
                }
            }
        };
        row.set_subtitle(&subtitle);

        // Create suffix box with status indicators and actions
        let suffix_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .build();

        // Connection status indicator
        let status_icon = match &source_info.connection_status {
            ConnectionStatus::Connected => {
                let icon = gtk4::Image::from_icon_name("emblem-ok-symbolic");
                icon.set_tooltip_text(Some("Connected"));
                icon.add_css_class("success");
                icon
            }
            ConnectionStatus::Connecting => {
                // Use a syncing icon instead of spinner for now
                let icon = gtk4::Image::from_icon_name("emblem-synchronizing-symbolic");
                icon.set_tooltip_text(Some("Connecting..."));
                icon.add_css_class("accent");
                icon
            }
            ConnectionStatus::Disconnected => {
                let icon = gtk4::Image::from_icon_name("network-offline-symbolic");
                icon.set_tooltip_text(Some("Disconnected"));
                icon.add_css_class("warning");
                icon
            }
            ConnectionStatus::Error(err) => {
                let icon = gtk4::Image::from_icon_name("dialog-error-symbolic");
                icon.set_tooltip_text(Some(&format!("Error: {}", err)));
                icon.add_css_class("error");
                icon
            }
        };

        // Add progress bar if syncing
        if source_info.sync_progress.is_syncing {
            let progress_bar = gtk4::ProgressBar::new();
            let progress_fraction =
                source_info.sync_progress.overall_progress.clamp(0.0, 1.0) as f64;
            progress_bar.set_fraction(progress_fraction);
            progress_bar.set_show_text(false);
            progress_bar.set_width_request(100);
            progress_bar.set_valign(gtk4::Align::Center);
            progress_bar.set_halign(gtk4::Align::Fill);

            // Add CSS classes based on sync stage
            match source_info.sync_progress.current_stage {
                SyncStage::Failed { .. } => progress_bar.add_css_class("error"),
                SyncStage::Complete => progress_bar.add_css_class("success"),
                _ => progress_bar.add_css_class("accent"),
            }

            suffix_box.append(&progress_bar);
        }

        // Sync button (changes to stop when syncing)
        let sync_button = if source_info.sync_progress.is_syncing {
            let button = gtk4::Button::builder()
                .icon_name("process-stop-symbolic")
                .tooltip_text("Stop sync")
                .build();
            button.add_css_class("flat");
            button.add_css_class("destructive-action");
            button
        } else {
            let button = gtk4::Button::builder()
                .icon_name("view-refresh-symbolic")
                .tooltip_text("Sync now")
                .build();
            button.add_css_class("flat");
            button
        };

        // Remove button
        let remove_button = gtk4::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Remove source")
            .build();
        remove_button.add_css_class("flat");
        remove_button.add_css_class("destructive-action");

        // Connect sync button
        let source_id = source.id.clone();
        let page_weak = self.downgrade();
        sync_button.connect_clicked(move |_| {
            if let Some(page) = page_weak.upgrade() {
                if let Some(vm) = &*page.imp().view_model.borrow() {
                    let source_id_clone = source_id.clone();
                    let vm_clone = vm.clone();
                    glib::spawn_future_local(async move {
                        // TODO: Add stop sync functionality
                        if let Err(e) = vm_clone.sync_source(source_id_clone).await {
                            error!("Failed to sync source: {}", e);
                        }
                    });
                }
            }
        });

        // Connect remove button - remove auth provider, not just source
        let auth_provider_id = source.auth_provider_id.clone();
        let source_name = source.name.clone();
        let source_id = source.id.clone();
        let page_weak = self.downgrade();

        // Only enable remove button if we have an auth provider ID
        if auth_provider_id.is_none() {
            remove_button.set_sensitive(false);
            remove_button.set_tooltip_text(Some("Cannot remove: no auth provider associated"));
        }

        remove_button.connect_clicked(move |_| {
            if let Some(page) = page_weak.upgrade() {
                if let Some(provider_id) = &auth_provider_id {
                    page.confirm_remove_provider(provider_id, &source_name);
                } else {
                    error!("Cannot remove source {}: no auth provider ID", source_id);
                }
            }
        });

        suffix_box.append(&status_icon);
        suffix_box.append(&sync_button);
        suffix_box.append(&remove_button);

        row.add_suffix(&suffix_box);

        row
    }

    pub fn add_plex_account(&self) {
        info!("Adding Plex account");

        // Use the existing auth dialog
        if let Some(state) = self.imp().state.borrow().as_ref() {
            let auth_dialog =
                crate::platforms::gtk::ui::auth_dialog::ReelAuthDialog::new(state.clone());
            if let Some(window) = self.root().and_downcast::<gtk4::Window>() {
                auth_dialog.present(Some(&window));
                auth_dialog.start_authentication();

                // Refresh when dialog closes
                let page_weak = self.downgrade();
                auth_dialog.connect_closed(move |_| {
                    if let Some(page) = page_weak.upgrade()
                        && let Some(vm) = &*page.imp().view_model.borrow()
                    {
                        // Refresh sources through ViewModel after auth dialog closes
                        glib::spawn_future_local({
                            let vm = vm.clone();
                            async move {
                                if let Err(e) = vm.load_sources().await {
                                    error!("Failed to refresh sources after auth: {}", e);
                                }
                            }
                        });
                    }
                });
            }
        }
    }

    pub fn add_jellyfin_server(&self) {
        info!("Adding Jellyfin server");

        if let Some(state) = self.imp().state.borrow().as_ref() {
            let auth_dialog =
                crate::platforms::gtk::ui::auth_dialog::ReelAuthDialog::new(state.clone());
            auth_dialog
                .set_backend_type(crate::platforms::gtk::ui::auth_dialog::BackendType::Jellyfin);

            if let Some(window) = self.root().and_downcast::<gtk4::Window>() {
                auth_dialog.present(Some(&window));

                let page_weak = self.downgrade();
                auth_dialog.connect_closed(move |_| {
                    if let Some(page) = page_weak.upgrade()
                        && let Some(vm) = &*page.imp().view_model.borrow()
                    {
                        // Refresh sources through ViewModel after auth dialog closes
                        glib::spawn_future_local({
                            let vm = vm.clone();
                            async move {
                                if let Err(e) = vm.load_sources().await {
                                    error!("Failed to refresh sources after auth: {}", e);
                                }
                            }
                        });
                    }
                });
            }
        }
    }

    fn confirm_remove_provider(&self, provider_id: &str, source_name: &str) {
        let dialog = adw::MessageDialog::builder()
            .title("Remove Account")
            .body(&format!("Are you sure you want to remove the account for '{}'? This will remove the account, all associated servers, and cached data.", source_name))
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
        info!("Removing auth provider: {}", provider_id);

        let state = self.imp().state.borrow().as_ref().unwrap().clone();
        let provider_id = provider_id.to_string();
        let page_weak = self.downgrade();

        glib::spawn_future_local(async move {
            // Remove the auth provider through the auth manager
            match state
                .source_coordinator
                .get_auth_manager()
                .remove_provider(&provider_id)
                .await
            {
                Ok(_) => {
                    info!("Auth provider {} removed successfully", provider_id);

                    // The ViewModel will automatically refresh UI through reactive bindings
                    // when it receives the UserLoggedOut event
                    if let Some(page) = page_weak.upgrade()
                        && let Some(vm) = &*page.imp().view_model.borrow()
                    {
                        // Trigger a refresh to update the UI immediately
                        info!(
                            "Triggering source refresh after provider {} removal",
                            provider_id
                        );
                        if let Err(e) = vm.load_sources().await {
                            error!("Failed to refresh sources after provider removal: {}", e);
                        } else {
                            info!("Source refresh completed after provider removal");
                        }
                    } else {
                        error!(
                            "Could not access page or ViewModel for UI refresh after provider removal"
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to remove auth provider {}: {}", provider_id, e);
                    if let Some(page) = page_weak.upgrade()
                        && let Some(window) = page.root().and_downcast::<gtk4::Window>()
                    {
                        let dialog = adw::AlertDialog::new(
                            Some("Failed to Remove Account"),
                            Some(&format!("Could not remove the account: {}", e)),
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
