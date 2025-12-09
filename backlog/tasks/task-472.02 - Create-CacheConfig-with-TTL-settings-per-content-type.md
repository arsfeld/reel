---
id: task-472.02
title: Create CacheConfig with TTL settings per content type
status: Done
assignee: []
created_date: '2025-12-09 18:51'
updated_date: '2025-12-09 19:15'
labels:
  - configuration
dependencies: []
parent_task_id: task-472
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Create a centralized cache configuration that defines TTL durations for different content types. This replaces the unused `SyncStrategy` with something actually used.

## Implementation

1. **Create CacheConfig** in `src/services/core/cache_config.rs`:
```rust
pub struct CacheConfig {
    pub libraries_ttl: Duration,      // 1 hour
    pub media_items_ttl: Duration,    // 4 hours
    pub episodes_ttl: Duration,       // 12 hours
    pub full_metadata_ttl: Duration,  // 24 hours (cast/crew)
    pub home_sections_ttl: Duration,  // 30 minutes
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            libraries_ttl: Duration::from_secs(3600),
            media_items_ttl: Duration::from_secs(4 * 3600),
            episodes_ttl: Duration::from_secs(12 * 3600),
            full_metadata_ttl: Duration::from_secs(24 * 3600),
            home_sections_ttl: Duration::from_secs(1800),
        }
    }
}
```

2. **Add helper methods**:
   - `is_stale(fetched_at: DateTime, content_type: ContentType) -> bool`
   - `ttl_for(content_type: ContentType) -> Duration`

3. **Integrate with app state**:
   - Add `CacheConfig` to application initialization
   - Make available to services that need TTL checks

## Files to Create/Modify
- `src/services/core/cache_config.rs` (new)
- `src/services/core/mod.rs` - export CacheConfig
- `src/services/initialization.rs` - initialize config

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 CacheConfig struct with TTL per content type
- [x] #2 Default values are sensible (1h to 24h range)
- [x] #3 Helper method to check if content is stale
- [x] #4 Config is accessible from services
<!-- SECTION:DESCRIPTION:END -->

<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Completed: Created CacheConfig in src/services/core/cache_config.rs with TTL settings for libraries (1h), media_items (4h), episodes (12h), full_metadata (24h), and home_sections (30m). Added is_stale(), is_stale_naive(), ttl_for(), and age_secs() helper methods. Config is accessible via cache_config() global function or by instantiating CacheConfig directly.
<!-- SECTION:NOTES:END -->
