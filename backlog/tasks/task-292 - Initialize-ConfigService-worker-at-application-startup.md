---
id: task-292
title: Initialize ConfigService worker at application startup
status: Done
assignee:
  - '@claude'
created_date: '2025-09-28 01:08'
updated_date: '2025-09-28 01:39'
labels:
  - config
  - initialization
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Properly initialize the ConfigService and its worker component during application startup to ensure the config system is ready before other components need it. This includes setting up the file watcher and loading initial configuration.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 ConfigService is initialized in app startup sequence
- [x] #2 ConfigManager worker is started and connected
- [x] #3 Initial config is loaded before UI components initialize
- [x] #4 Config service is accessible globally throughout the app
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Move ConfigService initialization to app startup before UI components
2. Ensure initial config is loaded from disk during service initialization
3. Start ConfigManager worker early in MainWindow init
4. Verify config is accessible globally via the static CONFIG_SERVICE
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented proper ConfigService initialization at application startup:

1. **Early Initialization**: Moved ConfigService initialization to app::run() before any UI components are created
2. **Synchronous Config Loading**: Added synchronous loading of initial config using block_on to ensure configuration is ready before UI initialization
3. **ConfigManager Worker**: Verified that ConfigManager worker is properly started in MainWindow::init() to watch for config file changes
4. **Global Access**: ConfigService remains globally accessible via the static CONFIG_SERVICE instance using once_cell::Lazy

The service now initializes eagerly at app startup, loads the initial configuration synchronously, and logs the loaded player backend for debugging. The ConfigManager worker continues to be started in MainWindow to handle runtime config file changes.
<!-- SECTION:NOTES:END -->
