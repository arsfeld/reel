---
title: "feat: Detail page Liquid Glass polish — hero restructure, layout fixes, CSS hygiene"
date: 2026-05-27
status: active
type: feat
origin: docs/brainstorms/2026-05-27-detail-page-liquid-glass-polish-requirements.md
---

## Summary

Restructure the movie and show detail hero so the poster, title, metadata,
credits, and Play button live together over the backdrop (Plex / Apple TV
style). Fix three layout bugs that shipped with the original Liquid Glass
redesign (`b33a5f9`): episode-card thumbnail overlap, season-card poster
clipping, cast-photo clipping. Remove unsupported GTK4 CSS properties that
generate runtime warnings. Tighten shared visual rhythm across the glass
surfaces.

## Problem Frame

The Liquid Glass detail redesign introduced a hero + indented-content
layout where the floating poster lived inside the hero `gtk::Overlay` and
the content area below reserved a 198px-wide spacer so the title would
appear "right of" the poster. In practice the poster never extends past
the hero's bottom edge — GTK4 widget margins can't be negative and
`valign: End` bottoms the poster out exactly at the hero border — so the
title row ends up with an empty 198px column to the left of the title and
the poster floating far above it. Three additional bugs shipped at the
same time:

- `.season-card` has `border-radius: 14px`; the contained `gtk::Picture`
  has no matching radius. GTK4 doesn't clip child widgets to a parent's
  border-radius, so the square poster corners poke past the card's
  rounded corners. The card's content (195 poster + name + episode count)
  also slightly exceeds the 215px scroll height.
- The episode-card thumbnail is constructed with
  `gtk::Overlay::add_overlay(&thumb_picture)` but never sets a base child
  via `set_child`. The Overlay collapses to zero natural height, so the
  picture renders but doesn't push the title / meta / overview siblings
  down — they paint on top of the thumbnail.
- `style.css` contains `transform`, `transition`, `translateY` properties
  that GTK4's CSS parser doesn't support and warns about at runtime.

The fix is one coordinated pass across `movie_detail.rs`, `show_detail.rs`,
and `style.css` — the surfaces overlap enough that doing them together
keeps the visual treatment coherent.

---

## Key Technical Decisions

**Hero contains the headline; content area starts at overview.** The
poster, title, metadata badges, credits, and Play button all live inside
the hero `gtk::Overlay` as a single horizontal headline row positioned
with `valign: End` and bottom padding. The content section below begins
with the overview and continues with genres, then the existing sections.
This removes the 198px-spacer hack and sidesteps GTK4's no-negative-
margins constraint, because the poster no longer needs to span the
hero/content boundary. (see origin: `docs/brainstorms/2026-05-27-detail-page-liquid-glass-polish-requirements.md`)

**Movie and show share the hero shape but not a shared widget builder.**
Both detail pages get the same hero structure, but the per-type
populating code (movie: runtime, director, writer; show: season count,
creator) differs enough that a shared helper would either be parametric
to the point of obscurity or carry both shapes' fields. Two parallel
implementations are clearer and align with the project's "don't add
abstractions the task doesn't require" guidance.

**No-backdrop fallback is solid dark, not blurred poster.** When a
backdrop is missing, the hero falls back to the existing dark gradient
scrim with the poster visible on top. A blurred-poster fallback would
require generating a CPU-blurred texture at load time (cairo or
gdk-pixbuf), which is a separate piece of work and outside the polish
scope.

**Hover and selection effects use background-color / box-shadow swaps.**
GTK4 CSS does not support `transform`, `transition`, or any animation
properties (per `CLAUDE.md` § GTK4 CSS Rules). Where the existing CSS
relied on `transform: translateY(-2px)` for lift on hover, the
replacement is a heavier `box-shadow` swap that conveys the same lift
without the unsupported transform.

**One commit per Implementation Unit by default.** The bug fixes (U1),
CSS hygiene (U2), and polish pass (U6) each stand alone. The two hero
restructures (U3, U4) ship together as a paired commit so movie and show
don't diverge visually mid-series.

---

## Requirements

Carrying forward from origin requirements doc. R-IDs preserved.

