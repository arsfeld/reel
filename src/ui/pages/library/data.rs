use gtk::prelude::*;
use relm4::prelude::*;
use std::collections::HashMap;
use tracing::{debug, error, trace};

use super::LibraryPage;
use super::messages::LibraryPageInput;
use super::types::{SortBy, SortOrder};
use crate::workers::ImageLoaderInput;

impl LibraryPage {
    /// Load all media items from database for the current library
    pub(super) fn load_all_items(&mut self, sender: AsyncComponentSender<Self>) {
        if let Some(library_id) = &self.library_id {
            self.is_loading = true;

            let db = self.db.clone();
            let library_id = library_id.clone();
            let sort_by = self.sort_by;
            let sort_order = self.sort_order;
            let selected_media_type = self.selected_media_type.clone();

            relm4::spawn_local(async move {
                use crate::db::repository::{
                    LibraryRepositoryImpl, MediaRepository, MediaRepositoryImpl, Repository,
                };
                let library_repo = LibraryRepositoryImpl::new(db.clone());
                let media_repo = MediaRepositoryImpl::new(db.clone());

                // First, get the library to determine its type
                let library_result = library_repo.find_by_id(library_id.as_ref()).await;

                let (library_type, media_result) = match library_result {
                    Ok(Some(library)) => {
                        let lib_type = library.library_type.to_lowercase();

                        // For mixed libraries, check if we have a media type filter
                        let media_result = if lib_type == "mixed" {
                            // Use the selected media type filter if set
                            if let Some(media_type) = selected_media_type {
                                media_repo
                                    .find_by_library_and_type(library_id.as_ref(), &media_type)
                                    .await
                            } else {
                                // Get all items if no filter is set
                                media_repo.find_by_library(library_id.as_ref()).await
                            }
                        } else {
                            // Determine the appropriate media type filter based on library type
                            let media_type = match lib_type.as_str() {
                                "movies" => Some("movie"),
                                "shows" => Some("show"),
                                "music" => Some("album"), // For music libraries, show albums, not individual tracks
                                _ => None,                // For unknown types, get all items
                            };

                            // Get ALL items for this library without pagination
                            if let Some(media_type) = media_type {
                                media_repo
                                    .find_by_library_and_type(library_id.as_ref(), media_type)
                                    .await
                            } else {
                                // For unknown types, get all items
                                media_repo.find_by_library(library_id.as_ref()).await
                            }
                        };

                        (Some(lib_type), media_result)
                    }
                    _ => {
                        // If we can't get library info, get all items
                        (None, media_repo.find_by_library(library_id.as_ref()).await)
                    }
                };

                match media_result {
                    Ok(mut items) => {
                        // For LastWatched sort, we need to fetch playback progress data
                        let playback_map = if matches!(sort_by, SortBy::LastWatched) {
                            let media_ids: Vec<String> =
                                items.iter().map(|item| item.id.clone()).collect();
                            match crate::services::core::MediaService::get_playback_progress_batch(
                                &db, &media_ids,
                            )
                            .await
                            {
                                Ok(map) => map,
                                Err(e) => {
                                    error!("Failed to fetch playback progress for sorting: {}", e);
                                    HashMap::new()
                                }
                            }
                        } else {
                            HashMap::new()
                        };

                        // Sort items based on sort criteria and order
                        match (sort_by, sort_order) {
                            (SortBy::Title, SortOrder::Ascending) => {
                                items.sort_by(|a, b| a.sort_title.cmp(&b.sort_title));
                            }
                            (SortBy::Title, SortOrder::Descending) => {
                                items.sort_by(|a, b| b.sort_title.cmp(&a.sort_title));
                            }
                            (SortBy::Year, SortOrder::Ascending) => {
                                items.sort_by(|a, b| a.year.cmp(&b.year));
                            }
                            (SortBy::Year, SortOrder::Descending) => {
                                items.sort_by(|a, b| b.year.cmp(&a.year));
                            }
                            (SortBy::DateAdded, SortOrder::Ascending) => {
                                items.sort_by(|a, b| a.added_at.cmp(&b.added_at));
                            }
                            (SortBy::DateAdded, SortOrder::Descending) => {
                                items.sort_by(|a, b| b.added_at.cmp(&a.added_at));
                            }
                            (SortBy::Rating, SortOrder::Ascending) => {
                                items.sort_by(|a, b| {
                                    a.rating
                                        .partial_cmp(&b.rating)
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                });
                            }
                            (SortBy::Rating, SortOrder::Descending) => {
                                items.sort_by(|a, b| {
                                    b.rating
                                        .partial_cmp(&a.rating)
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                });
                            }
                            (SortBy::Duration, SortOrder::Ascending) => {
                                items.sort_by(|a, b| a.duration_ms.cmp(&b.duration_ms));
                            }
                            (SortBy::Duration, SortOrder::Descending) => {
                                items.sort_by(|a, b| b.duration_ms.cmp(&a.duration_ms));
                            }
                            (SortBy::LastWatched, SortOrder::Ascending) => {
                                items.sort_by(|a, b| {
                                    let a_time =
                                        playback_map.get(&a.id).and_then(|p| p.last_watched_at);
                                    let b_time =
                                        playback_map.get(&b.id).and_then(|p| p.last_watched_at);
                                    a_time.cmp(&b_time)
                                });
                            }
                            (SortBy::LastWatched, SortOrder::Descending) => {
                                items.sort_by(|a, b| {
                                    let a_time =
                                        playback_map.get(&a.id).and_then(|p| p.last_watched_at);
                                    let b_time =
                                        playback_map.get(&b.id).and_then(|p| p.last_watched_at);
                                    b_time.cmp(&a_time)
                                });
                            }
                        }

                        sender.input(LibraryPageInput::AllItemsLoaded {
                            items,
                            library_type,
                        });
                    }
                    Err(e) => {
                        error!("Failed to load library items: {}", e);
                        sender.input(LibraryPageInput::AllItemsLoaded {
                            items: Vec::new(),
                            library_type,
                        });
                    }
                }
            });
        }
    }

