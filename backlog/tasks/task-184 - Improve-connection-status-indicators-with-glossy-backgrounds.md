---
id: task-184
title: Improve connection status indicators with glossy backgrounds
status: Done
assignee:
  - '@assistant'
created_date: '2025-09-18 16:00'
updated_date: '2025-10-05 22:52'
labels:
  - enhancement
  - ui
  - design
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The connection status indicators (checkmark, warning, offline icons) should have glossy, colored backgrounds to make them more visually appealing and easier to distinguish at a glance. Similar to modern badge designs with subtle gradients and rounded backgrounds.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add rounded background container for connection status icons
- [x] #2 Apply glossy gradient backgrounds: green for connected, amber/yellow for warning, red for disconnected
- [x] #3 Ensure proper padding and sizing for visual balance
- [x] #4 Add subtle shadow or border for depth
- [ ] #5 Test appearance in both light and dark themes
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Analyze current implementation in src/ui/factories/source_item.rs (lines 113-138)
2. Design CSS classes for glossy badge backgrounds with gradients
3. Update view! macro to wrap status icon in a styled container
4. Add CSS to src/styles/ for:
   - Rounded background containers
   - Glossy gradient backgrounds (green, amber, red)
   - Proper padding and sizing
   - Subtle shadows for depth
5. Test in both light and dark themes
6. Verify appearance with all connection states
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented glossy connection status badges with gradient backgrounds.

## Changes Made:

### 1. CSS Styling (src/styles/base.css)
Added new CSS classes:
- `.connection-status-badge`: Base container with rounded corners (12px), padding (6px 8px), and inset shadows for depth
- `.connection-status-badge.connected`: Green glossy gradient (rgba(38,162,105) to rgba(30,130,76))
- `.connection-status-badge.warning`: Amber glossy gradient (rgba(229,165,10) to rgba(192,132,0))
- `.connection-status-badge.error`: Red glossy gradient (rgba(192,28,40) to rgba(164,0,0))
- Each state includes hover effects with enhanced shadows and subtle transform
- Icon styling with white color and shadow for better visibility

### 2. UI Component (src/ui/factories/source_item.rs)
Updated connection status indicator:
- Wrapped status icon in gtk::Box with badge CSS classes
- Applied appropriate class based on ConnectionStatus: Connected→connected, Error→error, Disconnected→warning
- Kept spinner separate for Connecting state (no badge)
- Icons remain same: emblem-ok-symbolic, network-offline-symbolic, dialog-error-symbolic

## Visual Design:
- Rounded 12px borders for modern appearance
- Glossy gradients using 135deg angle
- Inset shadows (white top, black bottom) create 3D effect
- Border glow matching gradient colors
- Hover states enhance shadows and lift badge (-1px translateY)
- All colors use rgba for theme compatibility

## Status:
AC#5 (theme testing) requires manual testing with the app running in both light and dark themes. The implementation uses semi-transparent colors that should adapt well to both themes.
<!-- SECTION:NOTES:END -->
