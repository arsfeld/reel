use adw::prelude::*;
use gtk::gio;
use gtk::prelude::*;
use libadwaita as adw;
use relm4::gtk;
use relm4::prelude::*;

use super::dialogs::{AuthDialog, AuthDialogInput, AuthDialogOutput};
use super::pages::{
    HomePage, LibraryPage, MovieDetailsPage, PlayerPage, PreferencesPage, ShowDetailsPage,
    SourcesPage,
};
use super::sidebar::{Sidebar, SidebarInput, SidebarOutput};
use crate::db::connection::DatabaseConnection;
use crate::models::{LibraryId, MediaItemId, PlaylistContext, SourceId};

#[derive(Debug)]
pub struct MainWindow {
    db: DatabaseConnection,
    sidebar: Controller<Sidebar>,
    home_page: AsyncController<HomePage>,
    library_page: Option<AsyncController<LibraryPage>>,
    movie_details_page: Option<AsyncController<MovieDetailsPage>>,
    show_details_page: Option<AsyncController<ShowDetailsPage>>,
    player_page: Option<AsyncController<PlayerPage>>,
    sources_page: Option<AsyncController<SourcesPage>>,
    sources_nav_page: Option<adw::NavigationPage>,
    preferences_page: Option<AsyncController<PreferencesPage>>,
    preferences_nav_page: Option<adw::NavigationPage>,
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
    ToggleSidebar,
    SyncSource(SourceId),
    RestoreWindowChrome,
    ResizeWindow(i32, i32),
    SetHeaderStartContent(Option<gtk::Widget>),
    SetHeaderEndContent(Option<gtk::Widget>),
    ClearHeaderContent,
    ShowToast(String),
}

#[derive(Debug)]
pub enum MainWindowOutput {
    Quit,
}

#[relm4::component(pub async)]
impl AsyncComponent for MainWindow {
    type Init = DatabaseConnection;
    type Input = MainWindowInput;
    type Output = MainWindowOutput;
    type CommandOutput = (bool, usize, Option<String>);

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

