---
id: task-403
title: Add user preferences for skip intro/credits behavior
status: Done
assignee:
  - '@assistant'
created_date: '2025-10-05 22:17'
updated_date: '2025-10-05 22:42'
labels:
  - player
  - preferences
  - ui
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement user preferences to control skip intro/credits behavior, including auto-skip options and configurable detection thresholds. This completes task-167 AC#3 and AC#6.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add skip intro/credits settings to preferences UI
- [x] #2 Implement auto-skip intro option (skip automatically without showing button)
- [x] #3 Implement auto-skip credits option
- [x] #4 Add configurable threshold for marker detection (e.g., minimum marker duration)
- [x] #5 Persist preferences in config file
- [x] #6 Apply preferences when displaying skip buttons
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add skip intro/credits configuration fields to PlaybackConfig:
   - auto_skip_intro: bool
   - auto_skip_credits: bool
   - skip_intro_enabled: bool
   - skip_credits_enabled: bool
   - minimum_marker_duration_seconds: u32

2. Update preferences UI in src/ui/pages/preferences.rs:
   - Add "Playback Behavior" preferences group
   - Add switches for auto-skip options
   - Add switches for button visibility
   - Add spin button for minimum marker duration

3. Apply config values in player.rs:
   - Load config when initializing player
   - Check auto-skip flags when markers detected
   - Automatically seek if auto-skip enabled
   - Respect minimum duration threshold

4. Test:
   - Verify preferences save/load correctly
   - Test auto-skip functionality
   - Test minimum duration threshold
   - Test button visibility based on config
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented comprehensive user preferences for skip intro/credits behavior.

## Changes Made:

### 1. Config System (src/config.rs)
Added new fields to PlaybackConfig:
- skip_intro_enabled: bool (default: true) - Show skip intro button
- skip_credits_enabled: bool (default: true) - Show skip credits button  
- auto_skip_intro: bool (default: false) - Auto-skip intros
- auto_skip_credits: bool (default: false) - Auto-skip credits
- minimum_marker_duration_seconds: u32 (default: 5) - Min duration threshold

### 2. Preferences UI (src/ui/pages/preferences.rs)
Added new "Playback Behavior" preferences group with:
- Switch for showing skip intro button
- Switch for auto-skip intro
- Switch for showing skip credits button
- Switch for auto-skip credits
- Spin button for minimum marker duration (1-60 seconds)

All preferences are persisted to config file and loaded on startup.

### 3. Player Integration (src/ui/pages/player.rs)
Updated skip button logic:
- Added cached config fields to avoid repeated file reads
- Check config_skip_intro_enabled before showing intro button
- Check config_skip_credits_enabled before showing credits button
- Check minimum marker duration before showing buttons
- Auto-skip when auto_skip_intro/credits enabled
- Config updates via MessageBroker refresh cached values

## Behavior:
- Default: Show skip buttons, no auto-skip
- Auto-skip triggers once at marker start (within 1 second)
- Markers shorter than threshold are ignored
- Config changes apply immediately via MessageBroker
- All preferences persist across app restarts
<!-- SECTION:NOTES:END -->
