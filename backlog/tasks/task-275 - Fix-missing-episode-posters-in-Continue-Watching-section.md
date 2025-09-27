---
id: task-275
title: Fix missing episode posters in Continue Watching section
status: Done
assignee:
  - '@assistant'
created_date: '2025-09-27 00:46'
updated_date: '2025-09-27 00:59'
labels:
  - ui
  - homepage
  - images
dependencies: []
priority: high
---

## Description

Episode items in the Continue Watching section are not displaying their poster images, while the same episodes show posters correctly in other sections like On Deck or Recently Added. This creates an inconsistent and poor user experience.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify why episode posters are missing only in Continue Watching section
- [x] #2 Compare the data/logic differences between Continue Watching and other sections
- [x] #3 Fix the poster loading for episodes in Continue Watching
- [x] #4 Verify episodes show correct show posters in Continue Watching
- [x] #5 Test with multiple different episodes to ensure consistency
<!-- AC:END -->


## Implementation Plan

1. Investigate Continue Watching section data handling in home.rs
2. Compare episode data structure between Continue Watching and other sections
3. Check if episodes are missing thumb/posterUrl fields in Continue Watching
4. Implement fix to ensure episodes use show posters in Continue Watching
5. Test with running application to verify poster display


## Implementation Notes

Found and fixed the root cause: When the same episode appeared in multiple sections (e.g., Continue Watching and On Deck), the image request tracking was using only the item ID as the HashMap key. This caused the second section to overwrite the first section's tracking entry, so images would only load for the last section that requested them.

Fixed by creating unique tracking keys using format "section_id::item_id", allowing each section to independently track and load images for the same items appearing in multiple sections.
