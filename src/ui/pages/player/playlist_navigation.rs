use crate::models::PlaylistContext;
use relm4::prelude::*;
use tracing::debug;

use super::{PlayerInput, PlayerPage};

/// Playlist navigation and position tracking methods
impl PlayerPage {
    /// Update the playlist position label based on current context
    pub(super) fn update_playlist_position_label(&self, context: &PlaylistContext) {
        match context {
            PlaylistContext::SingleItem => {
                self.playlist_position_label.set_text("");
            }
            PlaylistContext::TvShow {
                show_title,
                current_index,
                episodes,
                ..
            } => {
                if let Some(current_episode) = episodes.get(*current_index) {
                    let text = format!(
                        "{} - S{}E{} - Episode {} of {}",
                        show_title,
                        current_episode.season_number,
                        current_episode.episode_number,
                        current_index + 1,
                        episodes.len()
                    );
                    self.playlist_position_label.set_text(&text);
                }
            }
            PlaylistContext::PlayQueue {
                current_index,
                items,
                ..
            } => {
                if let Some(current_item) = items.get(*current_index) {
                    let text = format!(
                        "{} - Item {} of {}",
                        current_item.title,
                        current_index + 1,
                        items.len()
                    );
                    self.playlist_position_label.set_text(&text);
                }
            }
        }
    }

    /// Handle previous episode/item navigation
    pub(super) fn handle_previous_navigation(&self, sender: &AsyncComponentSender<Self>) {
        debug!("Previous track requested");

        if let Some(ref context) = self.playlist_context {
            if let Some(prev_id) = context.get_previous_item() {
                // Keep the context and just load the previous media
                let mut new_context = context.clone();
                new_context.update_current_index(&prev_id);

                sender.input(PlayerInput::LoadMediaWithContext {
                    media_id: prev_id,
                    context: new_context,
                });
            } else {
                debug!("No previous episode available");
            }
        } else {
            debug!("No playlist context available for previous navigation");
        }
    }

    /// Handle next episode/item navigation
    pub(super) fn handle_next_navigation(&self, sender: &AsyncComponentSender<Self>) {
        debug!("Next track requested");

        if let Some(ref context) = self.playlist_context {
            if let Some(next_id) = context.get_next_item() {
                // Keep the context and just load the next media
                let mut new_context = context.clone();
                new_context.update_current_index(&next_id);

                sender.input(PlayerInput::LoadMediaWithContext {
                    media_id: next_id,
                    context: new_context,
                });
            } else {
                debug!("No next episode available");
            }
        } else {
            debug!("No playlist context available for next navigation");
        }
    }
}
