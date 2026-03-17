---
title: "feat: Adwaita Native Navigation and Sidebar"
type: feat
status: completed
date: 2026-03-16
origin: docs/brainstorms/2026-03-16-adwaita-native-gtk-brainstorm.md
---

# feat: Adwaita Native Navigation and Sidebar

## Overview

Replace the current `AdwOverlaySplitView` + `GtkStack` navigation architecture with `AdwNavigationSplitView` + `AdwNavigationView` to make Reel look and feel like a native GNOME app (Music/Videos style). Modernize the sidebar with icons and section headers.

## Problem Statement

The current GTK app uses `AdwOverlaySplitView` (designed for mobile overlay patterns) with a `GtkStack` that switches views by name with flat crossfade transitions. There is no back navigation — Escape always goes to Home. Drill-down from grid to detail to player has no native page transitions or back buttons. The sidebar uses 3 plain `GtkListBox` groups without icons. The app works but does not feel like a native GNOME application.

## Proposed Solution

Adopt the standard GNOME sidebar+content navigation pattern (see brainstorm: `docs/brainstorms/2026-03-16-adwaita-native-gtk-brainstorm.md`):

1. **`AdwNavigationSplitView`** — replaces `AdwOverlaySplitView` as the main container
2. **`AdwNavigationView`** — replaces `GtkStack` in the content pane, providing a page stack with push/pop transitions
3. **`AdwNavigationPage`** — wraps each view, giving native titles, back buttons, and lifecycle signals
4. **Modernized sidebar** — icons per row, section headers, proper GNOME styling

## Technical Approach

### Architecture

```
AdwApplicationWindow
  └── AdwNavigationSplitView
        ├── sidebar: AdwNavigationPage("Reel")
        │     └── AdwToolbarView
        │           ├── AdwHeaderBar
        │           └── GtkScrolledWindow
        │                 └── GtkListBox (single, with section headers)
        │                       ├── [Section: Library]
        │                       │   ├── Row: Home (icon: go-home-symbolic)
        │                       │   ├── Row: Movies (icon: camera-video-symbolic)
        │                       │   ├── Row: TV Shows (icon: tv-symbolic)
        │                       │   └── Row: Other (icon: folder-symbolic)
        │                       ├── [Section: Personal]
        │                       │   ├── Row: Favorites (icon: starred-symbolic)
        │                       │   ├── Row: Collections (icon: view-list-symbolic)
        │                       │   └── Row: Files (icon: folder-open-symbolic)
        │                       └── [Section: System]
        │                           ├── Row: Downloads (icon: folder-download-symbolic)
        │                           └── Row: Settings (icon: emblem-system-symbolic)
        │
        └── content: AdwNavigationPage("Home")
              └── AdwNavigationView (nav_view)
                    └── [page stack — pushed/popped dynamically]
                        ├── Top-level: HomeView / MoviesView / etc.
                        ├── Drill-down: DetailView (pushed on poster click)
                        └── Drill-down: PlayerView (pushed on play)
```

### Key Architectural Decisions

**1. Sidebar clicks use `replace()`, not `push()`**
When the user clicks a sidebar item, the navigation stack is replaced with a single top-level page. This prevents unbounded history growth and matches GNOME Files/Music behavior. Back buttons only appear for drill-down pages (detail, player), never for top-level sidebar views.

**2. Views are singletons, created once at startup**
All view structs (HomeView, MoviesView, DetailView, etc.) are created once during `buildSidebarLayout()` and stored in `AppState`. Their root widgets are wrapped in `AdwNavigationPage` instances that are reused across navigations. This preserves the existing global pointer pattern (`global_detail`, `global_controls`, etc.) and avoids expensive re-creation of the GtkGLArea/mpv render context.

Since `AdwNavigationView` does not allow pushing a page that is already in the stack, top-level pages must be removed before re-pushing. The `replace()` method handles this cleanly — it replaces the entire stack with the given page array.

**3. Player page is a navigation page, playback stops on pop**
The player is pushed onto the navigation stack like any other drill-down page. When popped (back button or sidebar switch), playback stops and the mpv player is sent an idle command. The VideoArea's GL context persists because the widget is a singleton (never destroyed).

