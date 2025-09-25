---
id: task-235
title: Implement standalone build mode with self-update and desktop file creation
status: To Do
assignee: []
created_date: '2025-09-24 19:04'
labels:
  - feature
  - build
  - core
dependencies: []
priority: high
---

## Description

Create a standalone build mode where the binary can self-update and automatically create/manage its own desktop file for system integration. This mode should work independently of package managers and provide a simple installation experience.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Add standalone feature flag to Cargo.toml
- [ ] #2 Implement desktop file generation at runtime
- [ ] #3 Add logic to detect and create XDG desktop entry on first run
- [ ] #4 Integrate self-update functionality in standalone mode
- [ ] #5 Add standalone-specific configuration directory handling
- [ ] #6 Implement icon installation to appropriate XDG directories
- [ ] #7 Add command-line flag to enable/disable standalone features
- [ ] #8 Handle desktop file updates when app version changes
- [ ] #9 Add uninstall mechanism that removes desktop file and icons
- [ ] #10 Test standalone mode on different Linux distributions
- [ ] #11 Document standalone installation and usage in README
<!-- AC:END -->
