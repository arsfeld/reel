---
id: task-016
title: Port GTK player's modern Infuse-style UI design to Relm4
status: Done
assignee: []
created_date: '2025-09-15 02:12'
updated_date: '2025-09-16 04:34'
labels:
  - ui
  - player
  - relm4
  - design
dependencies: []
---

## Description

The GTK player implementation has a sophisticated, modern UI design similar to Infuse with beautiful OSD controls, overlays, and animations. The Relm4 player currently has a basic, unpolished UI. We need to port all the visual design elements, layouts, and styling from the GTK implementation including the Blueprint UI templates.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Port player.blp Blueprint template structure to Relm4 view macro
- [ ] #2 Implement all overlay layers (controls, top OSD, loading, error, auto-play)
- [ ] #3 Add modern glass-morphism/blur effects to OSD controls
- [ ] #4 Implement smooth opacity transitions and animations
- [ ] #5 Port skip intro/credits buttons with pill styling
- [ ] #6 Implement auto-play overlay with PiP preview and countdown
- [ ] #7 Add loading overlay with spinner and styled text
- [ ] #8 Port error overlay with retry functionality
- [ ] #9 Apply all CSS classes (osd, circular, pill, video-container)
- [ ] #10 Implement time display modes (duration, remaining, end time)
- [ ] #11 Add audio/subtitle track selection buttons
- [ ] #12 Ensure proper z-ordering of all overlay elements
<!-- AC:END -->
