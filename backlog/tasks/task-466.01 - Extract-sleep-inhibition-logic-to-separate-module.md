---
id: task-466.01
title: Extract sleep inhibition logic to separate module
status: Done
assignee: []
created_date: '2025-11-22 18:33'
updated_date: '2025-11-22 18:42'
labels: []
dependencies:
  - task-466.07
parent_task_id: task-466
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract the sleep/screensaver inhibition functionality from player.rs into a dedicated `sleep_inhibition.rs` module. This is the simplest extraction and a good starting point.

Current location: Lines 445-475 in player.rs
Functions to extract:
- `setup_sleep_inhibition(&mut self)`
- `release_sleep_inhibition(&mut self)`

Required state fields:
- `inhibit_cookie: Option<u32>`
- Access to `window: adw::ApplicationWindow`
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 New file created: src/ui/pages/player/sleep_inhibition.rs
- [ ] #2 Sleep inhibition methods moved to new module with proper encapsulation
- [ ] #3 PlayerPage delegates to sleep inhibition module methods
- [ ] #4 Code compiles without errors
- [ ] #5 Sleep inhibition still works correctly during playback
<!-- AC:END -->
