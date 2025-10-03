---
id: task-393
title: Integrate AdaptiveQualityManager with PlayerController and CacheProxy
status: In Progress
assignee:
  - '@claude'
created_date: '2025-10-04 02:22'
updated_date: '2025-10-04 02:31'
labels:
  - transcoding
  - adaptive-quality
  - integration
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Wire up the AdaptiveQualityManager component into the player architecture to enable automatic quality switching based on network conditions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 PlayerController instantiates AdaptiveQualityManager when playback starts
- [x] #2 Player state changes are forwarded to AdaptiveQualityManager
- [ ] #3 CacheProxy reports chunk download metrics to AdaptiveQualityManager
- [ ] #4 Quality decisions from manager trigger actual quality changes
- [ ] #5 Adaptive mode preference is loaded from config on startup
- [ ] #6 UI receives bandwidth/quality status updates for display
- [ ] #7 Manual quality changes disable auto mode
- [ ] #8 Tests verify state transitions and quality switching
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Analyze existing code structure and dependencies
2. Add AdaptiveQualityManager fields to PlayerController
3. Create state broadcasting channel in Player backends
4. Instantiate AdaptiveQualityManager when playback starts
5. Forward player state changes to manager
6. Add bandwidth reporting hook in CacheProxy
7. Handle quality decisions from manager
8. Add adaptive mode preference loading
9. Add UI status update mechanism
10. Handle manual quality changes
11. Add comprehensive tests
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Completed PlayerController integration with AdaptiveQualityManager:

## Implemented
1. Added AdaptiveQualityHandle struct to manage communication with spawned manager
2. Added PlayerCommand variants for adaptive quality control:
   - EnableAdaptiveQuality
   - DisableAdaptiveQuality
   - SetAdaptiveMode
   - SetQuality
   - ReportChunkDownload
3. PlayerController now:
   - Spawns AdaptiveQualityManager when enabled
   - Broadcasts player state changes to manager
   - Receives quality decisions from manager
   - Tracks state changes in event loop
4. PlayerHandle exposes public API for adaptive quality control

## Architecture Issue - Bandwidth Monitoring
The AdaptiveQualityManager's record_download() method cannot be called after the manager is spawned (run() consumes self). This prevents CacheProxy from reporting bandwidth metrics.

Options to fix:
1. Modify AdaptiveQualityManager to accept bandwidth updates via channel
2. Use Arc<Mutex<>> or Arc<RwLock<>> to share manager (requires changing run() signature)

For now, AC #3 is blocked pending architecture decision.

## Remaining TODOs
- AC #3: Fix bandwidth reporting architecture
- AC #4: Implement actual quality change when decisions arrive
- AC #5: Load adaptive mode preference from config
- AC #6: Add UI status update mechanism
- AC #7: Implement manual quality change logic
- AC #8: Add comprehensive tests

## Follow-up Task
Created task-395 to fix the bandwidth monitoring architecture using a channel-based approach.
<!-- SECTION:NOTES:END -->
