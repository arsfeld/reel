---
id: task-395
title: Fix AdaptiveQualityManager bandwidth reporting architecture
status: Done
assignee:
  - '@claude'
created_date: '2025-10-04 02:31'
updated_date: '2025-10-04 02:38'
labels:
  - adaptive-quality
  - refactoring
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The AdaptiveQualityManager's record_download() method cannot be called after run() is called because run() consumes self. This prevents CacheProxy from reporting bandwidth metrics to the manager.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add bandwidth update channel to AdaptiveQualityManager
- [x] #2 Modify AdaptiveQualityManager to receive bandwidth updates via channel in run() loop
- [x] #3 Update PlayerController to forward bandwidth reports through the channel
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add bandwidth update channel (sender/receiver) to AdaptiveQualityManager::new()
2. Modify AdaptiveQualityManager::run() to listen for bandwidth updates in the tokio::select! loop
3. Remove record_download() public method from AdaptiveQualityManager (no longer needed)
4. Update PlayerController::enable_adaptive_quality() to keep sender for bandwidth updates
5. Implement PlayerCommand::ReportChunkDownload to forward bandwidth reports via channel
6. Test bandwidth monitoring integration
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Fixed the AdaptiveQualityManager bandwidth reporting architecture by implementing a channel-based approach:

## Changes Made

1. **AdaptiveQualityManager** (src/player/adaptive_quality.rs):
   - Added bandwidth_rx channel parameter to new()
   - Modified run() to use tokio::select! for concurrent message handling
   - Changed record_download() from public to private handle_bandwidth_update()
   - Updated tests to pass bandwidth channel

2. **PlayerController** (src/player/controller.rs):
   - Added bandwidth_tx to AdaptiveQualityHandle
   - Created bandwidth channel in enable_adaptive_quality()
   - Implemented ReportChunkDownload command handler to forward bandwidth metrics
   - Added report_bandwidth() method to AdaptiveQualityHandle

3. **PlayerHandle** API:
   - report_chunk_download() method already exists and is ready to use

## Architecture

Bandwidth data flow:
CacheProxy → PlayerHandle::report_chunk_download() → PlayerCommand::ReportChunkDownload → AdaptiveQualityHandle → bandwidth channel → AdaptiveQualityManager::run()

## Status

The architectural blocker is resolved. The channel-based approach allows bandwidth metrics to be reported to the manager after it has been spawned.

AC#4 (CacheProxy integration) is now tracked in task-396 as it requires passing PlayerHandle through the cache layer.

Build status: ✓ Compiles successfully
Tests: Unit tests updated and passing
<!-- SECTION:NOTES:END -->
