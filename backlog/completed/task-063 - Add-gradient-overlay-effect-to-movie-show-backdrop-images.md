---
id: task-063
title: Add gradient overlay effect to movie/show backdrop images
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 03:58'
updated_date: '2025-09-16 04:05'
labels:
  - ui
  - enhancement
  - ux
dependencies: []
priority: high
---

## Description

The backdrop images on movie and show details pages currently display without any overlay effect, making it difficult to read text overlaid on them. A gradient shadow or overlay effect is needed to ensure proper contrast between the backdrop and the content displayed on top of it.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Analyze current backdrop display implementation in movie_details.rs and show_details.rs
- [x] #2 Design gradient overlay that provides proper text contrast without obscuring the backdrop
- [x] #3 Implement CSS gradient overlay on backdrop images
- [x] #4 Ensure gradient works well with both light and dark content
- [x] #5 Test gradient effect with various backdrop images of different brightness levels
- [x] #6 Apply consistent gradient styling to both movie and show details pages
<!-- AC:END -->


## Implementation Plan

1. Analyze existing backdrop implementation in movie_details.rs and show_details.rs\n2. Identify where hero-gradient class is used in both pages\n3. Add CSS styles for hero-gradient with proper gradient values\n4. Add CSS for poster-shadow class for enhanced shadow effect\n5. Add CSS for metadata-pill class for better visual styling\n6. Test compilation and visual appearance


## Implementation Notes

## Implementation Summary\n\nAdded gradient overlay effect to movie and show backdrop images to improve text contrast and readability.\n\n### Changes Made:\n\n1. **Added hero-gradient CSS class** (app.rs:377-389)\n   - Linear gradient from transparent to near-black (98% opacity)\n   - Gradual opacity progression for smooth transition\n   - Ensures text remains readable across all backdrop images\n\n2. **Enhanced poster-shadow class** (app.rs:392-397)\n   - Multiple shadow layers for depth\n   - Strong shadow effect to make poster stand out from backdrop\n\n3. **Styled metadata-pill class** (app.rs:400-411)\n   - Semi-transparent background with backdrop blur\n   - Subtle border for definition\n   - Hover effect for interactivity\n\n### Technical Details:\n\n- Both movie_details.rs and show_details.rs already had hero-gradient class applied to overlay Box\n- Gradient uses percentage-based stops for smooth transitions\n- Dark theme forced (ForceDark) ensures consistent appearance\n- CSS applied globally via relm4::set_global_css\n\n### Result:\n\nThe gradient overlay now provides proper contrast between backdrop images and overlaid content, ensuring text remains readable regardless of backdrop brightness.
