use adw::prelude::*;
use gtk::gio;
use libadwaita as adw;
use relm4::gtk;
use relm4::prelude::*;

use super::{MainWindow, MainWindowInput};
use crate::models::{LibraryId, MediaItemId, PlaylistContext, SourceId};
use crate::ui::dialogs::PreferencesDialogOutput;
use crate::ui::pages::{
    LibraryPage, MovieDetailsPage, PlayerPage, SearchPage, ShowDetailsPage, SourcesPage,
};
use crate::ui::sidebar::SidebarInput;
use crate::workers::{SearchWorkerInput, SyncWorkerInput};

/// Handle navigation to a specific page by name
pub fn handle_navigate(
    window: &mut MainWindow,
    page: String,
    sender: &AsyncComponentSender<MainWindow>,
) {
    tracing::info!("Navigating to: {}", page);
    match page.as_str() {
        "back" => navigate_back(window, sender),
        "init_sync" => init_sync(window, sender),
        "init_search_index" => init_search_index(),
        "refresh_search_index" => refresh_search_index(window),
        "update_header" => update_header(window),
        "home" => navigate_home(window, sender),
        "sources" => navigate_sources(window, sender),
        "preferences" => navigate_preferences(window, sender),
        "auth_dialog" => navigate_auth_dialog(window),
        "refresh_sidebar" => refresh_sidebar(window),
        "refresh_sources_page" => refresh_sources_page(window),
        _ => {}
    }
}

/// Navigate back to the previous page
fn navigate_back(window: &mut MainWindow, sender: &AsyncComponentSender<MainWindow>) {
    // Check if we have pages to pop (more than 1 page in stack)
    if window.navigation_view.navigation_stack().n_items() > 1 {
        // Check if we're leaving the player page and need to stop it
        if let Some(page) = window.navigation_view.visible_page()
            && page.title() == "Player"
        {
            // Stop the player before leaving the page
            if let Some(ref player_page) = window.player_page {
                tracing::info!("Stopping player before navigation back");
                player_page
                    .sender()
                    .send(crate::ui::pages::player::PlayerInput::Stop)
                    .unwrap_or_else(|_| {
                        tracing::error!("Failed to stop player");
                    });
            }
        }

        window.navigation_view.pop();

        // Clear any custom header content when going back
        sender.input(MainWindowInput::ClearHeaderContent);

        // Check if we're back on Sources page and restore its header button
        if let Some(page) = window.navigation_view.visible_page()
            && page.title() == "Sources"
        {
            // Re-add the "Add Source" button to the header
            let add_button = gtk::Button::builder()
                .icon_name("list-add-symbolic")
                .tooltip_text("Add Source")
                .css_classes(vec!["suggested-action"])
                .build();

            let sender_clone = sender.input_sender().clone();
            add_button.connect_clicked(move |_| {
                sender_clone.emit(MainWindowInput::Navigate("auth_dialog".to_string()));
            });

            sender.input(MainWindowInput::SetHeaderEndContent(Some(
                add_button.upcast(),
            )));
        }
    }
}

