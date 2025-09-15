---
id: task-028
title: Remove glassmorphism effect from Connect to Server button
status: Done
assignee:
  - '@claude'
created_date: '2025-09-15 15:03'
updated_date: '2025-09-15 15:12'
labels:
  - ui
  - styling
dependencies: []
---

## Description

The Connect to Server button currently uses a glassmorphism effect that should be removed for a cleaner, more standard appearance that better fits with the application's design system.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Glassmorphism styling removed from Connect to Server button
- [x] #2 Button uses standard styling consistent with other buttons in the app
<!-- AC:END -->


## Implementation Plan

1. Identify all Connect to Server buttons in sources.rs
2. Remove glassmorphism CSS classes from style.css
3. Update button styling to use standard GTK/Adwaita patterns
4. Test that buttons appear clean and consistent


## Implementation Notes

Removed glassmorphism effects from Connect to Server buttons in Relm4 UI:

## Changes Made:

1. **Fixed location**: Updated styles in the correct Relm4 app.rs file (not the deprecated GTK UI)
2. **Removed glass effects**: Eliminated gradients, translucent box-shadows, and inset highlights from `.pill.suggested-action` CSS rules
3. **Applied standard styling**: Replaced with clean GTK/Adwaita theme variables (@accent_bg_color and @accent_fg_color)
4. **Simplified hover states**: Removed complex shadow animations, kept only subtle background shade change

## Technical Details:
- Modified: src/platforms/relm4/app.rs (lines 291-300)
- Affected buttons: "Add Source", "Connect to Server", and all authentication dialog buttons using pill + suggested-action classes
- Buttons now follow standard system theme without distracting glass effects
