---
id: task-472.07
title: Add refresh indicators to UI components
status: To Do
assignee: []
created_date: '2025-12-09 18:51'
labels:
  - ui
  - ux
dependencies: []
parent_task_id: task-472
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Show subtle indicators when content is being refreshed in the background, without blocking the UI.

## Implementation

1. **Add refresh state to pages**:
```rust
struct LibraryPage {
    // ...existing fields
    is_refreshing: bool,
    last_refreshed: Option<DateTime>,
}
```

2. **Subtle refresh indicator**:
   - Small spinner in header/toolbar area
   - NOT a full-page loading state
   - Tooltip showing "Refreshing..." or "Last updated: X ago"

3. **Update indicators**:
   - Set `is_refreshing = true` when refresh queued
   - Set `is_refreshing = false` when `RefreshCompleted` received
   - Update `last_refreshed` timestamp

4. **Optional: Stale data indicator**:
   - Subtle badge/icon when showing data older than 2x TTL
   - "Content may be outdated" tooltip
   - Manual refresh button

## UI Components
- Header bar refresh spinner
- "Last updated" timestamp in footer/status
- Optional stale data warning

## Files to Modify
- `src/ui/pages/home.rs`
- `src/ui/pages/library/mod.rs`
- `src/styles/` - add refresh indicator styles

## Acceptance Criteria
- [ ] Refresh spinner shows during background refresh
- [ ] Last updated timestamp is displayed
- [ ] Indicators are subtle, not blocking
- [ ] Optional stale warning for very old data
<!-- SECTION:DESCRIPTION:END -->