### Hero restructure
- R1. Hero is a `gtk::Overlay` with backdrop, gradient scrim, and a
  horizontal headline row (poster left, text column right).
- R2. Text column contains title, metadata badge row, credits, Play button.
- R3. Poster inside hero is 170×255, bottom-aligned, left-aligned with
  consistent spacing to the text column.
- R4. Content section below hero starts with overview, then genre chips,
  then existing sections.
- R5. No-backdrop fallback: dark gradient with poster still visible. Headline
  content remains legible.
- R6. Gradient scrim strong enough that white text reads over any backdrop.

### Layout bug fixes
- R7. Season-card posters render inside the rounded card boundary; card
  height accommodates poster + labels without bottom clipping.
- R8. Episode-card thumbnails establish the Overlay's natural size so
  siblings render below, not on top.
- R9. Cast-photo circles render as actual circles, with image content
  clipped to the round shape.

### CSS hygiene
- R10. No `transform`, `transition`, `translateY`, `cursor`, or
  `pointer-events` in `style.css`; no new GTK CSS warnings at runtime.
- R11. Hover and selection effects achieved via supported properties
  (`background-color`, `box-shadow`).

### Polish
- R12. Glass panels share a consistent inner inset highlight.
- R13. Section spacing inside content area is consistent.
- R14. Section titles share one styled class.
- R15. Selected season treatment keeps an accent ring without layout
  reflow on select/deselect.

---

## High-Level Technical Design

### Hero widget tree (after restructure)

```
gtk::Overlay (height_request: 420, hexpand)
├── child: gtk::Picture (backdrop, content_fit: Cover)
├── overlay: gtk::Box (.detail-hero-overlay — gradient scrim, vexpand+hexpand)
└── overlay: adw::Clamp (maximum_size: 1400, valign: End)
    └── gtk::Box (.detail-hero-headline, orientation: Horizontal,
                  spacing: 28, margin_bottom: 24, margin_start: 28,
                  margin_end: 28, valign: End)
        ├── gtk::Picture (.detail-poster-hero, 170×255, halign: Start)
        └── gtk::Box (orientation: Vertical, hexpand: true,
                      spacing: 10, valign: End)
            ├── gtk::Label (.title-1)
            ├── gtk::Box (.meta-badges — year, runtime/seasons, rating, content rating)
            ├── gtk::Label (director / creator, .dim-label, visible-when-populated)
            ├── gtk::Label (writer, .dim-label, visible-when-populated)
            └── gtk::Box (.detail-actions — Play button)
```

Below the hero, `content_box` starts with the overview label, then genre
chips, then the existing sections (cast, seasons/episodes, technical,
collections). The previous `title_meta_row` (with the 198px spacer and
the two-column header) is removed entirely.

### Episode-card thumbnail (after fix)

```
gtk::Box (.episode-card, orientation: Vertical, width_request: 290)
├── gtk::Overlay
│   ├── child: gtk::Picture (.episode-card-thumb, 290×163)  ← set_child, not add_overlay
│   └── overlay: gtk::Label (.episode-number-badge)
├── title label
├── meta label
├── overview label
└── progress bar / watched label
```

The single-call change is `thumb_overlay.set_child(Some(&thumb_picture))`
instead of `thumb_overlay.add_overlay(&thumb_picture)`. The badge stays
as `add_overlay`.

### Season-card sizing (after fix)

The `.season-card-poster` CSS class gains `border-radius: 14px 14px 0 0`
(matching the top of the card; bottom is straight because the labels sit
below). `season_scroll` height grows from 215 to 248 to accommodate
poster (195) + name label (~22) + ep count (~16) + padding. The card box
gets `overflow: hidden` equivalent via the parent `.season-card`'s
`border-radius` and a tightened `width_request` so children honor the
clip.

---

## Implementation Units

### U1. Fix three layout bugs in detail components

**Goal:** Fix the episode-card thumbnail Overlay, the season-card poster
clipping, and verify/fix cast-photo circular clipping. These are small,
unrelated mechanical fixes bundled into one commit.

**Requirements:** R7, R8, R9.

