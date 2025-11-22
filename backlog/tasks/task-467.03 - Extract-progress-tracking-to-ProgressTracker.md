---
id: task-467.03
title: Extract progress tracking to ProgressTracker
status: Done
assignee: []
created_date: '2025-11-22 19:06'
updated_date: '2025-11-22 21:55'
labels:
  - refactoring
  - player
dependencies: []
parent_task_id: task-467
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract playback progress tracking and syncing from player mod.rs into a dedicated `progress_tracker.rs` module with a ProgressTracker struct.

State to extract:
- `last_progress_save: std::time::Instant`
- `config_auto_resume: bool`
- `config_resume_threshold_seconds: u64`
- `config_progress_update_interval_seconds: u64`

Logic to extract:
- Periodic progress save checks (every N seconds)
- Watch status determination (>90% = watched)
- Progress sync to database
- Watch status sync to backend (Plex/Jellyfin)
- PlayQueue progress updates
- Resume position calculation

ProgressTracker API:
- `new(config) -> Self`
- `should_save_progress(position, duration) -> bool`
- `save_progress(db, media_id, position, duration, player_state, context, sender)`
- `update_config(auto_resume, threshold, interval)`
- `reset_save_timer()`
- `should_resume(saved_position, duration) -> bool`
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Create src/ui/pages/player/progress_tracker.rs with ProgressTracker struct
- [ ] #2 Extract progress tracking state from PlayerPage
- [ ] #3 Move periodic save logic to tracker
- [ ] #4 Move watch status calculation to tracker
- [ ] #5 Code compiles without errors
- [ ] #6 Progress saves correctly at configured intervals and on completion
<!-- AC:END -->