                            pack_end = &gtk::Button {
                                set_icon_name: "system-search-symbolic",
                                set_tooltip_text: Some("Search Media"),
                                add_css_class: "flat",
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
        db: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
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
                .website("https://github.com/arosenfeld/gnome-reel")
                .issue_url("https://github.com/arosenfeld/gnome-reel/issues")
                .license_type(gtk::License::Gpl30)
                .developers(vec!["Aaron Rosenfeld"])
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

        // Set keyboard shortcuts at the application level if available
        if let Some(app) = root.application() {
            if let Some(adw_app) = app.downcast_ref::<adw::Application>() {
                adw_app.set_accels_for_action("win.preferences", &["<primary>comma"]);
                adw_app.set_accels_for_action("win.quit", &["<primary>q"]);
                adw_app.set_accels_for_action("window.close", &["<primary>w"]);
            }
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
        let home_page = HomePage::builder()
            .launch(db.clone())
            .forward(sender.input_sender(), |output| match output {
                crate::platforms::relm4::components::pages::home::HomePageOutput::NavigateToMediaItem(id) => {
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

        let mut model = Self {
            db,
            sidebar,
            home_page,
            auth_dialog,
            library_page: None,
            movie_details_page: None,
            show_details_page: None,
            player_page: None,
            sources_page: None,
            sources_nav_page: None,
            preferences_page: None,
            preferences_nav_page: None,
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
        if let Some(pref_action) = root.lookup_action("preferences") {
            if let Some(simple_action) = pref_action.downcast_ref::<gio::SimpleAction>() {
                tracing::info!("Preferences action enabled: {}", simple_action.is_enabled());
            }
        }

        tracing::info!("Primary menu configured with Preferences, About, and Quit actions");

        // Store references to widgets for later use
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

        // Trigger initial sync of all sources after a short delay to let UI initialize
        sender.input(MainWindowInput::Navigate("init_sync".to_string()));

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
                tracing::info!("Navigating to: {}", page);
                match page.as_str() {
                    "back" => {
                        // Check if we have pages to pop (more than 1 page in stack)
                        if self.navigation_view.navigation_stack().n_items() > 1 {
                            self.navigation_view.pop();

                            // Clear any custom header content when going back
                            sender.input(MainWindowInput::ClearHeaderContent);

                            // Check if we're back on Sources page and restore its header button
                            if let Some(page) = self.navigation_view.visible_page() {
                                if page.title() == "Sources" {
                                    // Re-add the "Add Source" button to the header
                                    let add_button = gtk::Button::builder()
                                        .icon_name("list-add-symbolic")
                                        .tooltip_text("Add Source")
                                        .css_classes(vec!["suggested-action"])
                                        .build();

                                    let sender_clone = sender.input_sender().clone();
                                    add_button.connect_clicked(move |_| {
                                        sender_clone.emit(MainWindowInput::Navigate(
                                            "auth_dialog".to_string(),
                                        ));
                                    });

                                    sender.input(MainWindowInput::SetHeaderEndContent(Some(
                                        add_button.upcast(),
                                    )));
                                }
                            }
                        }
                    }
                    "init_sync" => {
                        // Trigger sync for all existing sources on startup
                        let db_clone = self.db.clone();
                        sender.oneshot_command(async move {
                            // Wait a moment for the UI to fully initialize
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                            use crate::models::SourceId;
                            use crate::services::commands::{
                                Command, auth_commands::LoadSourcesCommand,
                            };
                            use crate::services::core::backend::BackendService;

                            let cmd = LoadSourcesCommand {
                                db: db_clone.clone(),
                            };
                            match cmd.execute().await {
                                Ok(sources) => {
                                    tracing::info!(
                                        "Found {} sources to sync on startup",
                                        sources.len()
                                    );
                                    let mut any_synced = false;
                                    for source in sources {
                                        let source_id = SourceId::new(source.id.clone());
                                        tracing::info!(
                                            "Starting startup sync for source: {}",
                                            source.name
                                        );

                                        // Sync the source in the background
                                        match BackendService::sync_source(&db_clone, &source_id)
                                            .await
                                        {
                                            Ok(sync_result) => {
                                                tracing::info!(
                                                    "Source {} sync completed: {} items synced",
                                                    source.name,
                                                    sync_result.items_synced
                                                );
                                                any_synced = true;
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    "Failed to sync source {}: {}",
                                                    source.name,
                                                    e
                                                );
                                            }
                                        }
                                    }

                                    // After all syncs complete, refresh the sidebar
                                    if any_synced {
                                        // We'll handle the refresh after this command completes
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to load sources for initial sync: {}",
                                        e
                                    );
                                }
                            }
                            // Return a dummy value since this is just startup sync
                            (true, 0, None)
                        });

                        // Schedule a delayed sidebar refresh since sources might sync
                        let sender_clone = sender.clone();
                        relm4::spawn_local(async move {
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                            sender_clone
                                .input(MainWindowInput::Navigate("refresh_sidebar".to_string()));
                        });
                    }
                    "update_header" => {
                        // Update back button visibility based on navigation stack
                        let can_pop = self.navigation_view.navigation_stack().n_items() > 1;
                        self.back_button.set_visible(can_pop);

                        // Update title and subtitle based on current page
                        if let Some(page) = self.navigation_view.visible_page() {
                            let title = page.title();
                            let subtitle = match title.as_str() {
                                "Home" => "",
                                "Sources" => "Manage your media sources",
                                "Preferences" => "Configure application settings",
                                "Library" => "Browse your media collection",
                                "Movie Details" => "Movie information",
                                "Show Details" => "TV show information",
                                "Player" => "", // Hide title in player
                                _ => "",
                            };
                            self.content_title.set_title(&title);
                            self.content_title.set_subtitle(subtitle);
                        }
                    }
                    "home" => {
                        // Switch to content view and show home page
                        self.content_stack.set_visible_child_name("content");

                        // Clear library tracking since we're on home
                        self.current_library_id = None;

                        // Check if we need to navigate to home or if we're already there
                        let stack = self.navigation_view.navigation_stack();

                        // If there's only 1 page in stack, check if it's the home page
                        if stack.n_items() == 1 {
                            if let Some(page) = stack.item(0) {
                                if let Ok(nav_page) = page.downcast::<adw::NavigationPage>() {
                                    if nav_page.title() == "Home" {
                                        // Already on home page with clean stack, just trigger header update
                                        sender.input(MainWindowInput::ClearHeaderContent);
                                        sender.input(MainWindowInput::Navigate(
                                            "update_header".to_string(),
                                        ));
                                        return;
                                    }
                                }
                            }
                        }

                        // Pop all pages except the root page
                        while stack.n_items() > 1 {
                            self.navigation_view.pop();
                        }

                        // Check if the remaining page is home, if not, replace it
                        let needs_home_page = if stack.n_items() == 1 {
                            if let Some(page) = stack.item(0) {
                                if let Ok(nav_page) = page.downcast::<adw::NavigationPage>() {
                                    nav_page.title() != "Home"
                                } else {
                                    true
                                }
                            } else {
                                true
                            }
                        } else {
                            // No pages in stack, need to add home
                            true
                        };

                        if needs_home_page {
                            // If we have a non-home page as the root, pop it too
                            if stack.n_items() > 0 {
                                self.navigation_view.pop();
                            }

                            // Create and push home page
                            let home_nav_page = adw::NavigationPage::builder()
                                .title("Home")
                                .child(self.home_page.widget())
                                .build();
                            self.navigation_view.push(&home_nav_page);
                        }

                        // Clear any custom header content
                        sender.input(MainWindowInput::ClearHeaderContent);

                        // Trigger header update
                        sender.input(MainWindowInput::Navigate("update_header".to_string()));
                    }
                    "sources" => {
                        // Switch to content view
                        self.content_stack.set_visible_child_name("content");

                        // Clear library tracking since we're on sources
                        self.current_library_id = None;

                        // Check if Sources page exists in the navigation stack
                        let stack = self.navigation_view.navigation_stack();
                        let mut sources_page_index = None;

                        for i in 0..stack.n_items() {
                            if let Some(page) = stack.item(i) {
                                if let Ok(nav_page) = page.downcast::<adw::NavigationPage>() {
                                    if nav_page.title() == "Sources" {
                                        sources_page_index = Some(i);
                                        break;
                                    }
                                }
                            }
                        }

                        // If Sources page exists in stack, pop to it instead of pushing new one
                        if let Some(index) = sources_page_index {
                            // Pop back to the Sources page
                            while self.navigation_view.navigation_stack().n_items() > index + 1 {
                                self.navigation_view.pop();
                            }

                            // If Sources page is not visible, make it visible
                            if let Some(visible_page) = self.navigation_view.visible_page() {
                                if visible_page.title() != "Sources" {
                                    // Navigate to the Sources page that's already in the stack
                                    if let Some(page) = stack.item(index) {
                                        if let Ok(nav_page) = page.downcast::<adw::NavigationPage>()
                                        {
                                            // The page should now be visible after popping
                                        }
                                    }
                                }
                            }
                        } else {
                            // Sources page doesn't exist in stack, create and push it

                            // Create sources page if it doesn't exist
                            if self.sources_page.is_none() {
                                let sources_controller = SourcesPage::builder()
                                    .launch(self.db.clone())
                                    .forward(sender.input_sender(), |output| match output {
                                        crate::platforms::relm4::components::pages::sources::SourcesPageOutput::OpenAuthDialog => {
                                            tracing::info!("Opening auth dialog for adding source");
                                            MainWindowInput::Navigate("auth_dialog".to_string())
                                        }
                                    });

                                // Create the navigation page once
                                let page = adw::NavigationPage::builder()
                                    .title("Sources")
                                    .child(sources_controller.widget())
                                    .build();

                                self.sources_nav_page = Some(page);
                                self.sources_page = Some(sources_controller);
                            }

                            // Push the existing navigation page
                            if let Some(ref page) = self.sources_nav_page {
                                self.navigation_view.push(page);
                            }
                        }

                        // Always set up the header button regardless of how we navigated to Sources
                        // Add the "Add Source" button to the header
                        tracing::info!("Adding Add Source button to header");
                        let add_button = gtk::Button::builder()
                            .icon_name("list-add-symbolic")
                            .tooltip_text("Add Source")
                            .css_classes(vec!["suggested-action"])
                            .build();

                        let sender_clone = sender.input_sender().clone();
                        add_button.connect_clicked(move |_| {
                            tracing::info!("Add Source button clicked");
                            sender_clone.emit(MainWindowInput::Navigate("auth_dialog".to_string()));
                        });

                        add_button.set_visible(true);
                        sender.input(MainWindowInput::SetHeaderEndContent(Some(
                            add_button.upcast(),
                        )));

                        // Trigger header update
                        sender.input(MainWindowInput::Navigate("update_header".to_string()));
                    }
                    "preferences" => {
                        // Switch to content view
                        self.content_stack.set_visible_child_name("content");

                        // Check if we're already on the preferences page
                        if let Some(visible_page) = self.navigation_view.visible_page() {
                            if visible_page.title() == "Preferences" {
                                // Already on preferences page, don't push again
                                return;
                            }
                        }

                        // Create preferences page if it doesn't exist
                        if self.preferences_page.is_none() {
                            let preferences_controller = PreferencesPage::builder()
                                .launch(self.db.clone())
                                .forward(sender.input_sender(), |output| match output {
                                    crate::platforms::relm4::components::pages::preferences::PreferencesOutput::PreferencesSaved => {
                                        tracing::info!("Preferences saved");
                                        MainWindowInput::Navigate("preferences_saved".to_string())
                                    }
                                    crate::platforms::relm4::components::pages::preferences::PreferencesOutput::Error(msg) => {
                                        tracing::error!("Preferences error: {}", msg);
                                        MainWindowInput::Navigate("preferences_error".to_string())
                                    }
                                });

                            // Create the navigation page once
                            let page = adw::NavigationPage::builder()
                                .title("Preferences")
                                .child(preferences_controller.widget())
                                .build();

                            self.preferences_nav_page = Some(page);
                            self.preferences_page = Some(preferences_controller);
                        }

                        // Push the existing navigation page
                        if let Some(ref page) = self.preferences_nav_page {
                            self.navigation_view.push(page);

                            // Trigger header update
                            sender.input(MainWindowInput::Navigate("update_header".to_string()));
                        }
                    }
                    "auth_dialog" => {
                        tracing::info!("Opening authentication dialog");
                        // Send show message to the auth dialog
                        self.auth_dialog.emit(AuthDialogInput::Show);
                    }
                    "refresh_sidebar" => {
                        tracing::info!("Refreshing sidebar after sync");
                        // Trigger sidebar refresh
                        self.sidebar.emit(SidebarInput::RefreshSources);
                    }
                    "refresh_sources_page" => {
                        // Refresh sources page if it exists
                        if let Some(ref sources_page) = self.sources_page {
                            tracing::info!("Refreshing sources page after sync");
                            sources_page.emit(crate::platforms::relm4::components::pages::sources::SourcesPageInput::LoadData);
                        }
                    }
                    _ => {}
                }
            }
            MainWindowInput::NavigateToPreferences => {
                tracing::info!("Opening preferences dialog");

                // Create a preferences dialog window
                let dialog = adw::Dialog::builder()
                    .title("Preferences")
                    .content_width(600)
                    .content_height(500)
                    .build();

                // Create preferences page if it doesn't exist
                if self.preferences_page.is_none() {
                    let preferences_controller = PreferencesPage::builder()
                        .launch(self.db.clone())
                        .forward(sender.input_sender(), |output| match output {
                            crate::platforms::relm4::components::pages::preferences::PreferencesOutput::PreferencesSaved => {
                                tracing::info!("Preferences saved");
                                MainWindowInput::Navigate("preferences_saved".to_string())
                            }
                            crate::platforms::relm4::components::pages::preferences::PreferencesOutput::Error(msg) => {
                                tracing::error!("Preferences error: {}", msg);
                                MainWindowInput::Navigate("preferences_error".to_string())
                            }
                        });

                    self.preferences_page = Some(preferences_controller);
                }

                // Set the preferences page as the dialog's child
                if let Some(ref preferences_controller) = self.preferences_page {
                    dialog.set_child(Some(preferences_controller.widget()));
                }

                // Present the dialog
                dialog.present(Some(root));
            }
            MainWindowInput::NavigateToSource(source_id) => {
                tracing::info!("Navigating to source: {}", source_id);

                // Switch to content view
                self.content_stack.set_visible_child_name("content");

                // TODO: Create and push source page
                // For now, just create a placeholder page
                let page = adw::NavigationPage::builder()
                    .title(&format!("Source: {}", source_id))
                    .child(&gtk::Label::new(Some(&format!(
                        "Source {} content",
                        source_id
                    ))))
                    .build();
                self.navigation_view.push(&page);
            }
            MainWindowInput::NavigateToLibrary(library_id) => {
                tracing::info!("Navigating to library: {}", library_id);

                // Check if we're already on this library page
                if let Some(ref current_id) = self.current_library_id {
                    if current_id == &library_id {
                        tracing::debug!("Already on library: {}, skipping navigation", library_id);
                        return;
                    }
                }

                // Check if we're already on a library page in the navigation stack
                // and if so, pop back to before it
                if self.current_library_id.is_some() {
                    // We're on a library page, check if it's the visible page
                    if let Some(visible_page) = self.navigation_view.visible_page() {
                        let title = visible_page.title();
                        // If the current page is a library page (not Home, Sources, etc.)
                        if !title.is_empty()
                            && title != "Home"
                            && title != "Sources"
                            && title != "Preferences"
                            && title != "Movie Details"
                            && title != "Show Details"
                            && title != "Player"
                        {
                            // Pop the current library page
                            self.navigation_view.pop();
                        }
                    }
                }

                // Switch to content view
                self.content_stack.set_visible_child_name("content");

                // Always recreate the library page for each navigation to avoid widget parent conflicts
                // This ensures the widget isn't already attached to another navigation page
                let library_controller = LibraryPage::builder()
                    .launch(self.db.clone())
                    .forward(sender.input_sender(), |output| match output {
                        crate::platforms::relm4::components::pages::library::LibraryPageOutput::NavigateToMediaItem(id) => {
                            MainWindowInput::NavigateToMediaItem(id)
                        }
                    });

                // Set the library on the new controller
                library_controller.emit(crate::platforms::relm4::components::pages::library::LibraryPageInput::SetLibrary(library_id.clone()));

                // Fetch library details for the title
                let library_title = {
                    use crate::services::commands::{Command, media_commands::GetLibraryCommand};
                    let cmd = GetLibraryCommand {
                        db: self.db.clone(),
                        library_id: library_id.clone(),
                    };
                    match cmd.execute().await {
                        Ok(Some(library)) => library.title,
                        _ => "Library".to_string(),
                    }
                };

                // Create navigation page with the new controller's widget
                let page = adw::NavigationPage::builder()
                    .title(&library_title)
                    .child(library_controller.widget())
                    .build();

                // Store the controller for later use
                self.library_page = Some(library_controller);

                // Update current library ID
                self.current_library_id = Some(library_id);

                // Push the page to navigation
                self.navigation_view.push(&page);

                // Clear any custom header content (library page can add its own filters later)
                sender.input(MainWindowInput::ClearHeaderContent);

                // Trigger header update
                sender.input(MainWindowInput::Navigate("update_header".to_string()));
            }
            MainWindowInput::NavigateToMediaItem(item_id) => {
                tracing::info!("Navigating to media item: {}", item_id);

                // Clear library tracking since we're navigating to a media item
                self.current_library_id = None;

                // Switch to content view
                self.content_stack.set_visible_child_name("content");

                // First, we need to determine what type of media this is
                let db_clone = self.db.clone();
                let item_id_clone = item_id.clone();
                let sender_clone = sender.clone();

                relm4::spawn_local(async move {
                    use crate::db::repository::{MediaRepository, MediaRepositoryImpl, Repository};

                    let repo = MediaRepositoryImpl::new(db_clone);
                    if let Ok(Some(media)) = repo.find_by_id(item_id_clone.as_ref()).await {
                        match media.media_type.as_str() {
                            "movie" => {
                                sender_clone.input(MainWindowInput::NavigateToMovie(item_id_clone));
                            }
                            "show" => {
                                sender_clone.input(MainWindowInput::NavigateToShow(item_id_clone));
                            }
                            "episode" => {
                                // For episodes, navigate directly to player with context
                                sender_clone
                                    .input(MainWindowInput::NavigateToPlayer(item_id_clone));
                            }
                            _ => {
                                tracing::warn!("Unknown media type: {}", media.media_type);
                            }
                        }
                    } else {
                        tracing::error!("Failed to find media item: {}", item_id_clone);
                    }
                });
            }
            MainWindowInput::NavigateToMovie(item_id) => {
                tracing::info!("Navigating to movie: {}", item_id);

                // Always recreate the movie details page to avoid widget parent conflicts
                // This ensures the widget isn't already attached to another navigation page
                let db = std::sync::Arc::new(self.db.clone());
                let movie_controller = MovieDetailsPage::builder()
                    .launch((item_id.clone(), db))
                    .forward(sender.input_sender(), |output| match output {
                        crate::platforms::relm4::components::pages::movie_details::MovieDetailsOutput::PlayMedia(id) => {
                            tracing::info!("Playing media: {}", id);
                            MainWindowInput::NavigateToPlayer(id)
                        }
                        crate::platforms::relm4::components::pages::movie_details::MovieDetailsOutput::NavigateBack => {
                            MainWindowInput::Navigate("back".to_string())
                        }
                    });

                // Create navigation page with the new controller's widget
                let page = adw::NavigationPage::builder()
                    .title("Movie Details")
                    .child(movie_controller.widget())
                    .build();

                // Store the controller for later use
                self.movie_details_page = Some(movie_controller);

                // Push the page to navigation
                self.navigation_view.push(&page);
            }
            MainWindowInput::NavigateToShow(item_id) => {
                tracing::info!("Navigating to show: {}", item_id);

                // Always recreate the show details page to avoid widget parent conflicts
                // This ensures the widget isn't already attached to another navigation page
                let db = std::sync::Arc::new(self.db.clone());
                let show_controller = ShowDetailsPage::builder()
                    .launch((item_id.clone(), db))
                    .forward(sender.input_sender(), move |output| match output {
                        crate::platforms::relm4::components::pages::show_details::ShowDetailsOutput::PlayMedia(id) => {
                            tracing::info!("Playing episode: {}", id);
                            MainWindowInput::NavigateToPlayer(id)
                        }
                        crate::platforms::relm4::components::pages::show_details::ShowDetailsOutput::PlayMediaWithContext { media_id, context } => {
                            tracing::info!("Playing episode with context: {}", media_id);
                            MainWindowInput::NavigateToPlayerWithContext { media_id, context }
                        }
                        crate::platforms::relm4::components::pages::show_details::ShowDetailsOutput::NavigateBack => {
                            MainWindowInput::Navigate("back".to_string())
                        }
                    });

                // Create navigation page with the new controller's widget
                let page = adw::NavigationPage::builder()
                    .title("Show Details")
                    .child(show_controller.widget())
                    .build();

                // Store the controller for later use
                self.show_details_page = Some(show_controller);

                // Push the page to navigation
                self.navigation_view.push(&page);
            }
            MainWindowInput::NavigateToPlayer(media_id) => {
                tracing::info!("Navigating to player for media: {}", media_id);

                // Save current window state before entering player
                let (width, height) = root.default_size();
                self.saved_window_size = Some((width, height));
                self.was_maximized = root.is_maximized();
                self.was_fullscreen = root.is_fullscreen();

                // Hide window chrome for immersive viewing
                self.content_header.set_visible(false);
                self.sidebar_header.set_visible(false);
                self.split_view.set_collapsed(true);
                self.content_toolbar
                    .set_top_bar_style(adw::ToolbarStyle::Flat);
                self.sidebar_toolbar
                    .set_top_bar_style(adw::ToolbarStyle::Flat);

                // Create player page if not exists
                if self.player_page.is_none() {
                    let db = std::sync::Arc::new(self.db.clone());
                    let sender_clone = sender.clone();
                    self.player_page = Some(
                        PlayerPage::builder()
                            .launch((Some(media_id.clone()), db, root.clone()))
                            .forward(sender.input_sender(), move |output| match output {
                                crate::platforms::relm4::components::pages::player::PlayerOutput::NavigateBack => {
                                    // Restore window chrome when leaving player
                                    sender_clone.input(MainWindowInput::RestoreWindowChrome);
                                    MainWindowInput::Navigate("back".to_string())
                                }
                                crate::platforms::relm4::components::pages::player::PlayerOutput::MediaLoaded => {
                                    tracing::info!("Media loaded in player");
                                    MainWindowInput::Navigate("media_loaded".to_string())
                                }
                                crate::platforms::relm4::components::pages::player::PlayerOutput::Error(msg) => {
                                    tracing::error!("Player error: {}", msg);
                                    // Restore window chrome on error
                                    sender_clone.input(MainWindowInput::RestoreWindowChrome);
                                    MainWindowInput::Navigate("player_error".to_string())
                                }
                                crate::platforms::relm4::components::pages::player::PlayerOutput::WindowStateChanged { width, height } => {
                                    // Player is requesting window size change for aspect ratio
                                    MainWindowInput::ResizeWindow(width, height)
                                }
                            }),
                    );
                } else if let Some(ref player_page) = self.player_page {
                    // Update existing page with new media
                    player_page.sender().send(crate::platforms::relm4::components::pages::player::PlayerInput::LoadMedia(media_id.clone())).unwrap();
                }

                // Push the player page to navigation
                if let Some(ref player_page) = self.player_page {
                    let page = adw::NavigationPage::builder()
                        .title("Player")
                        .child(player_page.widget())
                        .build();
                    self.navigation_view.push(&page);
                }
            }
            MainWindowInput::NavigateToPlayerWithContext { media_id, context } => {
                tracing::info!("Navigating to player with context for media: {}", media_id);

                // Save current window state before entering player
                let (width, height) = root.default_size();
                self.saved_window_size = Some((width, height));
                self.was_maximized = root.is_maximized();
                self.was_fullscreen = root.is_fullscreen();

                // Hide window chrome for immersive viewing
                self.content_header.set_visible(false);
                self.sidebar_header.set_visible(false);
                self.split_view.set_collapsed(true);
                self.content_toolbar
                    .set_top_bar_style(adw::ToolbarStyle::Flat);
                self.sidebar_toolbar
                    .set_top_bar_style(adw::ToolbarStyle::Flat);

                // Create player page if not exists
                if self.player_page.is_none() {
                    let db = std::sync::Arc::new(self.db.clone());
                    let sender_clone = sender.clone();
                    self.player_page = Some(
                        PlayerPage::builder()
                            .launch((Some(media_id.clone()), db, root.clone()))
                            .forward(sender.input_sender(), move |output| match output {
                                crate::platforms::relm4::components::pages::player::PlayerOutput::NavigateBack => {
                                    // Restore window chrome when leaving player
                                    sender_clone.input(MainWindowInput::RestoreWindowChrome);
                                    MainWindowInput::Navigate("back".to_string())
                                }
                                crate::platforms::relm4::components::pages::player::PlayerOutput::MediaLoaded => {
                                    tracing::info!("Media loaded in player");
                                    MainWindowInput::Navigate("media_loaded".to_string())
                                }
                                crate::platforms::relm4::components::pages::player::PlayerOutput::Error(msg) => {
                                    tracing::error!("Player error: {}", msg);
                                    // Restore window chrome on error
                                    sender_clone.input(MainWindowInput::RestoreWindowChrome);
                                    MainWindowInput::Navigate("player_error".to_string())
                                }
                                crate::platforms::relm4::components::pages::player::PlayerOutput::WindowStateChanged { width, height } => {
                                    // Player is requesting window size change for aspect ratio
                                    MainWindowInput::ResizeWindow(width, height)
                                }
                            }),
                    );
                    // Send the context to the player
                    if let Some(ref player_page) = self.player_page {
                        player_page.sender().send(crate::platforms::relm4::components::pages::player::PlayerInput::LoadMediaWithContext {
                            media_id: media_id.clone(),
                            context,
                        }).unwrap();
                    }
                } else if let Some(ref player_page) = self.player_page {
                    // Update existing page with new media and context
                    player_page.sender().send(crate::platforms::relm4::components::pages::player::PlayerInput::LoadMediaWithContext {
                        media_id: media_id.clone(),
                        context,
                    }).unwrap();
                }

                // Push the player page to navigation
                if let Some(ref player_page) = self.player_page {
                    let page = adw::NavigationPage::builder()
                        .title("Player")
                        .child(player_page.widget())
                        .build();
                    self.navigation_view.push(&page);
                }
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

                // Show toast notification for sync start
                sender.input(MainWindowInput::ShowToast(
                    "Source added successfully. Starting sync...".to_string(),
                ));

                // Trigger sync in background
                let db = self.db.clone();
                let source_id_clone = source_id.clone();

                sender.oneshot_command(async move {
                    use crate::services::core::backend::BackendService;

                    // Sync the source
                    match BackendService::sync_source(&db, &source_id_clone).await {
                        Ok(sync_result) => {
                            tracing::info!(
                                "Source sync completed: {} items synced",
                                sync_result.items_synced
                            );
                            // Return the result to be handled in the command output
                            (true, sync_result.items_synced, None)
                        }
                        Err(e) => {
                            tracing::error!("Failed to sync source: {}", e);
                            // Return the error to be handled in the command output
                            (false, 0, Some(e.to_string()))
                        }
                    }
                });

                // Schedule UI refresh after sync completes
                let sender_clone = sender.clone();
                relm4::spawn_local(async move {
                    // Wait for sync to complete (approximate time)
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    sender_clone.input(MainWindowInput::Navigate("refresh_sidebar".to_string()));
                    sender_clone.input(MainWindowInput::Navigate(
                        "refresh_sources_page".to_string(),
                    ));
                });
            }
            MainWindowInput::RestoreWindowChrome => {
                tracing::info!("Restoring window chrome after player");

                // Show window chrome again
                self.content_header.set_visible(true);
                self.sidebar_header.set_visible(true);
                self.split_view.set_collapsed(false);
                self.content_toolbar
                    .set_top_bar_style(adw::ToolbarStyle::Raised);
                self.sidebar_toolbar
                    .set_top_bar_style(adw::ToolbarStyle::Raised);

                // Restore cursor visibility when leaving player
                if let Some(surface) = root.surface() {
                    if let Some(cursor) = gtk::gdk::Cursor::from_name("default", None) {
                        surface.set_cursor(Some(&cursor));
                    }
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
            MainWindowInput::ClearHeaderContent => {
                // Clear both header boxes
                while let Some(child) = self.header_start_box.first_child() {
                    self.header_start_box.remove(&child);
                }
                while let Some(child) = self.header_end_box.first_child() {
                    self.header_end_box.remove(&child);
                }
            }
            MainWindowInput::ShowToast(message) => {
                let toast = adw::Toast::new(&message);
                toast.set_timeout(3);
                self.toast_overlay.add_toast(toast);
            }
        }
    }

    async fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        let (success, items_synced, error) = message;
        if success {
            sender.input(MainWindowInput::ShowToast(format!(
                "Sync completed! {} items added to your library",
                items_synced
            )));
        } else if let Some(error_msg) = error {
            sender.input(MainWindowInput::ShowToast(format!(
                "Sync failed: {}",
                error_msg
            )));
        }
    }
}
