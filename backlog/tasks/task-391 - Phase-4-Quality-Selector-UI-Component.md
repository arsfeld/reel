---
id: task-391
title: 'Phase 4: Quality Selector UI Component'
status: Done
assignee:
  - '@claude'
created_date: '2025-10-04 01:12'
updated_date: '2025-10-04 01:24'
labels:
  - ui
  - relm4
  - transcoding
  - phase-4
dependencies:
  - task-390
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create UI component for quality selection in player controls. Part of Plex transcoding integration (Phase 4 of 8). See docs/transcode-plan.md for complete implementation plan.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create QualitySelector Relm4 component in src/ui/shared/quality_selector.rs
- [x] #2 Implement dropdown widget with quality options list
- [x] #3 Add quality info label showing resolution and bitrate
- [x] #4 Handle quality change events and emit QualitySelectorOutput
- [x] #5 Export QualitySelector in src/ui/shared/mod.rs
- [x] #6 Integrate component into PlayerPage controls overlay
- [x] #7 Quality dropdown shows all available options from StreamInfo
- [x] #8 Current quality displayed with resolution and bitrate
- [x] #9 Quality change event propagates to PlayerPage
- [x] #10 UI updates when new media loads
- [x] #11 Component follows Relm4 Component pattern with proper Init/Input/Output
- [x] #12 Files created/updated as per docs/transcode-plan.md Phase 4

- [x] #13 Create task for Phase 5: MediaService Integration
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Review models (QualityOption, StreamInfo, Resolution) - already exist
2. Create QualitySelector component in src/ui/shared/quality_selector.rs
3. Export QualitySelector in src/ui/shared/mod.rs
4. Integrate into PlayerPage controls overlay
5. Test UI component rendering and events
6. Create follow-up task for Phase 5
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Created QualitySelector Relm4 component for quality selection in player controls.

Implemented:
- QualitySelector component in src/ui/shared/quality_selector.rs with dropdown and quality info display
- Exported component in src/ui/shared/mod.rs
- Integrated into PlayerPage controls overlay using local_ref pattern
- Added StreamInfoLoaded and QualityChanged messages to PlayerInput
- Quality selector updates when media loads with available options
- Quality change events propagate to PlayerPage for handling

Files modified:
- src/ui/shared/quality_selector.rs (new)
- src/ui/shared/mod.rs
- src/ui/pages/player.rs

The component is ready for Phase 5 integration with MediaService.
<!-- SECTION:NOTES:END -->