**Dependencies:** None.

**Files:**
- `src/components/detail/show_detail.rs` (episode-card thumbnail + season-card height)
- `src/style.css` (.season-card-poster border-radius, .cast-photo if needed)
- `src/components/detail/movie_detail.rs` (cast card structure — for R9 if needed)

**Approach:**
- **Episode-card thumbnail (R8):** In `rebuild_episode_cards`, change
  `thumb_overlay.add_overlay(&thumb_picture)` to
  `thumb_overlay.set_child(Some(&thumb_picture))`. Keep the badge as
  `add_overlay`. This gives the Overlay a real base child so it sizes to
  the picture's natural height and siblings flow below it.
- **Season-card poster clipping (R7):** Add `border-radius: 14px 14px 0 0`
  to `.season-card-poster` (top corners match the card; bottom stays
  straight where labels sit). Increase `season_scroll.height_request`
  from 215 to 248 so the poster + name + ep count all render without
  bottom clipping. Confirm the season card box's children align with the
  parent's `border-radius` clip — if not, wrap the poster in a small
  container whose CSS class enforces the clip.
- **Cast-photo (R9):** Verify whether `border-radius: 50px` on
  `.cast-photo` actually clips the rendered image in GTK4. If yes, no
  change needed. If the image bleeds past the round shape, add a wrapping
  container with `overflow`-equivalent CSS, or set
  `picture.set_can_shrink(true)` and constrain via the picture's own
  border-radius. The 72×72 picture with `border-radius: 50px` should
  clamp to half = 36px = circle.

**Test scenarios:**
Test expectation: none -- GTK layout work, verified manually per
`CLAUDE.md` § "Do NOT Unit Test (Manual/Visual Only)".

**Verification:**
- Run the app, open a show detail page, scroll to episodes. Episode title,
  meta, and overview render *below* the thumbnail, not on top.
- Open the same show detail page, scroll to seasons. Season posters
  show with rounded top corners matching the card; name and episode
  count are fully visible at the bottom.
- Open any movie detail page (or show after metadata loads). Cast photos
  render as visually circular images, not squares behind a circular mask.

---

### U2. CSS hygiene — remove unsupported properties

**Goal:** Strip GTK4-unsupported CSS so the parser stops warning. Where
the unsupported properties carried real visual intent (hover lift,
transition fade), substitute supported equivalents.

**Requirements:** R10, R11.

**Dependencies:** None.

**Files:**
- `src/style.css`

**Approach:**
- Remove all `transition` declarations (current occurrences: lines 76,
  140, 347, 370–371; sweep for anything else added since). Note that
  removing line 347 (`.detail-hero picture { transition: opacity 400ms
  ease-out; }`) makes backdrop swaps instant when navigating between
  detail pages — acceptable for the same "instant change in GTK4" reason
  as the hover-lift swap.
- Remove `transform: translateY(-2px)` from `.detail-poster-hero:hover`
  (line 399). Replace the hover treatment by deepening the existing
  `box-shadow` (more spread, larger Y offset) so the poster still reads
  as "lifting" — without the transform.
- Sweep for `cursor`, `pointer-events`, `animation`, `@keyframes`,
  `letter-spacing`, `font-feature-settings` — remove any found. (Quick
  grep before editing confirms what's present; the CLAUDE.md GTK4 CSS
  rules list is the authoritative reject set.)
- For `.media-card-frame:hover` and other surfaces that previously
  relied on transitions, accept that hover changes are instant in GTK4
  CSS. This is consistent with how other Adwaita apps look.

**Test scenarios:**
Test expectation: none -- visual + log-based verification. No behavioral
contract to assert.

**Verification:**
- Run `nix develop -c cargo run` and hover over a season card, episode
  card, and the hero poster. Watch the terminal for `Gtk-WARNING` lines
  about unrecognized CSS properties — none should appear.
- The poster still reads as "lifted" on hover (shadow grows). Acceptable
  if the lift is shadow-only without the 2px translate.

---

### U3. Movie detail hero restructure

