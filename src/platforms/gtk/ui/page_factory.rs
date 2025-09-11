use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Weak};

use gtk4::prelude::*;
use tracing::{debug, info};

use super::pages;
use crate::models::{Episode, MediaItem, Movie, Show};
use crate::platforms::gtk::ui::navigation_request::NavigationRequest;
use crate::state::AppState;

/// PageFactory handles page creation and reuse without navigation logic.
/// This separates page lifecycle management from navigation concerns.
#[derive(Debug, Clone)]
pub struct PageFactory {
    state: Arc<AppState>,
    pages: RefCell<HashMap<String, gtk4::Widget>>,
    /// Weak reference to MainWindow for callbacks
    main_window: RefCell<Option<Weak<gtk4::ApplicationWindow>>>,
}

impl PageFactory {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            pages: RefCell::new(HashMap::new()),
            main_window: RefCell::new(None),
        }
    }

    /// Set the main window reference for callbacks
    pub fn set_main_window(&self, window: Weak<gtk4::ApplicationWindow>) {
        self.main_window.replace(Some(window));
    }

    /// Get or create a home page
    pub fn get_or_create_home_page(
        &self,
        source_id: Option<String>,
        on_navigation_request: impl Fn(NavigationRequest) + 'static,
    ) -> pages::HomePage {
        let page_key = format!("home_{}", source_id.as_deref().unwrap_or("all"));

        if let Some(widget) = self.pages.borrow().get(&page_key) {
            debug!("Reusing existing home page for source: {:?}", source_id);
            widget
                .clone()
                .downcast()
                .expect("Widget should be HomePage")
        } else {
            debug!("Creating new home page for source: {:?}", source_id);
            let page = pages::HomePage::new(
                self.state.clone(),
                source_id.clone(),
                |_| {}, // Header setup handled by MainWindow
                on_navigation_request,
            );

            // Store the page widget
            self.pages
                .borrow_mut()
                .insert(page_key, page.clone().upcast());

            page
        }
    }

    /// Get or create a sources page with header setup callback
    pub fn get_or_create_sources_page<F>(&self, on_header_setup: F) -> pages::SourcesPage
    where
        F: Fn(&gtk4::Label, &gtk4::Button) + 'static,
    {
        // Sources page cannot be cached because it needs a new callback each time
        debug!("Creating new sources page");
        let page = pages::SourcesPage::new(self.state.clone(), on_header_setup);

        page
    }

    /// Get or create a library page
    pub fn get_or_create_library_page(&self) -> pages::LibraryView {
        let page_key = "library".to_string();

        if let Some(widget) = self.pages.borrow().get(&page_key) {
            debug!("Reusing existing library page");
            widget
                .clone()
                .downcast()
                .expect("Widget should be LibraryView")
        } else {
            debug!("Creating new library page");
            let page = pages::LibraryView::new(self.state.clone());

            // Store the page widget
            self.pages
                .borrow_mut()
                .insert(page_key, page.clone().upcast());

            page
        }
    }

    /// Get or create a movie details page
    pub fn get_or_create_movie_details_page(&self) -> pages::MovieDetailsPage {
        let page_key = "movie_details".to_string();

        if let Some(widget) = self.pages.borrow().get(&page_key) {
            debug!("Reusing existing movie details page");
            widget
                .clone()
                .downcast()
                .expect("Widget should be MovieDetailsPage")
        } else {
            debug!("Creating new movie details page");
            let page = pages::MovieDetailsPage::new(self.state.clone());

            // Store the page widget
            self.pages
                .borrow_mut()
                .insert(page_key, page.clone().upcast());

            page
        }
    }

    /// Get or create a show details page
    pub fn get_or_create_show_details_page(&self) -> pages::ShowDetailsPage {
        let page_key = "show_details".to_string();

        if let Some(widget) = self.pages.borrow().get(&page_key) {
            debug!("Reusing existing show details page");
            widget
                .clone()
                .downcast()
                .expect("Widget should be ShowDetailsPage")
        } else {
            debug!("Creating new show details page");
            let page = pages::ShowDetailsPage::new(self.state.clone());

            // Store the page widget
            self.pages
                .borrow_mut()
                .insert(page_key, page.clone().upcast());

            page
        }
    }

    /// Create a player page (always creates new, never cached)
    pub fn create_player_page(&self) -> pages::PlayerPage {
        info!("Creating new player page");

        // Player pages are always recreated
        let page = pages::PlayerPage::new(self.state.clone());

        page
    }

    /// Configure a home page with callbacks
    pub fn setup_home_page(
        &self,
        page: &pages::HomePage,
        on_media_selected: impl Fn(&MediaItem) + 'static,
    ) {
        page.set_on_media_selected(on_media_selected);
    }

    /// Configure a library page with callbacks
    pub fn setup_library_page(
        &self,
        page: &pages::LibraryView,
        on_media_selected: impl Fn(&MediaItem) + 'static,
    ) {
        page.set_on_media_selected(on_media_selected);
    }

    /// Configure a movie details page with movie data and callbacks
    pub fn setup_movie_details_page(
        &self,
        page: &pages::MovieDetailsPage,
        movie: &Movie,
        on_play_callback: impl Fn(Movie) + 'static,
    ) {
        page.set_on_play_clicked(move |movie| {
            on_play_callback(movie.clone());
        });

        // Start loading the movie data
        page.load_movie(movie.clone());
    }

    /// Configure a show details page with show data and callbacks
    pub fn setup_show_details_page(
        &self,
        page: &pages::ShowDetailsPage,
        show: &Show,
        on_episode_selected: impl Fn(&Episode) + 'static,
    ) {
        page.set_on_episode_selected(on_episode_selected);

        // Start loading the show data
        page.load_show(show.clone());
    }

    /// Configure a player page with media item
    pub fn setup_player_page(&self, _page: &pages::PlayerPage, _media_item: &MediaItem) {
        // PlayerPage handles loading internally
    }

    /// Get the widget for a page to add to stack
    pub fn get_page_widget(&self, page_key: &str) -> Option<gtk4::Widget> {
        self.pages.borrow().get(page_key).cloned()
    }

    /// Remove a page from the cache
    pub fn remove_page(&self, page_key: &str) {
        debug!("Removing page from cache: {}", page_key);
        self.pages.borrow_mut().remove(page_key);
    }

    /// Clear all cached pages
    pub fn clear_all_pages(&self) {
        debug!("Clearing all cached pages");
        self.pages.borrow_mut().clear();
    }

    /// Get the stack page name for a navigation request
    pub fn get_stack_page_name(request: &NavigationRequest) -> &'static str {
        match request {
            NavigationRequest::ShowHome(_) => "home",
            NavigationRequest::ShowSources => "sources",
            NavigationRequest::ShowLibrary(_, _) | NavigationRequest::ShowLibraryByKey(_) => {
                "library"
            }
            NavigationRequest::ShowMovieDetails(_) => "movie_details",
            NavigationRequest::ShowShowDetails(_) => "show_details",
            NavigationRequest::ShowPlayer(_) => "player",
            _ => "unknown",
        }
    }

    /// Check if a page should be cached or always recreated
    pub fn should_cache_page(request: &NavigationRequest) -> bool {
        match request {
            // Player pages are always recreated
            NavigationRequest::ShowPlayer(_) => false,
            // All other pages can be cached
            _ => true,
        }
    }

    /// Clean up a page before removal (async version)
    pub async fn cleanup_page_async(&self, page_name: &str, _widget: &gtk4::Widget) {
        debug!("Cleaning up page: {}", page_name);

        // Note: PlayerPage cleanup is handled specially in MainWindow
        // because it needs access to the PlayerPage struct, not just the widget.
        // This method is here for future extensibility with other page types.

        // Additional cleanup can be added here for other page types
    }

    /// Check if cleanup is needed for a page type
    pub fn needs_cleanup(&self, page_name: &str) -> bool {
        matches!(page_name, "player")
    }
}
