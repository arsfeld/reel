---
id: task-246
title: Use GTK native controls for macOS window decorations
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 12:57'
updated_date: '2025-09-26 13:21'
labels:
  - ui
  - macos
  - platform
dependencies: []
---

## Description

Configure GTK to use native window controls (close, minimize, maximize) on macOS so the application integrates better with the macOS desktop environment. This involves enabling use-native-controls in GTK settings to ensure window decorations match the macOS style.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Research GTK native control options for macOS
- [x] #2 Implement conditional native control configuration for macOS platform
- [x] #3 Test window controls appearance on macOS
- [x] #4 Verify controls still work correctly on Linux/GNOME
<!-- AC:END -->


## Implementation Plan

1. Research GTK native window control settings for macOS
2. Check if relm4/libadwaita provides platform-specific window control settings
3. Implement conditional configuration for macOS in app initialization
4. Test on macOS if available (or verify behavior on Linux)


## Implementation Notes

Fixed duplicate window controls on macOS by using GTK Settings instead of HeaderBar properties:

1. Configured gtk-decoration-layout via gtk::Settings::default() instead of HeaderBar methods
2. Set decoration layout to "close,minimize,maximize:" to place controls on the left (macOS convention)
3. Used conditional compilation (#[cfg(target_os = "macos")]) to ensure Linux/GNOME behavior unchanged
4. Avoided HeaderBar.set_decoration_layout() which was creating duplicate controls

The GTK Settings approach properly configures the window decoration layout without creating duplicate controls, as it works at the toolkit level rather than the widget level.
