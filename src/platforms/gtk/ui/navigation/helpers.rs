use super::super::navigation_request::NavigationRequest;
#[allow(unused_imports)]
use crate::models::{Episode, MediaItem, Movie, Show};
use glib::WeakRef;
use gtk4::{glib, prelude::*};

/// NavigationHelper provides utility functions to eliminate repetitive navigation callback patterns
/// throughout the UI code. This centralizes the common "weak reference → upgrade → spawn async → navigate_to"
/// pattern into reusable functions.
pub struct NavigationHelper;

impl NavigationHelper {
    /// Creates a standardized movie play callback that navigates to the player.
    /// This eliminates the repetitive Pattern 3 identified in navigation analysis.
    ///
    /// # Arguments
    /// * `window_weak` - Weak reference to the main window
    ///
    /// # Returns
    /// A closure that takes a Movie and triggers player navigation
    pub fn create_movie_play_callback<W>(window_weak: WeakRef<W>) -> impl Fn(Movie) + 'static
    where
        W: NavigationTarget + ObjectType + 'static,
    {
        move |movie: Movie| {
            if let Some(window) = window_weak.upgrade() {
                let movie_item = MediaItem::Movie(movie);
                glib::spawn_future_local(async move {
                    window
                        .navigate_to(NavigationRequest::ShowPlayer(movie_item))
                        .await;
                });
            }
        }
    }

    /// Creates a standardized episode selection callback that navigates to the player.
    /// This eliminates the repetitive Pattern 4 identified in navigation analysis.
    ///
    /// # Arguments
    /// * `window_weak` - Weak reference to the main window
    ///
    /// # Returns
    /// A closure that takes an Episode reference and triggers player navigation
    pub fn create_episode_play_callback<W>(window_weak: WeakRef<W>) -> impl Fn(&Episode) + 'static
    where
        W: NavigationTarget + ObjectType + 'static,
    {
        move |episode: &Episode| {
            if let Some(window) = window_weak.upgrade() {
                let episode_item = MediaItem::Episode(episode.clone());
                glib::spawn_future_local(async move {
                    window
                        .navigate_to(NavigationRequest::ShowPlayer(episode_item))
                        .await;
                });
            }
        }
    }

    /// Creates a standardized media selection callback that routes to appropriate details/player.
    /// This eliminates the repetitive Pattern 2 identified in navigation analysis.
    ///
    /// # Arguments
    /// * `window_weak` - Weak reference to the main window
    ///
    /// # Returns
    /// A closure that takes a MediaItem reference and routes to correct navigation
    pub fn create_media_selection_callback<W>(
        window_weak: WeakRef<W>,
    ) -> impl Fn(&MediaItem) + 'static
    where
        W: NavigationTarget + ObjectType + 'static,
    {
        move |media_item: &MediaItem| {
            if let Some(window) = window_weak.upgrade() {
                let media_item = media_item.clone();
                glib::spawn_future_local(async move {
                    let nav_request = match &media_item {
                        MediaItem::Movie(movie) => {
                            NavigationRequest::ShowMovieDetails(movie.clone())
                        }
                        MediaItem::Show(show) => NavigationRequest::ShowShowDetails(show.clone()),
                        MediaItem::Episode(_) => NavigationRequest::ShowPlayer(media_item),
                        _ => return,
                    };
                    window.navigate_to(nav_request).await;
                });
            }
        }
    }
}

/// Trait for types that can handle navigation requests.
/// This allows NavigationHelper to work with any type that supports navigation.
pub trait NavigationTarget {
    /// Navigate to the specified request
    async fn navigate_to(&self, request: NavigationRequest);
}
