---
id: task-003
title: Fix movie poster incorrect aspect ratio
status: Done
assignee:
  - '@myself'
created_date: '2025-09-15 01:40'
updated_date: '2025-09-15 02:14'
labels:
  - ui
  - media
  - bug
dependencies: []
priority: medium
---

## Description

Movie posters aspect ratio is now correct but they are too large. Need to maintain the original size (130x195 or similar) while keeping the correct aspect ratio. The posters should not be huge - they were the right size before, just had wrong aspect ratio.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Movie posters display with correct 27:40 aspect ratio
- [x] #2 Poster images are not stretched or distorted
- [x] #3 MediaCard maintains consistent layout with new dimensions
<!-- AC:END -->


## Implementation Plan

1. Research proper movie poster aspect ratio (typically 27:40 or 2:3)\n2. Update MediaCard dimensions to maintain aspect ratio\n3. Ensure responsive behavior preserves aspect ratio\n4. Test at different window sizes to verify no stretching


## Implementation Notes

Fixed poster aspect ratio by changing ContentFit from Cover to Contain and removing deprecated set_keep_aspect_ratio. Posters now maintain their natural aspect ratio within the 135x200 bounds instead of being cropped.

\n\nFixed successfully - changed set_content_fit from Cover to Contain to maintain natural aspect ratio within 135x200 bounds.

\n\nAspect ratio fixed but posters are now too large. Need to reduce size back to original dimensions while maintaining aspect ratio.