    /// Refresh the library view by clearing cache and reloading
    pub(super) fn refresh(&mut self, sender: AsyncComponentSender<Self>) {
        self.loaded_count = 0;
        self.total_items.clear();
        self.has_loaded_all = false;
        self.needs_factory_clear = true;
        // Cancel pending images BEFORE clearing image_requests
        // Otherwise cancel_pending_images() has no requests to cancel
        self.cancel_pending_images();
        self.image_requests.clear();
        self.images_requested.clear();
        self.visible_start_idx = 0;
        self.visible_end_idx = 0;
        // Keep genre filters during refresh to maintain user selection
        self.load_all_items(sender);
    }

    /// Update the visible range based on scroll position
    pub(super) fn update_visible_range(&mut self, root: &gtk::Overlay) {
        // Get the Box from the overlay, then navigate to the scrolled window
        // Widget tree: Overlay -> Box (main) -> Box (content area) -> ScrolledWindow
        let box_widget = root
            .first_child()
            .and_then(|w| w.downcast::<gtk::Box>().ok());

        let scrolled = box_widget
            .and_then(|b| b.last_child()) // Get the main content area Box
            .and_then(|w| w.first_child()) // Get the ScrolledWindow inside it
            .and_then(|w| w.downcast::<gtk::ScrolledWindow>().ok());

        if let Some(scrolled) = scrolled {
            let adjustment = scrolled.vadjustment();
            let scroll_pos = adjustment.value();
            let page_size = adjustment.page_size();

            // Get the flow box to determine actual item dimensions
            let flow_box = scrolled
                .child()
                .and_then(|w| w.first_child())
                .and_then(|w| w.downcast::<gtk::FlowBox>().ok());

            let items_per_row = if let Some(flow_box) = flow_box {
                // Use actual columns from flowbox
                flow_box.min_children_per_line() as usize
            } else {
                4 // Default fallback
            };

            // More accurate row height accounting for reduced spacing
            let row_height = 270.0; // Card height (180) + spacing (16)

            let visible_start_row = (scroll_pos / row_height).floor() as usize;
            let visible_end_row = ((scroll_pos + page_size) / row_height).ceil() as usize + 1; // Add 1 for partial visibility

            self.visible_start_idx = visible_start_row * items_per_row;
            self.visible_end_idx = ((visible_end_row + 1) * items_per_row).min(self.loaded_count);

            trace!(
                "Viewport updated: scroll_pos={:.0}, page_size={:.0}, items {} to {} visible",
                scroll_pos, page_size, self.visible_start_idx, self.visible_end_idx
            );
        }
    }

