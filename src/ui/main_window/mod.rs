mod navigation;
mod workers;

use adw::prelude::*;
use gtk::gio;
use libadwaita as adw;
use relm4::gtk;
use relm4::prelude::*;

use super::dialogs::{
    AuthDialog, AuthDialogInput, AuthDialogOutput, PreferencesDialog, PreferencesDialogInput,
    PreferencesDialogOutput,
};
use super::pages::{
    HomePage, LibraryPage, MovieDetailsPage, PlayerPage, SearchPage, ShowDetailsPage, SourcesPage,
};
use super::shared::broker::{BROKER, BrokerMessage, SourceMessage};
use super::sidebar::{Sidebar, SidebarInput, SidebarOutput};
use crate::db::connection::DatabaseConnection;
use crate::models::{LibraryId, MediaItemId, PlaylistContext, SourceId};
use crate::services::core::ConnectionType;
use crate::workers::{
    ConnectionMonitor, ConnectionMonitorInput, ConnectionMonitorOutput, SearchWorker,
    SearchWorkerInput, SearchWorkerOutput, SyncWorker, SyncWorkerInput, SyncWorkerOutput,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;

#[derive(Debug)]
pub struct MainWindow {
    db: DatabaseConnection,
    runtime: Arc<Runtime>,
    sidebar: Controller<Sidebar>,
    home_page: AsyncController<HomePage>,
    connection_monitor: relm4::WorkerController<ConnectionMonitor>,
    sync_worker: relm4::WorkerController<SyncWorker>,
    search_worker: relm4::WorkerController<SearchWorker>,
    config_manager: relm4::WorkerController<crate::workers::config_manager::ConfigManager>,
    cache_cleanup_worker:
        relm4::WorkerController<crate::workers::cache_cleanup_worker::CacheCleanupWorker>,
    library_page: Option<AsyncController<LibraryPage>>,
    movie_details_page: Option<AsyncController<MovieDetailsPage>>,
    show_details_page: Option<AsyncController<ShowDetailsPage>>,
    player_page: Option<AsyncController<PlayerPage>>,
    sources_page: Option<AsyncController<SourcesPage>>,
    sources_nav_page: Option<adw::NavigationPage>,
    search_page: Option<AsyncController<SearchPage>>,
    search_nav_page: Option<adw::NavigationPage>,
    preferences_dialog: Option<AsyncController<PreferencesDialog>>,
    auth_dialog: AsyncController<AuthDialog>,
    navigation_view: adw::NavigationView,
    // Window chrome management
    content_header: adw::HeaderBar,
    sidebar_header: adw::HeaderBar,
    content_toolbar: adw::ToolbarView,
    sidebar_toolbar: adw::ToolbarView,
    // Navigation state
    split_view: adw::NavigationSplitView,
    content_stack: gtk::Stack,
    back_button: gtk::Button,
    content_title: adw::WindowTitle,
    // Header bar dynamic content
    header_start_box: gtk::Box,
    header_end_box: gtk::Box,
    // Window state for restoration
    saved_window_size: Option<(i32, i32)>,
    was_maximized: bool,
    was_fullscreen: bool,
    // Current navigation state
    current_library_id: Option<LibraryId>,
    // Toast overlay for notifications
    toast_overlay: adw::ToastOverlay,
    // Connection type tracking for remote connection warnings
    connection_types: HashMap<SourceId, ConnectionType>,
}

#[derive(Debug)]
pub enum MainWindowInput {
    Navigate(String),
    NavigateToSource(SourceId),
    NavigateToLibrary(LibraryId),
    NavigateToMediaItem(MediaItemId),
    NavigateToMovie(MediaItemId),
    NavigateToShow(MediaItemId),
    NavigateToPlayer(MediaItemId),
    NavigateToPlayerWithContext {
        media_id: MediaItemId,
        context: PlaylistContext,
    },
    NavigateToPreferences,
    NavigateToSearch,
    SearchQuery(String),
    SearchResultsReceived {
        query: String,
        results: Vec<MediaItemId>,
        total_hits: usize,
    },
    ToggleSidebar,
    SyncSource(SourceId),
    RestoreWindowChrome,
    ResizeWindow(i32, i32),
    SetHeaderStartContent(Option<gtk::Widget>),
    SetHeaderEndContent(Option<gtk::Widget>),
    SetTitleWidget(Option<gtk::Widget>),
    ClearHeaderContent,
    ShowToast(String),
    ConnectionStatusChanged {
        source_id: SourceId,
        status: ConnectionStatus,
    },
    ConfigUpdated,
}

#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Connected {
        url: String,
        connection_type: ConnectionType,
    },
    Disconnected,
}

