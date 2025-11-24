---
id: task-468.05
title: Remove dead code - unused functions and methods
status: Done
assignee: []
created_date: '2025-11-24 19:57'
updated_date: '2025-11-24 20:18'
labels:
  - cleanup
dependencies: []
parent_task_id: task-468
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Remove or use the following dead code (unused functions/methods):

**Unused functions:**
- src/cache/chunk_downloader.rs:333 - `calculate_total_chunks`
- src/cache/proxy.rs:50 - `is_terminal`
- src/cache/proxy.rs:876 - `create_chunk_based_progressive_stream`
- src/cache/proxy.rs:962 - `create_progressive_stream`
- src/services/core/sync.rs:22 - `estimate_total_items`
- src/ui/pages/player/buffering_warnings.rs:144 - `is_buffering_stalled`
- src/ui/pages/player/error_retry.rs:106 - `cancel_retry`
- src/ui/pages/player/progress_tracker.rs:55,77 - `should_resume`, `get_progress_update_interval_seconds`
- src/ui/pages/player/volume.rs:71 - `sync_from_player`

**Unused methods from impl_id_type macro:**
- src/models/identifiers.rs:12,16 - `new` and `as_str` for BackendId, ProviderId, UserId (lines 70, 71, 75)

**Unused library filter methods:**
- src/ui/pages/library/filters.rs:56,85,96,107,115,236 - `get_active_filter_count`, `get_genre_label`, `get_year_label`, `get_rating_label`, `get_watch_status_label`, `get_filter_suggestions`

**Unused constants:**
- src/ui/pages/player/mod.rs:123 - `CONTROL_FADE_ANIMATION_MS`
- src/ui/pages/player/buffering_warnings.rs:9,10 - `SLOW_DOWNLOAD`, `CRITICALLY_LOW_BUFFER`
<!-- SECTION:DESCRIPTION:END -->
