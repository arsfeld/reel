use tracing::debug;

use crate::models::{LibraryId, MediaItem, MediaItemId, SourceId};

/// Messages for media-related updates
#[derive(Debug, Clone)]
pub enum MediaMessage {
    /// Library added or updated
    LibraryUpdated {
        source_id: SourceId,
        library_id: LibraryId,
        item_count: usize,
    },
    /// Media item added or updated
    ItemUpdated {
        source_id: SourceId,
        library_id: LibraryId,
        item: MediaItem,
    },
    /// Multiple items updated (bulk operation)
    ItemsBulkUpdated {
        source_id: SourceId,
        library_id: LibraryId,
        count: usize,
    },
    /// Item removed
    ItemRemoved {
        source_id: SourceId,
        library_id: LibraryId,
        item_id: MediaItemId,
    },
    /// Library cleared
    LibraryCleared {
        source_id: SourceId,
        library_id: LibraryId,
    },
    /// Source cleared (all libraries and items)
    SourceCleared { source_id: SourceId },
}

// For now, we'll use these message types directly in components
// Components can create their own relm4::Sender/Receiver channels as needed
// This is simpler than trying to create a global broker that may not match Relm4's patterns

/// Convenience functions for logging media operations
pub fn log_library_updated(source_id: SourceId, library_id: LibraryId, item_count: usize) {
    debug!(
        "Library updated: source={}, library={}, items={}",
        source_id, library_id, item_count
    );
}

pub fn log_item_updated(source_id: SourceId, library_id: LibraryId, item: &MediaItem) {
    debug!(
        "Item updated: source={}, library={}, item={}",
        source_id,
        library_id,
        item.id()
    );
}

pub fn log_items_bulk_updated(source_id: SourceId, library_id: LibraryId, count: usize) {
    debug!(
        "Items bulk updated: source={}, library={}, count={}",
        source_id, library_id, count
    );
}

pub fn log_item_removed(source_id: SourceId, library_id: LibraryId, item_id: MediaItemId) {
    debug!(
        "Item removed: source={}, library={}, item={}",
        source_id, library_id, item_id
    );
}

pub fn log_library_cleared(source_id: SourceId, library_id: LibraryId) {
    debug!(
        "Library cleared: source={}, library={}",
        source_id, library_id
    );
}

pub fn log_source_cleared(source_id: SourceId) {
    debug!("Source cleared: source={}", source_id);
}
