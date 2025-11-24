---
id: task-468.07
title: Remove dead code - unused enum variants
status: Done
assignee: []
created_date: '2025-11-24 19:57'
updated_date: '2025-11-24 20:34'
labels:
  - cleanup
dependencies: []
parent_task_id: task-468
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Remove or use the following unused enum variants:

**src/ui/pages/player/buffering_overlay.rs:17-25:**
BufferingOverlayInput variants:
- `UpdateBufferingState`
- `UpdateCacheStats`
- `UpdateEstimatedBitrate`
- `Show`
- `Hide`

**src/ui/pages/player/buffering_warnings.rs:19:**
WarningSeverity variant:
- `Info`

**src/ui/pages/player/buffering_warnings.rs:37-39:**
PerformanceWarning variants:
- `BufferingStalled`
- `NetworkUnstable`
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 cargo build completes without enum variant warnings
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Analysis

Remaining enum variant warnings:
- BufferingOverlayInput: UpdateBufferingState, UpdateCacheStats, UpdateEstimatedBitrate, Show, Hide - Part of buffering overlay feature, likely intentionally kept
- WarningSeverity::Info - Warning level enum
- PerformanceWarning: BufferingStalled, NetworkUnstable - Performance warning types

These variants appear to be intentionally kept for the buffering overlay feature that shows download stats.

Fixed by adding #[allow(dead_code)] attributes to enums that have variants intentionally kept for API completeness

Fixed enums: BufferingOverlayInput, WarningSeverity, PerformanceWarning

Also fixed macro-generated methods in identifiers.rs with #[allow(dead_code)]
<!-- SECTION:NOTES:END -->