#[derive(Debug)]
pub enum MainWindowOutput {
    // No output messages currently defined
}

#[relm4::component(pub async)]
impl AsyncComponent for MainWindow {
    type Init = (DatabaseConnection, Arc<Runtime>);
    type Input = MainWindowInput;
    type Output = MainWindowOutput;
    type CommandOutput = Vec<crate::models::Source>;

    view! {
        #[root]
        adw::ApplicationWindow {
            set_title: Some("Reel"),
            set_default_width: 1200,
            set_default_height: 800,

            #[wrap(Some)]
            #[name(toast_overlay)]
            set_content = &adw::ToastOverlay {
                #[wrap(Some)]
                #[name(split_view)]
                set_child = &adw::NavigationSplitView {
                set_sidebar_width_fraction: 0.25,
                set_min_sidebar_width: 280.0,
                set_max_sidebar_width: 400.0,
                set_show_content: true,
                set_collapsed: false,

                #[wrap(Some)]
                set_sidebar = &adw::NavigationPage {
                    set_title: "Navigation",
                    set_can_pop: false,

                    #[wrap(Some)]
                    #[name(sidebar_toolbar)]
                    set_child = &adw::ToolbarView {
                        #[name(sidebar_header)]
                        add_top_bar = &adw::HeaderBar {
                            set_show_title: true,
                            set_title_widget: Some(&adw::WindowTitle::new("Reel", "")),

                            #[name(primary_menu_button)]
                            pack_end = &gtk::MenuButton {
                                set_icon_name: "open-menu-symbolic",
                                set_tooltip_text: Some("Main Menu"),
                                add_css_class: "flat",
                                set_primary: true,
                                set_direction: gtk::ArrowType::Down,
                            }
                        },

                        #[wrap(Some)]
                        #[name(sidebar_content)]
                        set_content = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                        },
                    },
                },

                #[wrap(Some)]
                set_content = &adw::NavigationPage {
                    set_title: "Content",
                    set_can_pop: false,

                    #[wrap(Some)]
                    #[name(content_toolbar)]
                    set_child = &adw::ToolbarView {
                        #[name(content_header)]
                        add_top_bar = &adw::HeaderBar {
                            set_show_title: true,

                            #[name(back_button)]
                            pack_start = &gtk::Button {
                                set_icon_name: "go-previous-symbolic",
                                set_tooltip_text: Some("Go Back"),
                                add_css_class: "flat",
                                set_visible: false,
                                connect_clicked => MainWindowInput::Navigate("back".to_string()),
                            },

                            pack_start = &gtk::Button {
                                set_icon_name: "sidebar-show-symbolic",
                                set_tooltip_text: Some("Toggle Sidebar"),
                                add_css_class: "flat",
                                connect_clicked => MainWindowInput::ToggleSidebar,
                            },

                            // Dynamic header start content (after sidebar button)
                            #[name(header_start_box)]
                            pack_start = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 6,
                            },

                            #[wrap(Some)]
                            #[name(content_title)]
                            set_title_widget = &adw::WindowTitle::new("Select a Library", ""),

                            // Dynamic header end content
                            #[name(header_end_box)]
                            pack_end = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 6,
                            },

                            pack_end = &gtk::SearchEntry {
                                set_placeholder_text: Some("Search media..."),
                                set_width_request: 250,
                                connect_activate[sender] => move |entry| {
                                    let query = entry.text().to_string();
                                    if !query.is_empty() {
                                        sender.input(MainWindowInput::NavigateToSearch);
                                        sender.input(MainWindowInput::SearchQuery(query));
                                    }
                                },
                            },
                        },

                        #[wrap(Some)]
                        #[name(content_stack)]
                        set_content = &gtk::Stack {
                            set_transition_type: gtk::StackTransitionType::Crossfade,
                            set_transition_duration: 200,

                            add_named[Some("empty")] = &adw::StatusPage {
                                set_icon_name: Some("folder-symbolic"),
                                set_title: "Select a Library",
                                set_description: Some("Choose a library from the sidebar to browse your media"),
                                set_vexpand: true,
                                set_hexpand: true,
                            },

                            #[name(navigation_view)]
                            add_named[Some("content")] = &adw::NavigationView {
                                set_animate_transitions: true,
                            },
                        },
                    },
                },
                },
            },
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let (db, runtime) = init;
        // Set up window actions - we'll add them to the window instead of the app
        // to ensure they're available when the menu is created

        // Preferences action
        let preferences_action = gio::SimpleAction::new("preferences", None);
        preferences_action.set_enabled(true);
        let sender_clone = sender.clone();
        preferences_action.connect_activate(move |_, _| {
            tracing::info!("Preferences action activated from menu");
            sender_clone.input(MainWindowInput::NavigateToPreferences);
        });
        root.add_action(&preferences_action);
        tracing::info!("Preferences action registered on window");

        // About action
        let about_action = gio::SimpleAction::new("about", None);
        about_action.set_enabled(true);
        let window_clone = root.clone();
        about_action.connect_activate(move |_, _| {
            let about_dialog = adw::AboutDialog::builder()
                .application_name("Reel")
                .application_icon("media-reel")
                .version(env!("CARGO_PKG_VERSION"))
                .comments("A native media player for Plex and Jellyfin")
                .website("https://github.com/arsfeld/reel")
                .issue_url("https://github.com/arsfeld/reel/issues")
                .license_type(gtk::License::Gpl30)
                .developers(vec!["Alex Rosenfeld"])
                .build();

            about_dialog.present(Some(&window_clone));
        });
        root.add_action(&about_action);

        // Quit action
        let quit_action = gio::SimpleAction::new("quit", None);
        quit_action.set_enabled(true);
        let root_clone = root.clone();
        quit_action.connect_activate(move |_, _| {
            if let Some(app) = root_clone.application() {
                app.quit();
            }
        });
        root.add_action(&quit_action);

        // Apply platform-specific styling
        crate::utils::platform::Platform::apply_platform_classes(&root);

        // Configure native window controls for macOS
        #[cfg(target_os = "macos")]
        {
            // TODO: Once we upgrade to libadwaita 1.8+, we should use native window controls
            // For now, we use custom CSS styling for the window control buttons

            if let Some(settings) = gtk::Settings::default() {
                // Set decoration layout for macOS button order (left side, no title)
                settings.set_property("gtk-decoration-layout", "close,minimize,maximize:");

                // Use smaller font for compact title bar
                settings.set_property("gtk-font-name", "SF Pro Text 11");
            }

            // Log platform detection for debugging
            tracing::info!(
                "Running on macOS - Applied platform-specific styles and window controls"
            );
        }

        // Set keyboard shortcuts at the application level if available
        if let Some(app) = root.application()
            && let Some(adw_app) = app.downcast_ref::<adw::Application>()
        {
            adw_app.set_accels_for_action("win.preferences", &["<primary>comma"]);
            adw_app.set_accels_for_action("win.quit", &["<primary>q"]);
            adw_app.set_accels_for_action("window.close", &["<primary>w"]);
        }

        // Initialize the sidebar
        let sidebar =
            Sidebar::builder()
                .launch(db.clone())
                .forward(sender.input_sender(), |output| match output {
                    SidebarOutput::NavigateToHome => MainWindowInput::Navigate("home".to_string()),
                    SidebarOutput::NavigateToLibrary(id) => MainWindowInput::NavigateToLibrary(id),
                    SidebarOutput::NavigateToSources => {
                        MainWindowInput::Navigate("sources".to_string())
                    }
                });

        // Initialize the home page
        let home_page =
            HomePage::builder()
                .launch(db.clone())
                .forward(sender.input_sender(), |output| match output {
                    crate::ui::pages::home::HomePageOutput::NavigateToMediaItem(id) => {
                        MainWindowInput::NavigateToMediaItem(id)
                    }
                });

        // Initialize the auth dialog with parent window
        let auth_dialog = AuthDialog::builder()
            .launch((db.clone(), Some(root.clone().upcast())))
            .forward(sender.input_sender(), |output| match output {
                AuthDialogOutput::SourceAdded(source_id) => {
                    tracing::info!("Source added: {:?}", source_id);
                    // Trigger a sync of the new source
                    MainWindowInput::SyncSource(source_id)
                }
                AuthDialogOutput::Cancelled => {
                    tracing::info!("Auth dialog cancelled");
                    MainWindowInput::Navigate("sources".to_string())
                }
            });

        // Initialize all background workers
        let workers_result = workers::initialize_workers(db.clone(), runtime.clone(), &sender);
        let workers::Workers {
            config_manager,
            connection_monitor,
            sync_worker,
            search_worker,
            cache_cleanup_worker,
        } = workers_result;

        let mut model = Self {
            db,
            runtime,
            sidebar,
            home_page,
            auth_dialog,
            connection_monitor,
            sync_worker,
            search_worker,
            config_manager,
            cache_cleanup_worker,
            library_page: None,
            movie_details_page: None,
            show_details_page: None,
            player_page: None,
            sources_page: None,
            sources_nav_page: None,
            search_page: None,
            search_nav_page: None,
            preferences_dialog: None,
            navigation_view: adw::NavigationView::new(),
            content_header: adw::HeaderBar::new(),
            sidebar_header: adw::HeaderBar::new(),
            content_toolbar: adw::ToolbarView::new(),
            sidebar_toolbar: adw::ToolbarView::new(),
            split_view: adw::NavigationSplitView::new(),
            content_stack: gtk::Stack::new(),
            back_button: gtk::Button::new(),
            content_title: adw::WindowTitle::new("", ""),
            header_start_box: gtk::Box::new(gtk::Orientation::Horizontal, 6),
            header_end_box: gtk::Box::new(gtk::Orientation::Horizontal, 6),
            saved_window_size: None,
            was_maximized: false,
            was_fullscreen: false,
            current_library_id: None,
            toast_overlay: adw::ToastOverlay::new(),
            connection_types: HashMap::new(),
        };

        let widgets = view_output!();

        // Set the sidebar widget in the sidebar toolbar
        widgets.sidebar_content.append(model.sidebar.widget());

        // Create primary menu
        let primary_menu = gio::Menu::new();

        // First section with preferences
        let section1 = gio::Menu::new();
        section1.append(Some("_Preferences"), Some("win.preferences"));
        primary_menu.append_section(None, &section1);

        // Second section with about
        let section2 = gio::Menu::new();
        section2.append(Some("_About Reel"), Some("win.about"));
        primary_menu.append_section(None, &section2);

        // Third section with quit
        let section3 = gio::Menu::new();
        section3.append(Some("_Quit"), Some("win.quit"));
        primary_menu.append_section(None, &section3);

        // Create a popover menu from the menu model
        let popover_menu = gtk::PopoverMenu::from_model(Some(&primary_menu));

        // Set the popover on the MenuButton instead of the menu model
        widgets.primary_menu_button.set_popover(Some(&popover_menu));

        // Verify actions are registered on the window
        let has_preferences = root.lookup_action("preferences").is_some();
        let has_about = root.lookup_action("about").is_some();
        let has_quit = root.lookup_action("quit").is_some();
        tracing::info!(
            "Window actions status - Preferences: {}, About: {}, Quit: {}",
            has_preferences,
            has_about,
            has_quit
        );

        // Also verify the actions are enabled
        if let Some(pref_action) = root.lookup_action("preferences")
            && let Some(simple_action) = pref_action.downcast_ref::<gio::SimpleAction>()
        {
            tracing::info!("Preferences action enabled: {}", simple_action.is_enabled());
        }

        tracing::info!("Primary menu configured with Preferences, About, and Quit actions");

        // Store references to widgets for later use
        model.toast_overlay.clone_from(&widgets.toast_overlay);
        model.navigation_view.clone_from(&widgets.navigation_view);
        model.content_header.clone_from(&widgets.content_header);
        model.sidebar_header.clone_from(&widgets.sidebar_header);
        model.content_toolbar.clone_from(&widgets.content_toolbar);
        model.sidebar_toolbar.clone_from(&widgets.sidebar_toolbar);
        model.split_view.clone_from(&widgets.split_view);
        model.content_stack.clone_from(&widgets.content_stack);
        model.back_button.clone_from(&widgets.back_button);
        model.content_title.clone_from(&widgets.content_title);
        model.header_start_box.clone_from(&widgets.header_start_box);
        model.header_end_box.clone_from(&widgets.header_end_box);

        // Start with empty state shown
        widgets.content_stack.set_visible_child_name("empty");

        // Connect navigation view signals
        {
            let sender_clone = sender.input_sender().clone();
            model.navigation_view.connect_pushed(move |_nav_view| {
                sender_clone
                    .send(MainWindowInput::Navigate("update_header".to_string()))
                    .unwrap();
            });
        }
        {
            let sender_clone = sender.input_sender().clone();
            model
                .navigation_view
                .connect_popped(move |_nav_view, _page| {
                    sender_clone
                        .send(MainWindowInput::Navigate("update_header".to_string()))
                        .unwrap();
                });
        }

        // Connect to visible-page changes to restore cursor when leaving player
        {
            use std::cell::RefCell;
            use std::rc::Rc;

            let root_clone = root.clone();
            let previous_page_title = Rc::new(RefCell::new(String::new()));

            model.navigation_view.connect_notify_local(
                Some("visible-page"),
                move |nav_view, _param| {
                    let prev_title = previous_page_title.borrow().clone();
                    let current_title = if let Some(visible_page) = nav_view.visible_page() {
                        visible_page.title().to_string()
                    } else {
                        "None".to_string()
                    };

                    // Check if we're transitioning away from the Player page
                    if prev_title == "Player" {
                        // Restore cursor on next event loop tick to ensure it happens after mouse leave events
                        let root_for_restore = root_clone.clone();
                        gtk::glib::idle_add_local_once(move || {
                            if let Some(surface) = root_for_restore.surface() {
                                if let Some(cursor) = gtk::gdk::Cursor::from_name("default", None) {
                                    surface.set_cursor(Some(&cursor));
                                } else {
                                    tracing::warn!("Failed to create default cursor");
                                }
                            } else {
                                tracing::warn!("No surface available for cursor restoration");
                            }
                        });
                    }

                    // Update the previous page title for next transition
                    *previous_page_title.borrow_mut() = current_title;
                },
            );
        }

        // Trigger initial sync of all sources after a short delay to let UI initialize
        sender.input(MainWindowInput::Navigate("init_sync".to_string()));

        // Initialize search index with existing media items
        sender.input(MainWindowInput::Navigate("init_search_index".to_string()));

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        match msg {
            MainWindowInput::Navigate(page) => {
                navigation::handle_navigate(self, page, &sender);
            }
            MainWindowInput::NavigateToPreferences => {
                navigation::navigate_to_preferences(self, &sender, root);
            }
            MainWindowInput::NavigateToSearch => {
                navigation::navigate_to_search(self, &sender);
            }
            MainWindowInput::SearchQuery(query) => {
                tracing::info!("Executing search query: {}", query);
                // Send query to SearchWorker
                self.search_worker
                    .emit(SearchWorkerInput::Search { query, limit: 50 });
            }
            MainWindowInput::SearchResultsReceived {
                query,
                results,
                total_hits,
            } => {
                tracing::info!(
                    "Received search results: {} results for '{}'",
                    results.len(),
                    query
                );

                // Navigate to search page if not already there
                if self.search_page.is_none() {
                    sender.input(MainWindowInput::NavigateToSearch);
                }

                // Send results to search page
                if let Some(ref search_page) = self.search_page {
                    use crate::ui::pages::search::SearchPageInput;
                    search_page
                        .sender()
                        .send(SearchPageInput::SetResults { query, results })
                        .ok();
                }

                // Show toast with result count
                self.toast_overlay.add_toast(
                    adw::Toast::builder()
                        .title(format!("Found {} results", total_hits))
                        .timeout(2)
                        .build(),
                );
            }
            MainWindowInput::NavigateToSource(source_id) => {
                navigation::navigate_to_source(self, source_id, &sender);
            }
            MainWindowInput::NavigateToLibrary(library_id) => {
                navigation::navigate_to_library(self, library_id, &sender);
            }
            MainWindowInput::NavigateToMediaItem(item_id) => {
                navigation::navigate_to_media_item(self, item_id, &sender);
            }
            MainWindowInput::NavigateToMovie(item_id) => {
                navigation::navigate_to_movie(self, item_id, &sender);
            }
            MainWindowInput::NavigateToShow(item_id) => {
                navigation::navigate_to_show(self, item_id, &sender);
            }
            MainWindowInput::NavigateToPlayer(media_id) => {
                navigation::navigate_to_player(self, media_id, &sender, root);
            }
            MainWindowInput::NavigateToPlayerWithContext { media_id, context } => {
                navigation::navigate_to_player_with_context(self, media_id, context, &sender, root);
            }
            MainWindowInput::ToggleSidebar => {
                tracing::info!("Toggling sidebar");

                // Toggle the collapsed state of the split view
                let is_collapsed = self.split_view.is_collapsed();
                self.split_view.set_collapsed(!is_collapsed);

                // If we're collapsing, ensure content is shown
                if !is_collapsed {
                    self.split_view.set_show_content(true);
                }
            }
            MainWindowInput::SyncSource(source_id) => {
                tracing::info!("Syncing new source: {:?}", source_id);

                // Trigger sync using the SyncWorker
                self.sync_worker
                    .sender()
                    .send(SyncWorkerInput::StartSync {
                        source_id: source_id.clone(),
                        library_id: None,
                        force: false,
                    })
                    .unwrap_or_else(|e| {
                        tracing::error!("Failed to send sync command to worker: {:?}", e);
                    });

                // Schedule UI refresh after sync completes
                let sender_clone = sender.clone();
                gtk::glib::timeout_add_local_once(std::time::Duration::from_secs(3), move || {
                    // Wait for sync to complete (approximate time)
                    sender_clone.input(MainWindowInput::Navigate("refresh_sidebar".to_string()));
                    sender_clone.input(MainWindowInput::Navigate(
                        "refresh_sources_page".to_string(),
                    ));
                });
            }
            MainWindowInput::RestoreWindowChrome => {
                tracing::info!("Restoring window chrome after player");

                // Stop the player before leaving the page
                if let Some(ref player_page) = self.player_page {
                    tracing::info!("Stopping player before navigation");
                    player_page
                        .sender()
                        .send(crate::ui::pages::player::PlayerInput::Stop)
                        .unwrap_or_else(|_| {
                            tracing::error!("Failed to stop player");
                        });
                }

                // Show window chrome again
                self.content_header.set_visible(true);
                self.sidebar_header.set_visible(true);
                self.split_view.set_collapsed(false);
                self.content_toolbar
                    .set_top_bar_style(adw::ToolbarStyle::Raised);
                self.sidebar_toolbar
                    .set_top_bar_style(adw::ToolbarStyle::Raised);

                // Restore cursor visibility when leaving player
                if let Some(surface) = root.surface()
                    && let Some(cursor) = gtk::gdk::Cursor::from_name("default", None)
                {
                    surface.set_cursor(Some(&cursor));
                }

                // Restore window size and state
                if let Some((width, height)) = self.saved_window_size {
                    root.set_default_size(width, height);
                }

                if self.was_maximized {
                    root.maximize();
                } else if self.was_fullscreen {
                    root.fullscreen();
                } else {
                    root.unmaximize();
                    root.unfullscreen();
                }

                // Pop the player page from navigation
                self.navigation_view.pop();
            }
            MainWindowInput::ResizeWindow(width, height) => {
                tracing::info!(
                    "Resizing window to {}x{} for video aspect ratio",
                    width,
                    height
                );
                root.set_default_size(width, height);

                // Center the window on screen after resize
                // Note: GTK4 doesn't have a direct center method, but setting default size
                // and letting the window manager handle it usually works well
            }
            MainWindowInput::SetHeaderStartContent(widget) => {
                // Clear existing content
                while let Some(child) = self.header_start_box.first_child() {
                    self.header_start_box.remove(&child);
                }
                // Add new content if provided
                if let Some(widget) = widget {
                    self.header_start_box.append(&widget);
                }
            }
            MainWindowInput::SetHeaderEndContent(widget) => {
                tracing::info!("Setting header end content");
                // Clear existing content
                while let Some(child) = self.header_end_box.first_child() {
                    self.header_end_box.remove(&child);
                }
                // Add new content if provided
                if let Some(widget) = widget {
                    tracing::info!("Adding widget to header end box");
                    self.header_end_box.append(&widget);
                    self.header_end_box.set_visible(true);
                } else {
                    tracing::info!("No widget provided to add");
                }
            }
            MainWindowInput::SetTitleWidget(widget) => {
                if let Some(widget) = widget {
                    self.content_header.set_title_widget(Some(&widget));
                } else {
                    // Reset to default title widget
                    self.content_header
                        .set_title_widget(Some(&self.content_title));
                }
            }
            MainWindowInput::ClearHeaderContent => {
                // Clear both header boxes
                while let Some(child) = self.header_start_box.first_child() {
                    self.header_start_box.remove(&child);
                }
                while let Some(child) = self.header_end_box.first_child() {
                    self.header_end_box.remove(&child);
                }
                // Reset title widget to default
                self.content_header
                    .set_title_widget(Some(&self.content_title));
            }
            MainWindowInput::ShowToast(message) => {
                let toast = adw::Toast::new(&message);
                toast.set_timeout(3);
                self.toast_overlay.add_toast(toast);
            }
            MainWindowInput::ConfigUpdated => {
                // Handle configuration updates from file watcher
                tracing::info!("Configuration has been updated from disk");
            }
            MainWindowInput::ConnectionStatusChanged { source_id, status } => {
                // Handle connection status changes from ConnectionMonitor
                let (is_connected, status_text) = match &status {
                    ConnectionStatus::Connected {
                        url,
                        connection_type,
                    } => {
                        // Check if this is a transition from local to remote/relay
                        let previous_type = self.connection_types.get(&source_id);

                        tracing::info!(
                            "Connection type for {}: previous={:?}, current={:?}",
                            source_id,
                            previous_type,
                            connection_type
                        );

                        // Show warning toast when falling back to remote/relay
                        let is_remote_transition = match (previous_type, connection_type) {
                            // If we previously had a local connection and now have remote/relay
                            (Some(ConnectionType::Local), ConnectionType::Remote)
                            | (Some(ConnectionType::Local), ConnectionType::Relay)
                            // Or if this is the first connection and it's remote/relay
                            | (None, ConnectionType::Remote)
                            | (None, ConnectionType::Relay) => true,
                            _ => false,
                        };

                        if is_remote_transition {
                            let message = match connection_type {
                                ConnectionType::Relay => {
                                    "Using relay connection - Direct connection unavailable"
                                }
                                ConnectionType::Remote => {
                                    "Using remote connection - Local connection unavailable"
                                }
                                ConnectionType::Local => "", // Won't happen due to match guard
                            };

                            tracing::warn!(
                                "⚠️  Source {} using {:?} connection (was {:?})",
                                source_id,
                                connection_type,
                                previous_type
                            );

                            sender.input(MainWindowInput::ShowToast(message.to_string()));
                        }

                        // Update stored connection type
                        self.connection_types
                            .insert(source_id.clone(), *connection_type);

                        tracing::info!(
                            "Source {} connected at {} ({:?})",
                            source_id,
                            url,
                            connection_type
                        );

                        let conn_label = match connection_type {
                            ConnectionType::Local => "local",
                            ConnectionType::Remote => "remote",
                            ConnectionType::Relay => "relay",
                        };
                        (
                            true,
                            format!("Source {} connected ({})", source_id, conn_label),
                        )
                    }
                    ConnectionStatus::Disconnected => {
                        tracing::warn!("Source {} disconnected", source_id);
                        sender.input(MainWindowInput::ShowToast(format!(
                            "Connection lost for source {}",
                            source_id
                        )));
                        // Clear stored connection type on disconnect
                        self.connection_types.remove(&source_id);
                        (false, format!("Source {} disconnected", source_id))
                    }
                };

                // Update sidebar with overall connection status text
                self.sidebar
                    .sender()
                    .send(SidebarInput::UpdateConnectionStatus(status_text))
                    .unwrap_or_else(|e| {
                        tracing::error!("Failed to update sidebar connection status: {:?}", e);
                    });

                // Update specific source with connection type info
                let conn_state = if is_connected {
                    super::sidebar::ConnectionState::Connected
                } else {
                    super::sidebar::ConnectionState::Disconnected
                };

                let conn_type = match &status {
                    ConnectionStatus::Connected {
                        connection_type, ..
                    } => Some(*connection_type),
                    ConnectionStatus::Disconnected => None,
                };

                tracing::info!(
                    "Sending connection type {:?} to sidebar for source {}",
                    conn_type,
                    source_id
                );

                self.sidebar
                    .sender()
                    .send(SidebarInput::UpdateSourceConnectionStatus {
                        source_id: source_id.clone(),
                        state: conn_state,
                        error: None,
                        connection_type: conn_type,
                    })
                    .unwrap_or_else(|e| {
                        tracing::error!("Failed to update source connection status: {:?}", e);
                    });

                // Also use ConnectionCheckResult which won't override SyncFailed state
                self.sidebar
                    .sender()
                    .send(SidebarInput::ConnectionCheckResult {
                        source_id: source_id.clone(),
                        is_connected,
                    })
                    .unwrap_or_else(|e| {
                        tracing::error!("Failed to update source connection indicator: {:?}", e);
                    });

                // If we have a sources page open, update it too
                if let Some(ref sources_page) = self.sources_page {
                    // Use the is_connected value we already computed above
                    sources_page
                        .sender()
                        .send(
                            crate::ui::pages::sources::SourcesPageInput::UpdateConnectionStatus {
                                source_id: source_id.clone(),
                                is_connected,
                            },
                        )
                        .unwrap_or_else(|e| {
                            tracing::error!(
                                "Failed to update sources page connection status: {:?}",
                                e
                            );
                        });
                }

                // Trigger connection monitor to check this specific source again in case of disconnect
                if matches!(status, ConnectionStatus::Disconnected) {
                    // Wait a bit before retrying
                    let monitor_sender = self.connection_monitor.sender().clone();
                    let source_id_clone = source_id.clone();
                    relm4::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        monitor_sender
                            .send(ConnectionMonitorInput::CheckSource(source_id_clone))
                            .unwrap_or_else(|e| {
                                tracing::error!("Failed to trigger connection check: {:?}", e);
                            });
                    });
                }
            }
        }
    }

    async fn update_cmd(
        &mut self,
        sources: Self::CommandOutput,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        // Start sync for all loaded sources using SyncWorker
        for source in sources {
            let source_id = SourceId::new(source.id.clone());
            tracing::info!("Starting startup sync for source: {}", source.name);

            // Trigger sync using the SyncWorker
            self.sync_worker
                .sender()
                .send(SyncWorkerInput::StartSync {
                    source_id,
                    library_id: None,
                    force: false,
                })
                .unwrap_or_else(|e| {
                    tracing::error!("Failed to send sync command to worker: {:?}", e);
                });
        }
    }
}
