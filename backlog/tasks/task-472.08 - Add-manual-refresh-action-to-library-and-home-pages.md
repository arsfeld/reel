---
id: task-472.08
title: Add manual refresh action to library and home pages
status: To Do
assignee: []
created_date: '2025-12-09 18:51'
labels:
  - ui
  - feature
dependencies: []
parent_task_id: task-472
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

Allow users to manually trigger a refresh when they want fresh data, regardless of TTL.

## Implementation

1. **Add refresh action to LibraryPage**:
   - Refresh button in header bar
   - Keyboard shortcut (Ctrl+R or F5)
   - Pull-to-refresh gesture (if applicable)

2. **Add refresh action to HomePage**:
   - Refresh button for individual sections
   - Global refresh all button

3. **Force refresh behavior**:
   - Bypass TTL check
   - Show loading indicator
   - Update `fetched_at` on completion

```rust
// In page message handler
Msg::RefreshRequested => {
    self.is_refreshing = true;
    sender.send(RefreshMessage::QueueLibraryRefresh {
        library_id: self.library_id.clone(),
        priority: RefreshPriority::High,
        force: true,  // Bypass TTL
    });
}
```

## Files to Modify
- `src/ui/pages/home.rs` - add refresh action
- `src/ui/pages/library/mod.rs` - add refresh action
- Potentially add to app keyboard shortcuts

## Acceptance Criteria
- [ ] Refresh button visible in library page header
- [ ] Refresh triggers immediate backend fetch
- [ ] Loading indicator shows during refresh
- [ ] Keyboard shortcut works
- [ ] Home page sections can be refreshed
<!-- SECTION:DESCRIPTION:END -->