    /// Load images for items currently in the visible viewport
    pub(super) fn load_images_for_visible_range(&mut self) {
        // Calculate which items need images with lookahead
        let lookahead_items = 30; // Load 30 items ahead and behind for smoother scrolling
        let load_start = self.visible_start_idx.saturating_sub(lookahead_items);
        let load_end = (self.visible_end_idx + lookahead_items).min(self.loaded_count);

        debug!(
            "Loading images for items {} to {} (visible: {} to {})",
            load_start, load_end, self.visible_start_idx, self.visible_end_idx
        );

        // Cancel images outside visible range
        let mut to_cancel = Vec::new();
        for idx in 0..self.loaded_count {
            if (idx < load_start || idx >= load_end) && idx < self.total_items.len() {
                let item_id = &self.total_items[idx].id;
                if self.image_requests.contains_key(item_id) {
                    to_cancel.push(item_id.clone());
                }
            }
        }

        // Cancel out-of-range images
        for id in to_cancel {
            trace!("Cancelling image load for out-of-range item: {}", id);
            let _ = self
                .image_loader
                .sender()
                .send(ImageLoaderInput::CancelLoad { id: id.clone() });
            self.pending_image_cancels.push(id);
        }

        // Load images for items in range
        let mut images_queued = 0;
        for idx in load_start..load_end {
            if idx < self.total_items.len() {
                let item = &self.total_items[idx];
                if let Some(poster_url) = &item.poster_url {
                    let id = item.id.clone();

                    // Skip if already requested or recently cancelled
                    if self.images_requested.contains(&id)
                        || self.pending_image_cancels.contains(&id)
                    {
                        continue;
                    }

                    // Calculate priority based on distance from current viewport
                    let priority = if idx >= self.visible_start_idx && idx < self.visible_end_idx {
                        0 // Highest priority for visible items
                    } else {
                        // Priority increases with distance from viewport
                        let distance = if idx < self.visible_start_idx {
                            self.visible_start_idx - idx
                        } else {
                            idx - self.visible_end_idx
                        };
                        (distance / 10).min(10) as u8
                    };

                    trace!(
                        "Queueing image for item {} (id: {}) with priority {}",
                        idx, id, priority
                    );

                    let _ = self.image_loader.sender().send(ImageLoaderInput::LoadImage(
                        crate::workers::ImageRequest {
                            id: id.clone(),
                            url: poster_url.clone(),
                            size: crate::workers::ImageSize::Thumbnail,
                            priority,
                        },
                    ));

                    // Mark this image as requested
                    self.images_requested.insert(id);
                    images_queued += 1;
                }
            }
        }

        if images_queued > 0 {
            debug!("Queued {} new image loads", images_queued);
        }

        // Clear pending cancels after a delay
        self.pending_image_cancels.clear();
    }

    /// Cancel all pending image load requests
    pub(super) fn cancel_pending_images(&mut self) {
        // Cancel all pending image loads
        for (id, _) in self.image_requests.iter() {
            let _ = self
                .image_loader
                .sender()
                .send(ImageLoaderInput::CancelLoad { id: id.clone() });
        }
    }
}
