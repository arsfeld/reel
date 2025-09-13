**UI Parity Analysis (GTK vs Relm4)**

- Window shell
  - GTK: `Adw.ApplicationWindow` uses a Blueprint-based composite template that nests an `Adw.NavigationSplitView` whose sidebar AND content are each wrapped in their own `Adw.ToolbarView` with a dedicated `Adw.HeaderBar` per pane (sidebar header with menu; content header with title and actions). Initial content shows an `Adw.StatusPage` empty state.
  - Relm4: `Adw.ApplicationWindow` contains a single top-level `Adw.ToolbarView` with a single `Adw.HeaderBar` that sits above an `Adw.NavigationSplitView`. The split view’s content hosts an `Adw.NavigationView`. No per-pane `ToolbarView` or `HeaderBar` wrappers. No `StatusPage` empty state.

- Header bars and actions
  - GTK: Two header bars (sidebar and content). The sidebar header contains a primary menu button bound to `primary_menu` (Preferences/About) using application actions (`app.preferences`, `app.about`). The content header (`content_header`) has `show-title: true` and is intended to be updated per-page (title/subtitle, context actions, filters). Accels: `<primary>comma` for preferences, `<primary>w` for close.
  - Relm4: One header bar. Start uses a `SplitButton` (sidebar toggle), center has a custom title widget with icon + `adw::WindowTitle`, end has search button and a `MenuButton` with no wired `gio::MenuModel` or app actions. No accels on the `Application` visible here.

- Navigation/back behavior
  - GTK: Uses a `NavigationManager` tied to `content_header` to manage back button and header state. Pages are stacked in a manual content `Stack`; blueprint sets up the structure where the header follows page context.
  - Relm4: Uses `adw::NavigationView` for push/pop. No explicit back button wired to header bar state. Header does not react to `NavigationView.can_pop()`.

- Sidebar UX
  - GTK: Sidebar is a reactive widget placed inside `sidebar_placeholder` of the sidebar’s `Adw.ToolbarView`. Sidebar area uses Adwaita conventions (`navigation-sidebar` styles, boxed lists, headings, separators). Menu in the sidebar header.
  - Relm4: Sidebar is a component controller set as the `NavigationSplitView` sidebar child directly (no `ToolbarView` wrapper), so it loses the distinct sidebar header area and its menu placement.

- Empty states and first-run
  - GTK: Shows `Adw.StatusPage` “Select a Library” until user selects something; progressive enhancement after sources load.
  - Relm4: Immediately pushes a “Home” page. No explicit neutral `StatusPage` placeholder.

- Player chrome behavior
  - GTK: Player page hides chrome and uses OSD-styled controls; relies on CSS classes (e.g., `.osd.pill`, overlays) from `style.css`.
  - Relm4: Hides header and sets `ToolbarView` top bar style to Flat during playback, then restores on exit. Some OSD CSS exists in global CSS, but not fully aligned with GTK `style.css`.

- Styling and CSS
  - GTK: Loads rich styles from `src/platforms/gtk/ui/style.css` (cards, poster overlays, episode styling, header filter controls, `navigation-split-view` backgrounds, status pages, etc.) via gresources. Class names used across widgets (`title-*`, `dim-label`, `pill`, `boxed-list`, `navigation-sidebar`, etc.).
  - Relm4: Uses `relm4::set_global_css` with a slimmed-down set of classes; many GTK styles are missing. Several components do add Adwaita-ish classes, but the CSS backing differs, so results look less “Adwaita-polished”.

- App actions and menus
  - GTK: `gio::SimpleAction` for preferences/about, menu model attached, and keyboard accelerators configured on the `Application`.
  - Relm4: No wired application actions in `MainWindow`; app-level example exists in `relm4/app.rs`, but not unified with `MainWindow`’s menu button.


**Adwaita Parity Plan (Relm4)**

- Layout and Structure
  - Replace single top-level `ToolbarView` with per-pane header bars:
    - Wrap `NavigationSplitView.sidebar` in an `Adw.ToolbarView` with its own `Adw.HeaderBar` (menu in the right end, optional “Reel” title on the left).
    - Wrap `NavigationSplitView.content` in an `Adw.ToolbarView` with its own `Adw.HeaderBar` (page title/subtitle in center, contextual actions on the right). Keep `Adw.NavigationView` as the content child.
    - Preserve split view sizing: `min_sidebar_width: 280`, `max_sidebar_width: 400`, `sidebar_width_fraction: 0.25`.

- Header Bar Behavior
  - Content header: use `adw::WindowTitle` and dynamically set page title/subtitle on navigation changes. Show back button when `navigation_view.can_pop() == true`.
  - Sidebar header: place the “hamburger” menu (`open-menu-symbolic`) bound to app actions. Move the sidebar toggle button either into the content header’s start (recommended) or keep as-is; ensure it’s `flat` and matches HIG spacing.
  - Add a `gtk::SearchEntry` or a search button that navigates to a dedicated search page; avoid a persistent search button if it’s not contextually useful per HIG.

- Empty State
  - Before any library/content is selected, show an `Adw.StatusPage` in the content area: icon `folder-symbolic`, title “Select a Library”, description guidance. Replace with actual page on navigation.

- Navigation Integration
  - Listen for `navigation_view` push/pop and update header title/subtitle and the back button visibility.
  - Standardize page titles:
    - Home: “Home” (subtitle optional)
    - Library: library name (subtitle library type or source name)
    - Movie/Show details: media title (subtitle year or source)
    - Player: hide content header; set `ToolbarStyle::Flat`; restore on exit.

