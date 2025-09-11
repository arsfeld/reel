use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gtk4::prelude::*;

use super::pages;
use crate::state::AppState;

/// PageFactory handles page creation and reuse without navigation logic.
/// This separates page lifecycle management from navigation concerns.
#[derive(Debug, Clone)]
pub struct PageFactory {
    state: Arc<AppState>,
    pages: RefCell<HashMap<String, gtk4::Widget>>,
}

impl PageFactory {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            pages: RefCell::new(HashMap::new()),
        }
    }

    /// Get or create a movie details page
    pub fn get_or_create_movie_details_page(&self) -> pages::MovieDetailsPage {
        let page_key = "movie_details".to_string();

        if let Some(widget) = self.pages.borrow().get(&page_key) {
            // Page exists, return it
            widget
                .clone()
                .downcast()
                .expect("Widget should be MovieDetailsPage")
        } else {
            // Create new page
            let page = pages::MovieDetailsPage::new(self.state.clone());

            // Store the page widget
            self.pages
                .borrow_mut()
                .insert(page_key, page.clone().upcast());

            page
        }
    }

    /// Configure a movie details page with movie data and callbacks
    pub fn setup_movie_details_page(
        &self,
        page: &pages::MovieDetailsPage,
        movie: &crate::models::Movie,
        on_play_callback: impl Fn(crate::models::Movie) + 'static,
    ) {
        // Set callback for when play is clicked
        page.set_on_play_clicked(move |movie| {
            on_play_callback(movie.clone());
        });

        // Start loading the movie data
        page.load_movie(movie.clone());
    }

    /// Get the widget for a page to add to stack
    pub fn get_page_widget(&self, page_key: &str) -> Option<gtk4::Widget> {
        self.pages.borrow().get(page_key).cloned()
    }
}
