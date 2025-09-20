---
id: task-022
title: Refine player OSD to remove glassmorphism for Infuse-like appearance
status: Done
assignee:
  - '@claude'
created_date: '2025-09-15 03:30'
updated_date: '2025-09-15 03:34'
labels:
  - ui
  - player
  - relm4
dependencies: []
priority: high
---

## Description

The current player OSD uses heavy glassmorphism effects that don't match the clean, modern aesthetic of Infuse. Need to simplify the design with solid dark backgrounds, subtle shadows, and minimal transparency for a more professional appearance.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Remove glassmorphism/backdrop-filter effects from player controls
- [x] #2 Use solid dark backgrounds with subtle transparency (like Infuse)
- [x] #3 Simplify shadows to be more subtle and refined
- [x] #4 Reduce animation complexity while keeping smooth transitions
- [x] #5 Ensure controls remain clearly visible over video content
- [x] #6 Test appearance over both dark and light video backgrounds
<!-- AC:END -->

## Implementation Notes

Refined player OSD styling to remove glassmorphism effects and create a cleaner Infuse-like appearance.

## Changes Made:
- Removed all backdrop-filter blur effects from OSD elements
- Replaced gradient backgrounds with solid dark colors (rgba(20, 20, 22, 0.9))
- Simplified box shadows from multiple layers to single subtle shadows
- Reduced animation complexity - removed blur filters from animations
- Made hover states more subtle with smaller scale transforms
- Cleaned up icon shadows and improved contrast
- Faster, simpler fade animations (250ms vs 350ms)

## Technical Details:
- Edited src/platforms/relm4/styles/player.css
- Changed OSD base style from glassmorphism to solid backgrounds
- Updated all player control elements for consistency
- Removed webkit-backdrop-filter properties
- Simplified gradients to solid colors with transparency

The result is a cleaner, more professional appearance that matches Infuse's design philosophy while maintaining good visibility over video content.