/// Initialize sync for all sources on startup
fn init_sync(window: &mut MainWindow, sender: &AsyncComponentSender<MainWindow>) {
    // Check for existing sources and navigate to home if they exist
    let db_clone = window.db.clone();
    let sync_worker = window.sync_worker.sender().clone();

    // First, delay a bit to let UI initialize, then check for sources
    gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(500), {
        let sender = sender.clone();
        let db_clone = db_clone.clone();
        move || {
            // Use spawn_local since sender is not Send
            relm4::spawn_local(async move {
                use crate::services::commands::{Command, auth_commands::LoadSourcesCommand};

                let cmd = LoadSourcesCommand { db: db_clone };
                match cmd.execute().await {
                    Ok(sources) => {
                        tracing::info!("Found {} sources to sync on startup", sources.len());

                        if !sources.is_empty() {
                            tracing::info!("Sources configured, navigating to home immediately");
                            sender.input(MainWindowInput::Navigate("home".to_string()));

                            // Trigger sync for all sources via SyncWorker
                            for source in sources {
                                let source_id = crate::models::SourceId::from(source.id.clone());
                                tracing::info!(
                                    "Triggering startup sync for source: {}",
                                    source.name
                                );

                                sync_worker
                                    .send(SyncWorkerInput::StartSync {
                                        source_id,
                                        library_id: None,
                                        force: false,
                                    })
                                    .unwrap_or_else(|e| {
                                        tracing::error!(
                                            "Failed to send sync command to worker: {:?}",
                                            e
                                        );
                                    });
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to load sources for initial sync: {}", e);
                    }
                }
            });
        }
    });

    // Schedule a delayed sidebar refresh since sources might sync
    let sender_clone_2 = sender.clone();
    gtk::glib::timeout_add_local_once(std::time::Duration::from_secs(5), move || {
        sender_clone_2.input(MainWindowInput::Navigate("refresh_sidebar".to_string()));
    });
}

/// Initialize search index (handled automatically by SearchWorker)
fn init_search_index() {
    // Search index is now initialized automatically by SearchWorker
    // on startup via broker messages - no manual initialization needed
    tracing::info!("Search index loads automatically via SearchWorker");
}

/// Refresh search index after sync (handled automatically by broker)
fn refresh_search_index(window: &mut MainWindow) {
    // Search index is now updated incrementally via broker messages
    // No manual refresh needed - just show the sync completed toast
    tracing::info!("Search index updates automatically via broker");
    window.toast_overlay.add_toast(
        adw::Toast::builder()
            .title("Sync completed")
            .timeout(3)
            .build(),
    );
}

/// Update header based on current navigation state
fn update_header(window: &mut MainWindow) {
    // Update back button visibility based on navigation stack
    let can_pop = window.navigation_view.navigation_stack().n_items() > 1;
    window.back_button.set_visible(can_pop);

    // Update title and subtitle based on current page
    if let Some(page) = window.navigation_view.visible_page() {
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
        window.content_title.set_title(&title);
        window.content_title.set_subtitle(subtitle);
    }
}

/// Navigate to home page
fn navigate_home(window: &mut MainWindow, sender: &AsyncComponentSender<MainWindow>) {
    // Switch to content view and show home page
    window.content_stack.set_visible_child_name("content");

    // Clear library tracking since we're on home
    window.current_library_id = None;

    // Check if we need to navigate to home or if we're already there
    let stack = window.navigation_view.navigation_stack();

    // First check if the visible page is already "Home"
    if let Some(visible_page) = window.navigation_view.visible_page()
        && visible_page.title() == "Home"
    {
        // Already on home page, don't do anything except update header
        // Don't pop any pages as that would remove the current Home page

        // Clear any custom header content
        sender.input(MainWindowInput::ClearHeaderContent);
        sender.input(MainWindowInput::Navigate("update_header".to_string()));
        return;
    }

    // Pop all pages except the root page
    while stack.n_items() > 1 {
        window.navigation_view.pop();
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
            window.navigation_view.pop();
        }

        // Create and push home page
        let home_nav_page = adw::NavigationPage::builder()
            .title("Home")
            .child(window.home_page.widget())
            .build();
        window.navigation_view.push(&home_nav_page);
    }

    // Clear any custom header content
    sender.input(MainWindowInput::ClearHeaderContent);

    // Trigger header update
    sender.input(MainWindowInput::Navigate("update_header".to_string()));
}