**Goal:** Move the title, metadata, credits, and Play button INSIDE the
hero `gtk::Overlay`, so the poster and headline content compose as one
unit over the backdrop. Remove the 198px spacer column and the
`title_meta_row` two-column header below the hero.

**Requirements:** R1, R2, R3, R4 (movie side).

**Dependencies:** U2 (so we're styling against the cleaned-up CSS baseline).

**Files:**
- `src/components/detail/movie_detail.rs` (hero construction + content_box reorganization)
- `src/style.css` (new `.detail-hero-headline` class, `.meta-badges` if needed)

**Approach:**
- Inside `init()`, build the hero overlay's third overlay child as an
  `adw::Clamp` containing a horizontal `gtk::Box` (.detail-hero-headline)
  with `valign: End`, `margin_bottom: 24`, `margin_start: 28`,
  `margin_end: 28`, `spacing: 28`.
- The headline box contains: poster (existing widget, moved from its
  current parent), then a vertical box (the text column) with `valign:
  End` containing title_label, meta_box, director_label, writer_label,
  and the action row (Play button).
- Remove the `detail_header_row` and `poster_column` widgets entirely
  from the content area below the hero. Remove the `poster_column`
  field from the `MovieDetail` struct.
- The content area below the hero now starts directly with the overview
  label, then genres, then the existing sections (cast, tech panel,
  collections).
- All of the existing populating logic in `update()` keeps the same
  widget references — only the parent layout changes. Reset logic that
  toggled `poster_column.set_visible(...)` is removed.
- Style notes: title and badge text need a `text-shadow` for legibility
  over backdrops with light areas; add to `.title-1` scoped under
  `.detail-hero-headline` (a new selector) or to a new class.

**Test scenarios:**
Test expectation: none -- GTK layout work.

**Verification:**
- Open a movie detail page (e.g., one with both backdrop and poster art).
  Poster, title, year + runtime + rating + content rating, "Directed
  by…", and Play button all render as one cluster anchored to the
  bottom-left of the hero, over the backdrop.
- Title is to the right of the poster, top-aligned with the poster's
  upper area.
- The overview text starts directly below the hero with normal content
  margins — no left indent, no spacer column.
- Genre chips appear after overview.
- Movie detail page still loads correctly on a movie with no backdrop
  (poster + headline content still readable; see U5 for fallback work).

---

### U4. Show detail hero restructure

**Goal:** Mirror U3 for show detail. Same hero shape with show-specific
populating differences (creator instead of director, season count badge,
etc.).

**Requirements:** R1, R2, R3, R4 (show side).

**Dependencies:** U3 (style classes and patterns established there;
applied here).

**Files:**
- `src/components/detail/show_detail.rs` (hero construction + content_box reorganization)

**Approach:**
- Same widget tree as U3's hero: clamp → horizontal headline box →
  poster + text column.
- The text column for shows contains: title, meta_box (year, rating,
  content rating — no runtime), director_label (rendered as "Created
  by"), writer_label, Play button. Note: the show's `meta_box` has 3
  badges where movie's has 4 (no runtime); already handled by visibility
  toggles.
- Remove the `title_meta_row`, `poster_spacer`, `title_meta_content`,
  and `hero_blend` widgets. The blend bar served no purpose with the
  separated layout and is unnecessary now.
- Remove the `poster_spacer` field from the `ShowDetail` struct.
- Content area below hero starts with overview, then genre chips, then
  the existing sections in their current order (show_info_panel,
  season_section, episode_section, cast_section, tech_panel,
  collections_panel).
- The seasons/episodes sections keep their current structure (subject to
  U1's fixes).

**Test scenarios:**
Test expectation: none -- GTK layout work.

**Verification:**
- Open a TV show detail page. Poster, title, year + rating + content
  rating, "Created by…", and Play button render as one cluster over the
  backdrop, anchored bottom-left.
- The show info panel ("N Seasons"), seasons row, episodes row, cast,
  tech, and collections all render below the hero in their existing
  order.
- No visual blend bar / gap between hero and content (the previously-
  added `hero_blend` is gone).
- Selecting a different season still scrolls episodes correctly.

---

### U5. Hero gradient tuning and no-backdrop fallback

**Goal:** Ensure white text is legible over any backdrop, and that pages
without backdrop art still look intentional (not broken).

**Requirements:** R5, R6.

**Dependencies:** U3, U4 (the hero's new layout is in place; this tunes
its appearance).

**Files:**
- `src/style.css` (`.detail-hero-overlay` gradient stops)
- `src/components/detail/movie_detail.rs` (no-backdrop path in `LoadMovie`)
- `src/components/detail/show_detail.rs` (no-backdrop path in `LoadShow`)

**Approach:**
- **Gradient (R6):** Strengthen `.detail-hero-overlay` so the bottom
  third — where the headline content sits — has enough opacity (≈0.65–
  0.75) to keep white text readable against a bright backdrop. Tune
  stops visually; suggested starting shape:
  ```
  linear-gradient(
    to top,
    alpha(black, 0.78) 0%,
    alpha(black, 0.62) 20%,
    alpha(black, 0.30) 55%,
    alpha(black, 0.05) 85%,
    transparent 100%
  )
  ```
  Add `text-shadow: 0 1px 3px rgba(0,0,0,0.55)` to the title label inside
  the hero (via a new `.detail-hero-headline .title-1` selector or a
  dedicated class) for additional safety against pathological backdrops.
- **No-backdrop fallback (R5):** When `item.backdrop_path` is `None` in
  `LoadMovie` / `LoadShow`, **first clear the backdrop Picture's
  paintable** (`self.backdrop.set_paintable(None::<&gtk::gdk::Texture>)`)
  so the previously-loaded item's texture isn't still painted over the
  fallback. Then add a `.detail-hero-empty` CSS class to the backdrop
  Picture that applies `background-color: alpha(@card_bg_color, 0.95)`
  or a background gradient. The existing gradient overlay continues to
  render on top. The poster and headline remain visible and legible.
  (The same clear-paintable step also fixes a pre-existing bug where
  navigating from item A to item B briefly shows A's backdrop while B
  loads, even when B has a backdrop.)
- Confirmed: a blurred-poster fallback is deferred (see Scope
  Boundaries).

**Test scenarios:**
Test expectation: none -- visual verification.

**Verification:**
- Open a movie detail page with a bright (e.g., daytime, snow) backdrop.
  Title text reads cleanly without squinting; metadata badges and Play
  button are clearly defined against the backdrop.
- Open a movie/show detail page where the item has no backdrop. Hero
  renders as a solid dark panel with the poster and headline content
  centered/anchored as designed — not blank or visually broken.
- Resize the window between narrow and wide. Hero contents stay
  positioned correctly (Clamp keeps headline width bounded; backdrop
  cover-fits).

---

### U6. Glass-surface polish pass

**Goal:** Tighten visual rhythm across the detail page surfaces — give
glass panels a shared inset highlight, normalize section titles,
stabilize the selected-season treatment so it doesn't reflow, and align
section spacing.

**Requirements:** R12, R13, R14, R15.

**Dependencies:** U1, U2, U3, U4 (the structure is in place; this is
finish work).

**Files:**
- `src/style.css` (panel inset, section title class, season-card-
  selected refinement)
- `src/components/detail/movie_detail.rs` (apply `.detail-section-title`
  consistently if any heading currently uses a different class)
- `src/components/detail/show_detail.rs` (same)

**Approach:**
- **Consistent insets (R12):** Add an `inset 0 0 0 1px alpha(white,
  0.05)` to `.detail-panel`, `.cast-card`, `.season-card`, and
  `.episode-card` so they share one subtle inner border. Existing
  shadows stay; this is an addition, not a replacement.
- **Section spacing (R13):** Verify the `content_box.spacing` of 18 in
  both components, plus the per-section spacing inside each cast / season
  / episode / tech / collections section. Make sure section titles and
  their content row use the same vertical rhythm. If any section's box
  spacing is off, normalize it to 8 (title to content) inside the
  section and rely on the outer `content_box.spacing(18)` for inter-
  section gaps.
- **Section title (R14):** Confirm "Cast", "Seasons", "Episodes" all use
  `.detail-section-title`. Already true in the current code; verify
  after restructure didn't regress.
- **Stable season selection (R15):** Replace the current
  `.season-card-selected` treatment (which currently uses `box-shadow:
  0 0 0 2px ...`) with an `inset 0 0 0 2px alpha(@accent_color, 0.6)`
  plus a subtle bg shift. The inset approach paints inside the card's
  existing bounds so selection doesn't shift the card's layout (the
  current outer box-shadow at 0 spread can still cause subpixel reflow
  with neighboring cards depending on shadow rendering).

**Test scenarios:**
Test expectation: none -- visual verification.

**Verification:**
- All glass surfaces (detail-panel, cast-card, season-card, episode-card)
  show a faint white inner border at close inspection. The four
  surfaces read as one design family.
- Scroll through cast and episode rows — vertical spacing between
  section title and its content is the same in both sections; spacing
  between sections is uniform.
- Click between season cards. The card you select gains the accent
  highlight without visibly nudging neighboring cards.

---

## Scope Boundaries

### Deferred to Follow-Up Work

- **Blurred-poster fallback for no-backdrop hero.** Generating a
  CPU-blurred texture (gdk-pixbuf scaling + downsample-and-upsample
  hack, or cairo blur) is a separate feature. Solid dark gradient ships
  in U5; blur is its own task if requested later.
- **Trailer / Favorite / "More info" secondary action buttons.** Styled
  in CSS (`.detail-secondary-btn`) but not currently wired up. Adding
  them is a feature, not part of this polish pass.
- **Real CSS animation polish.** GTK4 doesn't support `transform`,
  `transition`, or `@keyframes`; if/when the project adopts Adw's
  animations API or another approach, transitions can be re-introduced.
- **Cast-card corner clip** (if the cast-photo round shape needs a
  wrapper). U1 fixes this if needed; if no fix is required after
  verification, no further work.

### Outside this work

- Other detail-adjacent screens (library grid, home, search). This plan
  is scoped to the movie and show detail pages only.
- Backdrop blur behind hero text (true Apple-style frosted blur). Not
  available in GTK4 CSS.
- Drag-and-drop, keyboard focus rings beyond what GTK provides
  automatically.

---

## Open Questions (Deferred to Implementation)

These are runtime-verifiable questions where the right move is "try it,
observe, adjust" rather than "decide on paper." Each is owned by the
unit that surfaces it.

- **U1: Does GTK4 `border-radius` on `gtk::Picture` clip the rendered
  image content?** Verify visually. If yes (likely), R7 and R9 are
  one-line CSS changes. If no, wrap the Picture in a container whose
  CSS enforces the clip. Same answer applies to both season poster and
  cast photo.
- **U5: Final gradient stops.** The shape in High-Level Technical Design
  is a starting point. Tune against 3–4 backdrops with different
  brightness profiles during implementation; the spec'd values are
  directional, not authoritative.
- **U6: Whether the inset highlight value (`alpha(white, 0.05)`) reads
  correctly across Adwaita's light and dark themes.** GTK exposes light
  theme automatically; verify the inset still looks intentional under
  light theme, or wrap in a `:dir(ltr)` / theme-aware selector if not.

---

## Sources & Research

- Origin requirements doc:
  `docs/brainstorms/2026-05-27-detail-page-liquid-glass-polish-requirements.md`
- Original "Liquid Glass" redesign commit: `b33a5f9`.
- Current detail components:
  - `src/components/detail/movie_detail.rs`
  - `src/components/detail/show_detail.rs`
- Liquid Glass CSS: `src/style.css` lines 336–625.
- GTK4 CSS subset rules: `CLAUDE.md` § "GTK4 CSS Rules" — list of
  supported vs. unsupported properties; non-negative-margin constraint
  on widget builders.
- GTK test policy (no unit tests for layout): `CLAUDE.md` § "Do NOT
  Unit Test (Manual/Visual Only)".
- File size limits and test enforcement: `CLAUDE.md` § "File Size
  Limits" — `show_detail.rs` is currently 1182 lines, within the
  2000-line cap; this plan does not push it past the cap but flags U6
  to keep an eye on growth.
