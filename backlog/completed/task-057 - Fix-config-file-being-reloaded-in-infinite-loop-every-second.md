---
id: task-057
title: Fix config file being reloaded in infinite loop every second
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 03:05'
updated_date: '2025-09-16 03:14'
labels:
  - bug
  - performance
  - config
dependencies: []
priority: high
---

## Description

The application is continuously reloading the config file approximately every second, creating an infinite loop. The logs show config.toml being loaded repeatedly with timestamps exactly 1 second apart. This causes unnecessary disk I/O, potential performance issues, and may indicate a timer or watcher that's incorrectly configured.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify what triggers the config reload every second
- [x] #2 Determine if a file watcher or timer is misconfigured
- [x] #3 Fix the root cause of the continuous reloading
- [x] #4 Ensure config is only reloaded when actually needed (file changes or user action)
- [x] #5 Remove or properly configure any unnecessary timers
- [x] #6 Verify config loads once at startup and only on actual changes
<!-- AC:END -->


## Implementation Plan

1. Identify all Config::load() calls in player.rs
2. Add config cache field to PlayerModel struct
3. Load config once during initialization
4. Use cached config values in UpdatePosition handler
5. Remove Config::load() from position update loop
6. Test that config only loads at startup and preference changes


## Implementation Notes

Fixed config file being reloaded every second by caching config values in PlayerPage struct.

Root cause: The player component had a 1-second timer that triggered UpdatePosition, which called Config::load() every time to get the progress update interval. This caused the config file to be read from disk every second during playback.

Solution implemented:
1. Added three cached config fields to PlayerPage struct:
   - config_auto_resume: bool
   - config_resume_threshold_seconds: u64
   - config_progress_update_interval_seconds: u64

2. Load config once during component initialization and cache the values

3. Updated all three Config::load() calls in player.rs to use cached values:
   - Line 1340: UpdatePosition handler now uses cached progress_update_interval
   - Line 798: LoadMedia handler now uses cached auto_resume and threshold values
   - Line 925: LoadMediaWithContext handler now uses cached auto_resume and threshold values

This eliminates unnecessary disk I/O and improves performance. Config is now only loaded at startup and when preferences are changed.