**4. Data refresh via `showing` signal**
Each `AdwNavigationPage` emits `showing` when it becomes the visible page (on push or when a page above it is popped). Connect view `refresh()` methods to this signal. This naturally handles:
- First visit: data loads on push
- Return visit: data refreshes when popped back to
- Sidebar re-navigation: data refreshes on `replace()`

**5. Downloads timer lifecycle**
Connect to `showing`/`hiding` signals on the downloads page. Start the 500ms poll timer on `showing`, stop it on `hiding`. This prevents wasted cycles and avoids modifying widgets that are not in the active navigation stack.

**6. Fullscreen hides the entire NavigationSplitView sidebar**
When entering fullscreen during playback, set `adw_navigation_split_view_set_show_content(split_view, TRUE)` and force collapsed state. On exit, restore. This is equivalent to what `AdwOverlaySplitView.set_collapsed()` did but uses the correct API.

**7. Content header bar is per-page**
Each `AdwNavigationPage` gets its own `AdwToolbarView` + `AdwHeaderBar`. The `AdwNavigationView` automatically manages back buttons based on stack depth. The sidebar toggle button moves to the sidebar's header bar (or is removed — `AdwNavigationSplitView` handles collapse natively).

**8. Page titles**
- Top-level pages: match sidebar label ("Home", "Movies", "TV Shows", etc.)
- Detail page: media item title (e.g., "The Matrix")
- Player page: "Now Playing"

### Navigation Flow Summary

| Action | Method | Stack After |
|--------|--------|-------------|
| App startup | initial `replace([home_page])` | `[home]` |
| Sidebar: click Movies | `replace([movies_page])` | `[movies]` |
| Click poster | `push(detail_page)` | `[movies, detail]` |
| Click Play | `push(player_page)` | `[movies, detail, player]` |
| Back from player | `pop()` | `[movies, detail]` |
| Back from detail | `pop()` | `[movies]` |
| Sidebar: click Settings (from detail) | `replace([settings_page])` | `[settings]` |
| Keyboard Escape | `pop()` (no-op if stack depth 1) | depends |
| Keyboard 1-8 | `replace([page])` + update sidebar selection | `[page]` |

### Implementation Phases

#### Phase 1: Replace Split View Container

**Goal:** Swap `AdwOverlaySplitView` for `AdwNavigationSplitView` with the sidebar and content panes wrapped in `AdwNavigationPage`. Keep `GtkStack` inside the content page for now — the app should look and behave the same as before, but using the correct container widget.

**Files changed:**
- `src/apprt/gtk/app.zig` — `buildSidebarLayout()`: replace `adw_overlay_split_view_new()` with `adw_navigation_split_view_new()`, wrap sidebar and content in `AdwNavigationPage`, update property bindings

**Success criteria:**
- [x] App builds and launches
- [x] Sidebar visible, content area shows views
- [x] Sidebar toggle removed (NavigationSplitView handles collapse natively)
- [x] Fullscreen toggle updated for NavigationSplitView API

**Key API changes:**
```
// Old
adw_overlay_split_view_new()
adw_overlay_split_view_set_sidebar(split, sidebar_widget)
adw_overlay_split_view_set_content(split, content_widget)
adw_overlay_split_view_set_collapsed(split, collapsed)

// New
adw_navigation_split_view_new()
adw_navigation_split_view_set_sidebar(split, sidebar_nav_page)
adw_navigation_split_view_set_content(split, content_nav_page)
// collapsed is read-only, driven by min-sidebar-width
```

---

#### Phase 2: Replace GtkStack with AdwNavigationView

**Goal:** Replace the `GtkStack` content switching with `AdwNavigationView`. Each view gets its own `AdwNavigationPage` with a title and `AdwToolbarView` + `AdwHeaderBar`. Sidebar clicks call `replace()`. Keyboard shortcuts updated.

**Files changed:**
- `src/apprt/gtk/app.zig` — remove GtkStack creation, create AdwNavigationView, wrap each view in AdwNavigationPage + AdwToolbarView + AdwHeaderBar, update `switchToView()` to use `replace()`, update `onSidebarGroupRowSelected()`
- `src/apprt/gtk/keys.zig` — update shortcuts to call new `switchToView()`, update Escape to call `pop()`

