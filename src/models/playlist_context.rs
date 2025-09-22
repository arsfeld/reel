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
        /// Optional Plex PlayQueue metadata
        play_queue_info: Option<PlayQueueInfo>,
    },

    /// Generic PlayQueue context (for movies, mixed content, playlists)
    PlayQueue {
        /// Plex PlayQueue metadata
        play_queue_info: PlayQueueInfo,
        /// Current item index in the queue
        current_index: usize,
        /// All items in the queue
        items: Vec<QueueItem>,
        /// Whether to automatically play the next item
        auto_play_next: bool,
    },
}

/// Plex PlayQueue metadata for server-side queue management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayQueueInfo {
    /// PlayQueue ID from Plex server
    pub play_queue_id: i64,
    /// PlayQueue version for sync tracking
    pub play_queue_version: i32,
    /// Currently selected item ID in the queue
    pub play_queue_item_id: i64,
    /// Source URI that created this queue
    pub source_uri: Option<String>,
    /// Whether the queue is shuffled
    pub shuffled: bool,
}

/// Generic queue item for PlayQueue context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    /// Media item ID
    pub id: MediaItemId,
    /// Item title
    pub title: String,
    /// Media type (movie, episode, track, etc.)
    pub media_type: String,
    /// Duration in milliseconds
    pub duration_ms: Option<i64>,
    /// PlayQueue item ID for Plex sync
    pub play_queue_item_id: Option<i64>,
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
    /// PlayQueue item ID for Plex sync
    pub play_queue_item_id: Option<i64>,
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
            PlaylistContext::PlayQueue {
                items,
                current_index,
                ..
            } => {
                if *current_index + 1 < items.len() {
                    Some(items[*current_index + 1].id.clone())
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
            PlaylistContext::PlayQueue {
                items,
                current_index,
                ..
            } => {
                if *current_index > 0 {
                    Some(items[*current_index - 1].id.clone())
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
            PlaylistContext::PlayQueue {
                items,
                current_index,
                play_queue_info,
                ..
            } => {
                if let Some(new_index) = items.iter().position(|i| &i.id == item_id) {
                    *current_index = new_index;
                    // Update the selected item ID in PlayQueue info
                    if let Some(item) = items.get(new_index)
                        && let Some(item_id) = item.play_queue_item_id
                    {
                        play_queue_info.play_queue_item_id = item_id;
                    }
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
            PlaylistContext::PlayQueue {
                items,
                current_index,
                ..
            } => *current_index + 1 < items.len(),
        }
    }

    /// Check if there's a previous item available
    pub fn has_previous(&self) -> bool {
        match self {
            PlaylistContext::SingleItem => false,
            PlaylistContext::TvShow { current_index, .. } => *current_index > 0,
            PlaylistContext::PlayQueue { current_index, .. } => *current_index > 0,
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
            PlaylistContext::PlayQueue { .. } => {
                // PlayQueue uses generic items, not episodes
                None
            }
        }
    }

    /// Get PlayQueue info if available
    pub fn get_play_queue_info(&self) -> Option<&PlayQueueInfo> {
        match self {
            PlaylistContext::SingleItem => None,
            PlaylistContext::TvShow {
                play_queue_info, ..
            } => play_queue_info.as_ref(),
            PlaylistContext::PlayQueue {
                play_queue_info, ..
            } => Some(play_queue_info),
        }
    }

    /// Check if auto-play is enabled
    pub fn is_auto_play_enabled(&self) -> bool {
        match self {
            PlaylistContext::SingleItem => false,
            PlaylistContext::TvShow { auto_play_next, .. } => *auto_play_next,
            PlaylistContext::PlayQueue { auto_play_next, .. } => *auto_play_next,
        }
    }
}