- App Actions and Menus
  - Define `gio::SimpleAction`s for `app.preferences`, `app.about` on the `Application` and set accels: `<primary>comma` (preferences), `<primary>w` (close). Reuse app-level wiring from `relm4/app.rs` or move to a shared initializer.
  - Attach a `gio::MenuModel` to the `MenuButton` matching GTK’s `primary_menu` (Preferences, About Reel).

- CSS and Styling
  - Unify CSS:
    - Extract shared Adwaita-style CSS into a reusable file (e.g., `src/platforms/shared/style.css`) or a gresource used by both GTK and Relm4 builds.
    - Port key selectors from GTK `style.css`: `.navigation-sidebar`, boxed lists, headerbar filter controls, `statuspage` margins, media cards, overlays, episode cards, progress bars.
    - Remove duplicated or conflicting rules from `relm4::set_global_css` once shared CSS is loaded.
  - Ensure components add the same classes GTK uses (`boxed-list`, `heading`, `dim-label`, `pill`, `navigation-sidebar`, etc.). Audit Relm4 components and align class names.

- Player Chrome + OSD
  - Ensure player component adds the same CSS classes GTK expects (`.osd.pill`, `.auto-play-overlay`, `.pip-container`, etc.) and that shared CSS includes their definitions.
  - Maintain immersive mode: hide content header, set `ToolbarStyle::Flat`, restore chrome and window state on exit.

- Theming
  - Use `adw::StyleManager::default()` and follow the configured color scheme (PreferDark/ForceDark/ForceLight) similar to GTK path.
  - If the project has a config-driven theme preference, wire it to `StyleManager` in Relm4 like GTK does.

- Spacing, Sizing, Typography
  - Standardize margins/spacing to GNOME HIG scale (6/12/18/24). Review `set_spacing` and `set_margin_*` in Relm4 components.
  - Use `adw::WindowTitle` typography and keep `.title-*`, `.heading`, `.body`, `.caption` in shared CSS to match GTK visuals.

- Accessibility and States
  - Ensure focus rings/hover states follow Adwaita: prefer `flat` buttons in header, use `linked` for action groups, and keep status pages readable.
  - Verify contrast of overlays and labels in both dark and light color schemes.


**Implementation Steps**

1) Restructure Main Window Layout (Relm4)
   - Update `src/platforms/relm4/components/main_window.rs` view! macro:
     - Move the top-level `ToolbarView` and `HeaderBar` into the `NavigationSplitView.sidebar` and `.content` as independent `ToolbarView` + `HeaderBar` pairs.
     - Add `Adw.StatusPage` as default content.
     - Keep `Adw.NavigationView` as child inside the content pane’s `ToolbarView`.

2) Wire Header State + Back Button
   - On `NavigationView` push/pop, update `WindowTitle` and toggle back button visibility.
   - Standardize per-page titles/subtitles (Home, Library[name], Details[title], Player).

3) App Menu and Actions
   - Create `gio::MenuModel` (Preferences, About) and attach to the sidebar header `MenuButton`.
   - Register `gio::SimpleAction`s on `Application`; set accels: `<primary>comma`, `<primary>w`.

4) CSS Unification
   - Create `src/platforms/shared/style.css` (or reuse GTK `style.css`) and load it for Relm4 (via `relm4::set_global_css` or a gresource loader).
   - Port rules for: `navigation-split-view` scrolled background, `statuspage` margins, headerbar filter control sizes, `.navigation-sidebar` list styles, cards/posters/episodes, OSD.
   - Remove overlapping rules from existing `set_global_css` after confirming parity.

5) Sidebar Alignment
   - Ensure Relm4 `Sidebar` uses the same classes as GTK (boxed lists, headings, labels) and lives under a pane-level `ToolbarView`.
   - Place the primary menu button in the sidebar header; optional “Reel” title at start.

6) Player Immersive Mode
   - Confirm hide/show of header and `ToolbarStyle` changes already implemented.
   - Add/align OSD CSS classes and ensure runtime toggles apply the expected classes to the root container.

7) Theming and Preferences
   - Mirror GTK theme behavior: color scheme via `StyleManager`, hook in config if present.
   - Verify light/dark correctness across key views.

8) QA Checklist
   - Header looks identical across Home/Library/Details/Player.
   - Menu opens with Preferences/About; accels work.
   - Empty state appears before first selection; transitions look native.
   - Sidebar spacing, listbox separators, and hover/focus states match GTK.
   - Cards/posters/episodes visually consistent with GTK.
   - Player chrome hides/restores correctly; OSD matches styling.


**Milestones**

- M1: Header refactor + empty state visible (structural parity)
- M2: App menu/actions + back button behavior
- M3: CSS unification pass 1 (sidebar, headers, status pages)
- M4: CSS unification pass 2 (cards, details, episodes, OSD)
- M5: Player polish + theme verification


**Risks / Notes**

- Moving to per-pane `ToolbarView` requires small view! macro reshuffle but is low risk.
- CSS duplication can cause drift; centralize in a shared stylesheet to avoid divergence.
- Title/subtitle updates must be driven by navigation outputs; ensure all pages emit title context.
- If gresources are preferred for production, add a minimal loader in the Relm4 path and include the shared CSS there.

