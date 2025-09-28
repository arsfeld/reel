---
id: task-289
title: Integrate file watcher for config hot-reload
status: Done
assignee:
  - '@claude'
created_date: '2025-09-28 01:07'
updated_date: '2025-09-28 01:20'
labels:
  - config
  - enhancement
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Complete the integration of the notify crate file watcher to automatically detect and reload configuration changes from disk. The ConfigManager worker component infrastructure is already in place but needs to be properly connected to the ConfigService.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 File watcher detects changes to config.toml file
- [x] #2 Config automatically reloads when file is modified externally
- [x] #3 Debouncing prevents multiple rapid reloads
- [x] #4 File watcher errors are handled gracefully
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Review existing ConfigManager worker implementation with file watcher
2. Modify ConfigService to integrate with ConfigManager worker
3. Initialize ConfigManager worker at app startup
4. Test file watcher detects changes and reloads config
5. Verify debouncing works for rapid changes
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Successfully integrated the notify crate file watcher for config hot-reload:

1. Modified ConfigService to simplify integration with ConfigManager
2. ConfigManager worker with file watcher is now initialized in MainWindow init
3. File watcher monitors ~/.config/reel/config.toml for changes
4. Changes trigger automatic config reload with debouncing (100ms)
5. Config updates are propagated through MessageBroker to all components
6. Added toast notification to inform users when config is reloaded

The implementation leverages the existing ConfigManager worker which was already well-implemented with file watching capabilities using the notify crate. The worker is now properly initialized at application startup and integrates with the global ConfigService.
<!-- SECTION:NOTES:END -->