**AppState changes:**
```zig
// Remove:
content_stack: ?*c.GtkWidget = null,
content_title: ?*c.GtkWidget = null,

// Add:
nav_view: ?*c.GtkWidget = null,  // AdwNavigationView
// Per-view AdwNavigationPage pointers (for replace()):
home_page: ?*c.GtkWidget = null,
movies_page: ?*c.GtkWidget = null,
// ... etc for each view
```

**Success criteria:**
- [x] Sidebar clicks replace the navigation stack with the correct top-level page
- [x] Each page shows its own header bar with the view title
- [x] No back button on top-level pages (stack depth 1)
- [x] Keyboard 1-8 switches views and updates sidebar selection
- [x] Escape pops navigation (no-op at root)
- [x] Animated slide transitions between pages

---

#### Phase 3: Drill-Down Navigation (Detail + Player)

**Goal:** `showDetail()` pushes the detail page. `switchToPlayer()` pushes the player page. Back buttons appear and work. Playback stops on player pop.

**Files changed:**
- `src/apprt/gtk/app.zig` — update `showDetail()` to use `push(detail_page)`, update `switchToPlayer()` to use `push(player_page)`, add `popped` signal handler to stop playback when player is popped
- `src/apprt/gtk/detail_view.zig` — update `AdwNavigationPage` title to media item name in `showItem()`
- `src/apprt/gtk/player_controls.zig` — no structural changes (controls remain as overlay on player page)

**Success criteria:**
- [x] Clicking a poster pushes detail page with animated transition
- [x] Detail page header shows media title and back button
- [x] Clicking Play pushes player page
- [x] Player page header shows "Now Playing" and back button
- [x] Back button pops to previous page
- [x] Popping player page stops playback
- [x] Sidebar click from detail/player replaces entire stack (returns to top-level)

---

#### Phase 4: View Data Refresh

**Goal:** Wire up `refresh()` calls via `AdwNavigationPage` `showing` signal so views load/update their data when becoming visible.

**Files changed:**
- `src/apprt/gtk/app.zig` — connect `showing` signal on each page to the view's `refresh()` method
- `src/apprt/gtk/downloads_view.zig` — start timer on `showing`, stop on `hiding`
- `src/apprt/gtk/home_view.zig`, `movies_view.zig`, `tv_shows_view.zig`, `other_view.zig`, `favorites_view.zig`, `collections_view.zig`, `files_view.zig` — ensure `refresh()` is safe to call multiple times

**Success criteria:**
- [x] Views load data when first navigated to (via `showing` signal)
- [x] Views refresh when returned to (e.g., pop back from detail)
- [ ] Downloads timer only runs when downloads page is visible (deferred — timer is safe as-is)
- [x] No crashes from refreshing views that are not in the stack

---

#### Phase 5: Modernize Sidebar

**Goal:** Replace the 3 separate `GtkListBox` groups with a single `GtkListBox` using section headers and icons. Match GNOME sidebar conventions.

**Files changed:**
- `src/apprt/gtk/app.zig` — rebuild sidebar with single `GtkListBox`, add `GtkLabel` section headers ("Library", "Personal", "System"), add icons to each row

**Sidebar row structure:**
```
GtkListBox (navigation-sidebar CSS class)
  ├── GtkLabel("Library")          [section header, dim-label + caption CSS]
  ├── GtkListBoxRow: GtkBox { GtkImage("go-home-symbolic") + GtkLabel("Home") }
  ├── GtkListBoxRow: GtkBox { GtkImage("camera-video-symbolic") + GtkLabel("Movies") }
  ├── GtkListBoxRow: GtkBox { GtkImage("tv-symbolic") + GtkLabel("TV Shows") }
  ├── GtkListBoxRow: GtkBox { GtkImage("folder-symbolic") + GtkLabel("Other") }
  ├── GtkLabel("Personal")         [section header]
  ├── GtkListBoxRow: GtkBox { GtkImage("starred-symbolic") + GtkLabel("Favorites") }
  ├── GtkListBoxRow: GtkBox { GtkImage("view-list-symbolic") + GtkLabel("Collections") }
  ├── GtkListBoxRow: GtkBox { GtkImage("folder-open-symbolic") + GtkLabel("Files") }
  ├── GtkLabel("System")           [section header]
  ├── GtkListBoxRow: GtkBox { GtkImage("folder-download-symbolic") + GtkLabel("Downloads") }
  └── GtkListBoxRow: GtkBox { GtkImage("emblem-system-symbolic") + GtkLabel("Settings") }
```