/// Navigate to sources page
fn navigate_sources(window: &mut MainWindow, sender: &AsyncComponentSender<MainWindow>) {
    // Switch to content view
    window.content_stack.set_visible_child_name("content");

    // Clear library tracking since we're on sources
    window.current_library_id = None;

    // Check if Sources page exists in the navigation stack
    let stack = window.navigation_view.navigation_stack();
    let mut sources_page_index = None;

    for i in 0..stack.n_items() {
        if let Some(page) = stack.item(i)
            && let Ok(nav_page) = page.downcast::<adw::NavigationPage>()
            && nav_page.title() == "Sources"
        {
            sources_page_index = Some(i);
            break;
        }
    }

    // If Sources page exists in stack, pop to it instead of pushing new one
    if let Some(index) = sources_page_index {
        // Pop back to the Sources page
        while window.navigation_view.navigation_stack().n_items() > index + 1 {
            window.navigation_view.pop();
        }

        // If Sources page is not visible, make it visible
        if let Some(visible_page) = window.navigation_view.visible_page()
            && visible_page.title() != "Sources"
        {
            // Navigate to the Sources page that's already in the stack
            if let Some(page) = stack.item(index)
                && let Ok(_nav_page) = page.downcast::<adw::NavigationPage>()
            {
                // The page should now be visible after popping
            }
        }
    } else {
        // Sources page doesn't exist in stack, create and push it

        // Create sources page if it doesn't exist
        if window.sources_page.is_none() {
            let sources_controller = SourcesPage::builder().launch(window.db.clone()).forward(
                sender.input_sender(),
                |output| match output {
                    crate::ui::pages::sources::SourcesPageOutput::OpenAuthDialog => {
                        tracing::info!("Opening auth dialog for adding source");
                        MainWindowInput::Navigate("auth_dialog".to_string())
                    }
                    crate::ui::pages::sources::SourcesPageOutput::SyncSource(source_id) => {
                        tracing::info!("Source page requesting sync for: {:?}", source_id);
                        MainWindowInput::SyncSource(source_id)
                    }
                },
            );

            // Create the navigation page once
            let page = adw::NavigationPage::builder()
                .title("Sources")
                .child(sources_controller.widget())
                .build();

            window.sources_nav_page = Some(page);
            window.sources_page = Some(sources_controller);
        }

        // Push the existing navigation page
        if let Some(ref page) = window.sources_nav_page {
            window.navigation_view.push(page);
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

/// Navigate to preferences dialog
fn navigate_preferences(window: &mut MainWindow, sender: &AsyncComponentSender<MainWindow>) {
    // Switch to content view
    window.content_stack.set_visible_child_name("content");

    // Check if we're already on the preferences page
    if let Some(visible_page) = window.navigation_view.visible_page()
        && visible_page.title() == "Preferences"
    {
        // Already on preferences page, don't push again
        return;
    }

    // Create and show preferences dialog
    create_and_show_preferences_dialog(window, sender);
}

/// Open authentication dialog
fn navigate_auth_dialog(window: &mut MainWindow) {
    tracing::info!("Opening authentication dialog");
    // Send show message to the auth dialog
    window
        .auth_dialog
        .emit(crate::ui::dialogs::AuthDialogInput::Show);
}

/// Refresh sidebar after sync
fn refresh_sidebar(window: &mut MainWindow) {
    tracing::info!("Refreshing sidebar after sync");
    // Trigger sidebar refresh
    window.sidebar.emit(SidebarInput::RefreshSources);
}

/// Refresh sources page if it exists
fn refresh_sources_page(window: &mut MainWindow) {
    // Refresh sources page if it exists
    if let Some(ref sources_page) = window.sources_page {
        tracing::info!("Refreshing sources page after sync");
        sources_page.emit(crate::ui::pages::sources::SourcesPageInput::LoadData);
    }
}

/// Navigate to preferences dialog (from menu)
pub fn navigate_to_preferences(
    window: &mut MainWindow,
    sender: &AsyncComponentSender<MainWindow>,
    root: &adw::ApplicationWindow,
) {
    tracing::info!("Opening preferences dialog");
    create_and_show_preferences_dialog_with_root(window, sender, root);
}

/// Navigate to search page
pub fn navigate_to_search(window: &mut MainWindow, sender: &AsyncComponentSender<MainWindow>) {
    tracing::info!("Navigating to search page");

    // Switch to content view
    window.content_stack.set_visible_child_name("content");

    // Check if Search page exists in the navigation stack
    let stack = window.navigation_view.navigation_stack();
    let mut search_page_index = None;

    for i in 0..stack.n_items() {
        if let Some(page) = stack.item(i)
            && let Ok(nav_page) = page.downcast::<adw::NavigationPage>()
            && nav_page.title() == "Search"
        {
            search_page_index = Some(i);
            break;
        }
    }

    // If Search page exists in stack, pop to it instead of pushing new one
    if let Some(index) = search_page_index {
        // Pop back to the Search page
        while window.navigation_view.navigation_stack().n_items() > index + 1 {
            window.navigation_view.pop();
        }

        // If Search page is not visible, make it visible
        if let Some(visible_page) = window.navigation_view.visible_page()
            && visible_page.title() != "Search"
        {
            // Navigate to the Search page that's already in the stack
            if let Some(page) = stack.item(index)
                && let Ok(_nav_page) = page.downcast::<adw::NavigationPage>()
            {
                // The page should now be visible after popping
            }
        }
    } else {
        // Search page doesn't exist in stack, create and push it

        // Create search controller if it doesn't exist
        if window.search_page.is_none() {
            use crate::ui::pages::search::{SearchPage, SearchPageOutput};

            let search_controller = SearchPage::builder().launch(window.db.clone()).forward(
                sender.input_sender(),
                |output| match output {
                    SearchPageOutput::NavigateToMediaItem(id) => {
                        MainWindowInput::NavigateToMediaItem(id)
                    }
                },
            );

            // Create the navigation page once
            let page = adw::NavigationPage::builder()
                .title("Search")
                .child(search_controller.widget())
                .build();

            window.search_nav_page = Some(page);
            window.search_page = Some(search_controller);
        }

        // Push the existing navigation page
        if let Some(ref page) = window.search_nav_page {
            window.navigation_view.push(page);
        }
    }
}

/// Navigate to a specific source
pub fn navigate_to_source(
    window: &mut MainWindow,
    source_id: SourceId,
    _sender: &AsyncComponentSender<MainWindow>,
) {
    tracing::info!("Navigating to source: {}", source_id);

    // Switch to content view
    window.content_stack.set_visible_child_name("content");

    // TODO: Create and push source page
    // For now, just create a placeholder page
    let page = adw::NavigationPage::builder()
        .title(format!("Source: {}", source_id))
        .child(&gtk::Label::new(Some(&format!(
            "Source {} content",
            source_id
        ))))
        .build();
    window.navigation_view.push(&page);
}

/// Navigate to library page
pub fn navigate_to_library(
    window: &mut MainWindow,
    library_id: LibraryId,
    sender: &AsyncComponentSender<MainWindow>,
) {
    tracing::info!("Navigating to library: {}", library_id);

    // Check if we're already on this library page
    if let Some(ref current_id) = window.current_library_id
        && current_id == &library_id
    {
        tracing::debug!("Already on library: {}, skipping navigation", library_id);
        return;
    }

    // Check if we're already on a library page in the navigation stack
    // and if so, pop back to before it
    if window.current_library_id.is_some() {
        // We're on a library page, check if it's the visible page
        if let Some(visible_page) = window.navigation_view.visible_page() {
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
                window.navigation_view.pop();
            }
        }
    }

    // Switch to content view
    window.content_stack.set_visible_child_name("content");

    // Always recreate the library page for each navigation to avoid widget parent conflicts
    // This ensures the widget isn't already attached to another navigation page
    let library_controller =
        LibraryPage::builder()
            .launch(window.db.clone())
            .forward(sender.input_sender(), |output| match output {
                crate::ui::pages::library::LibraryPageOutput::NavigateToMediaItem(id) => {
                    MainWindowInput::NavigateToMediaItem(id)
                }
                crate::ui::pages::library::LibraryPageOutput::SetHeaderTitleWidget(widget) => {
                    MainWindowInput::SetTitleWidget(Some(widget))
                }
            });

    // Set the library on the new controller
    library_controller.emit(crate::ui::pages::library::LibraryPageInput::SetLibrary(
        library_id.clone(),
    ));

    // Create navigation page with the new controller's widget
    let page = adw::NavigationPage::builder()
        .title("Library")
        .child(library_controller.widget())
        .build();

    // Store the controller for later use
    window.library_page = Some(library_controller);

    // Update current library ID
    window.current_library_id = Some(library_id);

    // Push the page to navigation
    window.navigation_view.push(&page);

    // Clear any custom header content (library page can add its own filters later)
    sender.input(MainWindowInput::ClearHeaderContent);

    // Trigger header update
    sender.input(MainWindowInput::Navigate("update_header".to_string()));
}

/// Navigate to media item (determines type and navigates accordingly)
pub fn navigate_to_media_item(
    window: &mut MainWindow,
    item_id: MediaItemId,
    sender: &AsyncComponentSender<MainWindow>,
) {
    tracing::info!("Navigating to media item: {}", item_id);

    // Clear library tracking since we're navigating to a media item
    window.current_library_id = None;

    // Switch to content view
    window.content_stack.set_visible_child_name("content");

    // First, we need to determine what type of media this is
    let db_clone = window.db.clone();
    let item_id_clone = item_id.clone();
    let sender_clone = sender.clone();

    relm4::spawn_local(async move {
        use crate::db::repository::{MediaRepositoryImpl, Repository};

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
                    sender_clone.input(MainWindowInput::NavigateToPlayer(item_id_clone));
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

/// Navigate to movie details page
pub fn navigate_to_movie(
    window: &mut MainWindow,
    item_id: MediaItemId,
    sender: &AsyncComponentSender<MainWindow>,
) {
    tracing::info!("Navigating to movie: {}", item_id);

    // Always recreate the movie details page to avoid widget parent conflicts
    // This ensures the widget isn't already attached to another navigation page
    let db = std::sync::Arc::new(window.db.clone());
    let movie_controller = MovieDetailsPage::builder()
        .launch((item_id.clone(), db))
        .forward(sender.input_sender(), |output| match output {
            crate::ui::pages::movie_details::MovieDetailsOutput::PlayMedia(id) => {
                tracing::info!("Playing media: {}", id);
                MainWindowInput::NavigateToPlayer(id)
            }
        });

    // Create navigation page with the new controller's widget
    let page = adw::NavigationPage::builder()
        .title("Movie Details")
        .child(movie_controller.widget())
        .build();

    // Store the controller for later use
    window.movie_details_page = Some(movie_controller);

    // Push the page to navigation
    window.navigation_view.push(&page);
}

/// Navigate to show details page
pub fn navigate_to_show(
    window: &mut MainWindow,
    item_id: MediaItemId,
    sender: &AsyncComponentSender<MainWindow>,
) {
    tracing::info!("Navigating to show: {}", item_id);

    // Always recreate the show details page to avoid widget parent conflicts
    // This ensures the widget isn't already attached to another navigation page
    let db = std::sync::Arc::new(window.db.clone());
    let show_controller = ShowDetailsPage::builder()
        .launch((item_id.clone(), db))
        .forward(sender.input_sender(), move |output| match output {
            crate::ui::pages::show_details::ShowDetailsOutput::PlayMedia(id) => {
                tracing::info!("Playing episode: {}", id);
                MainWindowInput::NavigateToPlayer(id)
            }
            crate::ui::pages::show_details::ShowDetailsOutput::PlayMediaWithContext {
                media_id,
                context,
            } => {
                tracing::info!("Playing episode with context: {}", media_id);
                MainWindowInput::NavigateToPlayerWithContext { media_id, context }
            }
        });

    // Create navigation page with the new controller's widget
    let page = adw::NavigationPage::builder()
        .title("Show Details")
        .child(show_controller.widget())
        .build();

    // Store the controller for later use
    window.show_details_page = Some(show_controller);

    // Push the page to navigation
    window.navigation_view.push(&page);
}

/// Navigate to player page
pub fn navigate_to_player(
    window: &mut MainWindow,
    media_id: MediaItemId,
    sender: &AsyncComponentSender<MainWindow>,
    root: &adw::ApplicationWindow,
) {
    tracing::info!("Navigating to player for media: {}", media_id);

    // Save current window state before entering player
    let (width, height) = root.default_size();
    window.saved_window_size = Some((width, height));
    window.was_maximized = root.is_maximized();
    window.was_fullscreen = root.is_fullscreen();

    // Hide window chrome for immersive viewing
    window.content_header.set_visible(false);
    window.sidebar_header.set_visible(false);
    window.split_view.set_collapsed(true);
    window
        .content_toolbar
        .set_top_bar_style(adw::ToolbarStyle::Flat);
    window
        .sidebar_toolbar
        .set_top_bar_style(adw::ToolbarStyle::Flat);

    // Create player page if not exists
    if window.player_page.is_none() {
        let db = std::sync::Arc::new(window.db.clone());
        let sender_clone = sender.clone();
        window.player_page = Some(
            PlayerPage::builder()
                .launch((Some(media_id.clone()), db, root.clone()))
                .forward(sender.input_sender(), move |output| match output {
                    crate::ui::pages::player::PlayerOutput::NavigateBack => {
                        // Restore window chrome when leaving player
                        sender_clone.input(MainWindowInput::RestoreWindowChrome);
                        MainWindowInput::Navigate("back".to_string())
                    }
                    crate::ui::pages::player::PlayerOutput::MediaLoaded => {
                        tracing::info!("Media loaded in player");
                        MainWindowInput::Navigate("media_loaded".to_string())
                    }
                    crate::ui::pages::player::PlayerOutput::Error(msg) => {
                        tracing::error!("Player error: {}", msg);
                        // Show error toast - user can manually navigate back
                        let toast_msg = format!("Playback error: {}", msg);
                        MainWindowInput::ShowToast(toast_msg)
                    }
                    crate::ui::pages::player::PlayerOutput::ShowToast(msg) => {
                        MainWindowInput::ShowToast(msg)
                    }
                    crate::ui::pages::player::PlayerOutput::WindowStateChanged {
                        width,
                        height,
                    } => {
                        // Player is requesting window size change for aspect ratio
                        MainWindowInput::ResizeWindow(width, height)
                    }
                }),
        );
    } else if let Some(ref player_page) = window.player_page {
        // Update existing page with new media
        player_page
            .sender()
            .send(crate::ui::pages::player::PlayerInput::LoadMedia(
                media_id.clone(),
            ))
            .unwrap();
    }

    // Push the player page to navigation
    if let Some(ref player_page) = window.player_page {
        let page = adw::NavigationPage::builder()
            .title("Player")
            .child(player_page.widget())
            .build();
        window.navigation_view.push(&page);
    }
}

/// Navigate to player page with playback context
pub fn navigate_to_player_with_context(
    window: &mut MainWindow,
    media_id: MediaItemId,
    context: PlaylistContext,
    sender: &AsyncComponentSender<MainWindow>,
    root: &adw::ApplicationWindow,
) {
    tracing::info!("Navigating to player with context for media: {}", media_id);

    // Save current window state before entering player
    let (width, height) = root.default_size();
    window.saved_window_size = Some((width, height));
    window.was_maximized = root.is_maximized();
    window.was_fullscreen = root.is_fullscreen();

    // Hide window chrome for immersive viewing
    window.content_header.set_visible(false);
    window.sidebar_header.set_visible(false);
    window.split_view.set_collapsed(true);
    window
        .content_toolbar
        .set_top_bar_style(adw::ToolbarStyle::Flat);
    window
        .sidebar_toolbar
        .set_top_bar_style(adw::ToolbarStyle::Flat);

    // Create player page if not exists
    if window.player_page.is_none() {
        let db = std::sync::Arc::new(window.db.clone());
        let sender_clone = sender.clone();
        window.player_page = Some(
            PlayerPage::builder()
                .launch((Some(media_id.clone()), db, root.clone()))
                .forward(sender.input_sender(), move |output| match output {
                    crate::ui::pages::player::PlayerOutput::NavigateBack => {
                        // Restore window chrome when leaving player
                        sender_clone.input(MainWindowInput::RestoreWindowChrome);
                        MainWindowInput::Navigate("back".to_string())
                    }
                    crate::ui::pages::player::PlayerOutput::MediaLoaded => {
                        tracing::info!("Media loaded in player");
                        MainWindowInput::Navigate("media_loaded".to_string())
                    }
                    crate::ui::pages::player::PlayerOutput::Error(msg) => {
                        tracing::error!("Player error: {}", msg);
                        // Show error toast - user can manually navigate back
                        let toast_msg = format!("Playback error: {}", msg);
                        MainWindowInput::ShowToast(toast_msg)
                    }
                    crate::ui::pages::player::PlayerOutput::ShowToast(msg) => {
                        MainWindowInput::ShowToast(msg)
                    }
                    crate::ui::pages::player::PlayerOutput::WindowStateChanged {
                        width,
                        height,
                    } => {
                        // Player is requesting window size change for aspect ratio
                        MainWindowInput::ResizeWindow(width, height)
                    }
                }),
        );
        // Send the context to the player
        if let Some(ref player_page) = window.player_page {
            player_page
                .sender()
                .send(
                    crate::ui::pages::player::PlayerInput::LoadMediaWithContext {
                        media_id: media_id.clone(),
                        context,
                    },
                )
                .unwrap();
        }
    } else if let Some(ref player_page) = window.player_page {
        // Update existing page with new media and context
        player_page
            .sender()
            .send(
                crate::ui::pages::player::PlayerInput::LoadMediaWithContext {
                    media_id: media_id.clone(),
                    context,
                },
            )
            .unwrap();
    }

    // Push the player page to navigation
    if let Some(ref player_page) = window.player_page {
        let page = adw::NavigationPage::builder()
            .title("Player")
            .child(player_page.widget())
            .build();
        window.navigation_view.push(&page);
    }
}

/// Helper function to create and show preferences dialog
fn create_and_show_preferences_dialog(
    window: &mut MainWindow,
    sender: &AsyncComponentSender<MainWindow>,
) {
    // This function is called without root, so we can't present the dialog
    // Instead, we'll just create it if needed
    if window.preferences_dialog.is_none() {
        use crate::ui::dialogs::PreferencesDialog;

        let preferences_controller = PreferencesDialog::builder()
            .launch(window.db.clone())
            .forward(sender.input_sender(), |output| match output {
                PreferencesDialogOutput::Closed => {
                    tracing::info!("Preferences dialog closed");
                    MainWindowInput::Navigate("preferences_closed".to_string())
                }
            });

        window.preferences_dialog = Some(preferences_controller);
    } else if let Some(ref dialog) = window.preferences_dialog {
        // Send reload config message to refresh with current values
        dialog
            .sender()
            .send(crate::ui::dialogs::PreferencesDialogInput::ReloadConfig)
            .ok();
    }
}

/// Helper function to create and show preferences dialog with root window
fn create_and_show_preferences_dialog_with_root(
    window: &mut MainWindow,
    sender: &AsyncComponentSender<MainWindow>,
    root: &adw::ApplicationWindow,
) {
    if window.preferences_dialog.is_none() {
        use crate::ui::dialogs::PreferencesDialog;

        let preferences_controller = PreferencesDialog::builder()
            .launch(window.db.clone())
            .forward(sender.input_sender(), |output| match output {
                PreferencesDialogOutput::Closed => {
                    tracing::info!("Preferences dialog closed");
                    MainWindowInput::Navigate("preferences_closed".to_string())
                }
            });

        preferences_controller.widget().present(Some(root));
        window.preferences_dialog = Some(preferences_controller);
    } else if let Some(ref dialog) = window.preferences_dialog {
        // Send reload config message to refresh with current values
        dialog
            .sender()
            .send(crate::ui::dialogs::PreferencesDialogInput::ReloadConfig)
            .ok();
        dialog.widget().present(Some(root));
    }
}
