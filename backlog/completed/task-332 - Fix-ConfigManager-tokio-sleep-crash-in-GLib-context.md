---
id: task-332
title: 'Fix ConfigManager tokio::sleep crash in GLib context'
status: Done
assignee:
  - '@claude'
created_date: '2025-10-02 18:33'
updated_date: '2025-10-02 18:41'
labels:
  - bug
  - worker
  - crash
dependencies: []
priority: high
---

## Description

The ConfigManager worker crashes with 'there is no reactor running' error when calling tokio::time::sleep. This happens because Relm4 Workers run in GLib's main context, not a Tokio runtime. The previous fix (task-167) incorrectly replaced GLib timeout with tokio sleep, causing this crash at src/workers/config_manager.rs:234.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 ConfigManager no longer crashes with 'no reactor running' error
- [x] #2 Debouncing mechanism works correctly using GLib-compatible approach
- [x] #3 Application starts without panicking in ConfigManager worker
<!-- AC:END -->


## Implementation Plan

1. Analyze the current implementation - tokio::time::sleep at line 234 fails because there's no Tokio reactor in the GLib context
2. Review how other parts of the codebase (player.rs, main_window.rs) handle debouncing in GLib contexts
3. Replace tokio::time::sleep with glib::timeout_add_local_once for GLib-compatible debouncing
4. Test that config file changes are debounced correctly without crashes
5. Verify application starts without panicking


## Implementation Notes

Fixed ConfigManager crash by implementing timestamp-based debouncing that works in worker thread context.


## Changes
- Added last_reload_time field to ConfigManager struct using Arc<Mutex<Option<Instant>>>
- Replaced tokio::time::sleep with timestamp comparison logic
- Removed gtk import (no longer needed)
- Debouncing now checks time elapsed since last reload (100ms threshold)

## Why
The ConfigManager worker runs in a separate thread with GLib main context. Both tokio::time::sleep (requires Tokio reactor) and glib::timeout_add_local_once (requires main context ownership) fail in this context because:
1. tokio::time::sleep needs a Tokio runtime which isn't available in the worker\n2. glib::timeout_add_local_once tries to acquire the main context already owned by another thread\n\n## Solution\nUsed simple timestamp-based debouncing that:\n- Tracks last reload time with Arc<Mutex<Option<Instant>>>\n- Compares current time with last reload on each ReloadConfig message\n- Only triggers reload if 100ms has passed since last reload\n- No runtime dependencies - just timestamp comparison\n\nModified file: src/workers/config_manager.rs\nBuild: ✅ Success (0 errors, 169 warnings)
