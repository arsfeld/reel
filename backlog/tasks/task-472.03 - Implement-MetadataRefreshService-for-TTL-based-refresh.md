---
id: task-472.03
title: Implement MetadataRefreshService for TTL-based refresh
status: Done
assignee: []
created_date: '2025-12-09 18:51'
updated_date: '2025-12-09 19:20'
labels:
  - service
dependencies: []
parent_task_id: task-472
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Create a new service that handles TTL-based metadata refresh logic. This service decides when to refresh and queues refresh requests.

## Implementation

1. **Create MetadataRefreshService** in `src/services/core/metadata_refresh.rs`:

```rust
pub struct MetadataRefreshService;

impl MetadataRefreshService {
    /// Check if item needs refresh based on TTL
    pub fn needs_refresh(
        fetched_at: Option<DateTime>,
        content_type: ContentType,
        config: &CacheConfig,
    ) -> bool;
    
    /// Queue a background refresh for a library
    pub async fn queue_library_refresh(
        library_id: &LibraryId,
        priority: RefreshPriority,
    ) -> Result<()>;
    
    /// Queue refresh for specific items
    pub async fn queue_items_refresh(
        item_ids: &[MediaItemId],
        priority: RefreshPriority,
    ) -> Result<()>;
    
    /// Refresh a single item's full metadata (cast/crew)
    pub async fn refresh_item_metadata(
        db: &DatabaseConnection,
        backend: &dyn MediaBackend,
        item_id: &MediaItemId,
    ) -> Result<MediaItem>;
}

pub enum RefreshPriority {
    High,    // User is viewing this content
    Normal,  // Background refresh
    Low,     // Prefetch
}
```

2. **Integrate with MessageBroker**:
   - Define `RefreshMessage` enum for refresh requests
   - Workers subscribe and process refresh queue

## Files to Create/Modify
- `src/services/core/metadata_refresh.rs` (new)
- `src/services/core/mod.rs` - export service
- `src/ui/shared/broker.rs` - add RefreshMessage

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Service can check if content needs refresh
- [x] #2 Can queue refresh requests with priority
- [x] #3 Integrates with MessageBroker for async processing
- [x] #4 Can refresh individual item's full metadata
<!-- SECTION:DESCRIPTION:END -->

<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Completed: Created MetadataRefreshService in src/services/core/metadata_refresh.rs with needs_refresh(), queue_library_refresh(), queue_items_refresh(), queue_item_metadata_refresh(), refresh_movie_metadata(), refresh_show_metadata(), and get_stale_items() methods. Added MetadataRefreshMessage and RefreshPriority to broker.rs for async processing.
<!-- SECTION:NOTES:END -->
