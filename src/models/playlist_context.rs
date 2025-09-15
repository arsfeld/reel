use crate::models::{MediaItemId, ShowId};
use serde::{Deserialize, Serialize};

/// Represents the context in which media is playing, enabling navigation between items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlaylistContext {
    /// Single item with no playlist context
    SingleItem,

    /// TV show episode playlist
    TvShow {
        /// The parent show ID
        show_id: ShowId,
        /// Display title of the show
        show_title: String,
        /// Current episode index in the episodes list
        current_index: usize,
        /// All episodes in playback order
        episodes: Vec<EpisodeInfo>,
        /// Whether to automatically play the next episode
        auto_play_next: bool,
    },
    // Future variants:
    // Album { ... }
    // Playlist { ... }
    // Queue { ... }
}

/// Minimal episode information for playlist context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeInfo {
    /// Episode media item ID
    pub id: MediaItemId,
    /// Episode title
    pub title: String,
    /// Season number
    pub season_number: u32,
    /// Episode number within the season
    pub episode_number: u32,
    /// Duration in milliseconds
    pub duration_ms: Option<i64>,
    /// Whether the episode has been watched
    pub watched: bool,
    /// Current playback position in milliseconds
    pub playback_position_ms: Option<i64>,
}

impl PlaylistContext {
    /// Get the next item in the playlist after the current one
    pub fn get_next_item(&self) -> Option<MediaItemId> {
        match self {
            PlaylistContext::SingleItem => None,
            PlaylistContext::TvShow {
                episodes,
                current_index,
                ..
            } => {
                if *current_index + 1 < episodes.len() {
                    Some(episodes[*current_index + 1].id.clone())
                } else {
                    None
                }
            }
        }
    }

    /// Get the previous item in the playlist before the current one
    pub fn get_previous_item(&self) -> Option<MediaItemId> {
        match self {
            PlaylistContext::SingleItem => None,
            PlaylistContext::TvShow {
                episodes,
                current_index,
                ..
            } => {
                if *current_index > 0 {
                    Some(episodes[*current_index - 1].id.clone())
                } else {
                    None
                }
            }
        }
    }

    /// Update the current index when playing a different item
    pub fn update_current_index(&mut self, item_id: &MediaItemId) -> bool {
        match self {
            PlaylistContext::SingleItem => false,
            PlaylistContext::TvShow {
                episodes,
                current_index,
                ..
            } => {
                if let Some(new_index) = episodes.iter().position(|e| &e.id == item_id) {
                    *current_index = new_index;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Check if there's a next item available
    pub fn has_next(&self) -> bool {
        match self {
            PlaylistContext::SingleItem => false,
            PlaylistContext::TvShow {
                episodes,
                current_index,
                ..
            } => *current_index + 1 < episodes.len(),
        }
    }

    /// Check if there's a previous item available
    pub fn has_previous(&self) -> bool {
        match self {
            PlaylistContext::SingleItem => false,
            PlaylistContext::TvShow { current_index, .. } => *current_index > 0,
        }
    }

    /// Get information about the next episode (for auto-play UI)
    pub fn get_next_episode_info(&self) -> Option<&EpisodeInfo> {
        match self {
            PlaylistContext::SingleItem => None,
            PlaylistContext::TvShow {
                episodes,
                current_index,
                ..
            } => {
                if *current_index + 1 < episodes.len() {
                    Some(&episodes[*current_index + 1])
                } else {
                    None
                }
            }
        }
    }
}
