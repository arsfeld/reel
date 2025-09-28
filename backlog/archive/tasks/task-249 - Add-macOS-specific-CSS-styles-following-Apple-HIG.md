---
id: task-249
title: Add macOS-specific CSS styles following Apple HIG
status: In Progress
assignee:
  - '@claude'
created_date: '2025-09-26 13:23'
updated_date: '2025-09-26 13:47'
labels:
  - ui
  - macos
  - platform-specific
dependencies: []
priority: high
---

## Description

Implement platform-specific CSS styles that follow the macOS Human Interface Guidelines when running on macOS. This should include native-looking controls, appropriate spacing, typography, and visual elements that match the macOS aesthetic while maintaining the core functionality of the application.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Detect macOS platform at runtime and conditionally apply styles
- [ ] #2 Create macOS-specific CSS module with Apple HIG-compliant styles
- [ ] #3 Implement native-looking window controls and title bar styling
- [ ] #4 Apply San Francisco font stack for macOS typography
- [ ] #5 Match macOS system colors and adapt to light/dark mode
- [ ] #6 Test styles on macOS to ensure proper appearance
- [ ] #7 Ensure Linux/GNOME styles remain unchanged when not on macOS
<!-- AC:END -->


## Implementation Plan

1. Research current styling structure and platform detection
2. Create macOS-specific CSS module structure
3. Implement platform detection mechanism in Rust
4. Create base macOS styles with San Francisco font and system colors
5. Implement window chrome and controls styling
6. Add dark/light mode support matching macOS preferences
7. Create conditional loading system for platform-specific styles
8. Test on macOS (if available) or verify styles match HIG


## Implementation Notes

## Implementation Status: FAILED - Multiple Issues

The implementation attempted to add macOS-specific styles but has numerous problems:

### Major Issues:

1. **Window Controls Completely Broken**: 
   - Window control buttons either revert to GTK defaults or are stretched/deformed
   - Colors not applying properly, showing as squares instead of circles
   - Size constraints not working despite multiple attempts with !important

2. **Header Bar Styling Issues**:
   - Header bar remains too large despite setting min-height
   - Padding and sizing not respecting CSS values

3. **CSS Specificity Problems**:
   - Platform-specific selectors not overriding GTK defaults consistently
   - Styles getting overridden by GTK's internal styles

4. **Design Problems**:
   - Initial attempts had too much depth/shadows (not flat enough for modern macOS)
   - Scrollbar theming caused weird rectangles
   - Popover backgrounds showing solid backgrounds instead of transparent

### Technical Problems:

- CSS selector specificity not strong enough to override GTK/libadwaita defaults
- The current version of libadwaita lacks proper native window control support
- Platform detection works but CSS application is inconsistent

### Files Modified:
- Created: `src/styles/macos.css` (needs complete rewrite)
- Created: `src/utils/platform.rs` (platform detection - works)
- Modified: `src/app/app.rs` (conditional CSS loading - works)
- Modified: `src/ui/main_window.rs` (applies platform classes)

### Next Steps Required:

1. Research GTK4/libadwaita CSS precedence and specificity rules
2. Find proper selectors for window controls that actually work
3. Consider programmatic styling instead of pure CSS
4. May need to wait for libadwaita 1.8+ for proper native window controls
5. Test on actual macOS hardware to verify what actually works

### Conclusion:

The task is NOT complete and has significant bugs. The current implementation does not meet any of the acceptance criteria properly. A different approach is needed, possibly involving more programmatic control rather than pure CSS.