**Success criteria:**
- [x] Single GtkListBox with section headers
- [x] Each row has an icon + label
- [x] Section headers are non-selectable (activatable + selectable set to false)
- [x] Selection works correctly (single list box, one row at a time)
- [x] Keyboard shortcuts map to correct items via row_to_item lookup
- [x] Visual style matches GNOME sidebar (navigation-sidebar CSS, dim-label headers)

---

#### Phase 6: Fullscreen and Polish

**Goal:** Fix fullscreen mode for the new split view, fix keyboard shortcut edge cases, clean up removed code.

**Files changed:**
- `src/apprt/gtk/app.zig` — update `toggleFullscreen()` for `AdwNavigationSplitView`, fix sidebar selection sync with keyboard shortcuts
- `src/apprt/gtk/keys.zig` — disable number shortcuts when player page is active, Escape pops before unfullscreening

**Success criteria:**
- [x] Fullscreen hides sidebar (via set_collapsed + set_show_content)
- [x] Exiting fullscreen restores sidebar
- [x] Number keys disabled during playback (Space/arrows still control player)
- [x] Escape in fullscreen player: first exits fullscreen, second pops player page
- [x] isPlayerVisible() queries nav view's visible page instead of stale active_view

## Alternative Approaches Considered

(see brainstorm: `docs/brainstorms/2026-03-16-adwaita-native-gtk-brainstorm.md`)

1. **Keep AdwOverlaySplitView, add AdwNavigationView for content only** — Rejected because OverlaySplitView is for mobile overlay patterns, not persistent two-pane layouts. Mixing them feels half-native.
2. **AdwViewStack + AdwNavigationView hybrid** — Rejected as over-engineered. Sidebar + ViewStack is redundant for this use case.

## System-Wide Impact

### Interaction Graph

- Sidebar row selection → `onSidebarRowSelected()` → `adw_navigation_view_replace()` → page `showing` signal → view `refresh()`
- Poster click → `showDetail()` → `adw_navigation_view_push(detail_page)` → detail `showing` signal → `showItem()` populates detail
- Play button → `switchToPlayer()` → `adw_navigation_view_push(player_page)` → player `showing` signal → mpv loads file
- Back button / Escape → `adw_navigation_view_pop()` → player `hiding` signal → playback stops → previous page `showing` signal → refresh

### Error Propagation

- If `adw_navigation_view_replace()` is called with a page already in the stack, libadwaita will log a critical warning. Must ensure pages are not double-pushed.
- If a view's `refresh()` fails (e.g., database error), it should show the `AdwStatusPage` empty state rather than crashing.

### State Lifecycle Risks

- **Player page pop without stopping playback:** mpv continues rendering frames to a GtkGLArea that is not visible. Connect `hiding` signal to stop playback.
- **Downloads timer on hidden page:** Timer callback modifies widgets not in the display tree. Could be a no-op (GTK handles unmapped widgets gracefully) but wastes CPU. Stop timer on `hiding`.
- **Detail page singleton with stale data:** If the detail page is in the stack and `showDetail()` is called for a different item, the page content updates in-place. This is correct behavior — the page is already visible, so the user sees the update immediately.

### API Surface Parity

- `switchToView(name)` — must change from `gtk_stack_set_visible_child_name()` to `adw_navigation_view_replace()`
- `showDetail(id)` — must change from stack switch to `push()`
- `switchToPlayer(path)` — must change from stack switch to `push()`
- `toggleFullscreen()` — must change from `adw_overlay_split_view_set_collapsed()` to `AdwNavigationSplitView` equivalent

