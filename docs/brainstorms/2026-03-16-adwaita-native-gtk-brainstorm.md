# Brainstorm: Make GTK App Fully Adwaita Native

**Date:** 2026-03-16
**Status:** Ready for planning

## What We're Building

Rearchitect the GTK app's navigation and layout to use native Adwaita patterns so that Reel looks and feels indistinguishable from a GNOME core app (like GNOME Music or Videos).

### Current State

The app already uses GTK4 + libadwaita with:
- `AdwApplicationWindow`, `AdwOverlaySplitView`, `AdwHeaderBar`, `AdwToolbarView`
- `AdwClamp`, `AdwStatusPage`, `AdwPreferencesPage/Group/Row`
- `GtkStack` for view switching (no animated transitions, no back navigation)
- 3 separate `GtkListBox` groups in the sidebar (plain rows, no icons)

### Target State

- `AdwNavigationSplitView` replaces `AdwOverlaySplitView` for the main layout
- `AdwNavigationView` in the content pane — all views are `AdwNavigationPage` instances
- Native push/pop transitions for drill-down (grid -> detail -> player)
- Back button in header bar via navigation view
- Player view is a navigation page (not a fullscreen overlay takeover)
- Sidebar rows have proper icons, section headers, and GNOME-style visual weight

## Why This Approach

**AdwNavigationSplitView + AdwNavigationView** (Approach 1) was chosen because:

- It's exactly what GNOME designed for sidebar + content navigation apps
- GNOME Music and Videos use this pattern
- Gets native animated page transitions, back navigation, and responsive collapse for free
- Most authentic result — makes Reel indistinguishable from a core GNOME app

**Rejected alternatives:**
- **Approach 2 (keep OverlaySplitView, add NavigationView for content):** OverlaySplitView is designed for mobile overlay patterns, not persistent two-pane layouts. Mixing it with NavigationView feels half-native.
- **Approach 3 (ViewStack + NavigationView hybrid):** Over-engineered for this use case. Sidebar + ViewStack is redundant.

## Key Decisions

1. **Navigation container:** `AdwNavigationSplitView` (replaces `AdwOverlaySplitView`)
2. **Content navigation:** `AdwNavigationView` with `AdwNavigationPage` per view
3. **View switching model:** Sidebar selection pushes pages onto the navigation view; drill-down (poster -> detail -> player) pushes additional pages
4. **Player integration:** Player is a navigation page with push/pop, not a fullscreen overlay takeover
5. **Sidebar styling:** Modernize with icons per row, proper section headers matching GNOME conventions
6. **Scope:** Focus on look & feel consistency — not responsive/adaptive or mobile layouts (those can come later)

## Scope

### In Scope
- Replace `AdwOverlaySplitView` with `AdwNavigationSplitView`
- Replace `GtkStack` content switching with `AdwNavigationView`
- Wrap each view in `AdwNavigationPage`
- Add native page transitions (push/pop) for drill-down navigation
- Add back button support via `AdwNavigationView`
- Modernize sidebar: icons, section headers, proper Adwaita row styling
- Player view as navigation page

### Out of Scope
- Responsive/adaptive layouts (AdwBreakpoint, phone/tablet support)
- Replacing poster grid with a different widget
- Replacing player controls with Adwaita equivalents
- Home view hero banner redesign
- CSS stylesheet addition
- Any backend/core changes

## Open Questions

None — all key decisions resolved during brainstorming.