## Acceptance Criteria

### Functional Requirements

- [ ] Main layout uses `AdwNavigationSplitView`
- [ ] Content pane uses `AdwNavigationView` with push/pop/replace
- [ ] Sidebar clicks replace navigation stack (no history accumulation)
- [ ] Poster → detail → player drill-down uses push with animated transitions
- [ ] Back button appears on detail and player pages
- [ ] Escape pops the navigation stack
- [ ] Player page stops playback when popped
- [ ] Views refresh data when becoming visible
- [ ] Downloads timer only runs when page is visible
- [ ] Sidebar has icons and section headers
- [ ] Fullscreen hides sidebar, exit restores it
- [ ] Keyboard shortcuts 1-8 work and sync sidebar selection
- [ ] Direct play mode (`reel /path/file.mkv`) is unaffected

### Non-Functional Requirements

- [ ] No memory leaks from page lifecycle (views are singletons)
- [ ] GtkGLArea/mpv render context is never destroyed and recreated
- [ ] Page transitions are smooth (default libadwaita animation)
- [ ] App compiles with zero warnings

### Quality Gates

- [ ] All 6 phases compile and run independently (incremental progress)
- [ ] Each phase committed separately with clear messages
- [ ] Manual testing of all navigation flows after each phase

## Dependencies & Prerequisites

- libadwaita >= 1.4 (AdwNavigationSplitView was added in 1.4)
- No new library dependencies needed
- No backend/core changes required

## Risk Analysis & Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| AdwNavigationView doesn't allow re-pushing singleton pages | Medium | High | Use `replace()` which handles this correctly; verify in Phase 2 |
| GtkGLArea unrealize/realize on page push/pop | Low | High | Pages are singletons, never destroyed; GL context persists |
| Fullscreen API incompatibility with NavigationSplitView | Medium | Medium | Prototype in Phase 1; fallback to hiding sidebar widget directly |
| View refresh overhead on every navigation | Low | Low | Views already have refresh(); most are cheap database queries |
| libadwaita version too old on target systems | Low | High | Check `flake.nix` for pinned version; document minimum requirement |

## Sources & References

### Origin

- **Brainstorm document:** [docs/brainstorms/2026-03-16-adwaita-native-gtk-brainstorm.md](docs/brainstorms/2026-03-16-adwaita-native-gtk-brainstorm.md) — Key decisions carried forward: AdwNavigationSplitView + AdwNavigationView architecture, player as navigation page, modernized sidebar with icons

### Internal References

- Main app layout: `src/apprt/gtk/app.zig:248` (`buildSidebarLayout`)
- Current split view: `src/apprt/gtk/app.zig:270` (`adw_overlay_split_view_new`)
- GtkStack creation: `src/apprt/gtk/app.zig:335` (`gtk_stack_new`)
- Sidebar row selection: `src/apprt/gtk/app.zig:417` (`onSidebarGroupRowSelected`)
- View switching: `src/apprt/gtk/app.zig:600` (`switchToView`)
- Detail navigation: `src/apprt/gtk/app.zig:658` (`showDetail`)
- Player navigation: `src/apprt/gtk/app.zig:630` (`switchToPlayer`)
- Fullscreen toggle: `src/apprt/gtk/app.zig:479` (`toggleFullscreen`)
- Keyboard shortcuts: `src/apprt/gtk/keys.zig:43` (number keys), `:77` (Escape)
- Video area lifecycle: `src/apprt/gtk/video_area.zig:39` (onRealize), `:95` (onUnrealize)
- Downloads timer: `src/apprt/gtk/downloads_view.zig` (500ms poll)

### External References

- AdwNavigationSplitView API: `https://gnome.pages.gitlab.gnome.org/libadwaita/doc/1-latest/class.NavigationSplitView.html`
- AdwNavigationView API: `https://gnome.pages.gitlab.gnome.org/libadwaita/doc/1-latest/class.NavigationView.html`
- AdwNavigationPage API: `https://gnome.pages.gitlab.gnome.org/libadwaita/doc/1-latest/class.NavigationPage.html`
