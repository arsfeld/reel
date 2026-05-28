---
date: 2026-05-27
topic: detail-page-liquid-glass-polish
---

## Summary

Restructure the movie and show detail hero so the poster, title, metadata,
credits, and Play button sit together over the backdrop (Plex / Apple TV
style). Fix the season-card poster overflow and the episode-card thumbnail
overlay bug. Round out the Liquid Glass aesthetic with the polish items
that surfaced while investigating these issues.

## Problem Frame

The "Liquid Glass" detail-page redesign (`b33a5f9`) introduced a hero +
indented-content layout: the floating poster lives inside the hero
`gtk::Overlay`, and the content area below reserves a 198px-wide spacer so
the title appears "right of" the poster. In practice the poster never
extends past the hero's bottom edge — GTK4 widget margins cannot be
negative and the `valign: End` poster bottoms out exactly at the hero
border — so the title row sits an empty 198px column to the right of
nothing, with the poster floating far above it.

Two unrelated structural bugs shipped alongside:

- `.season-card` has `border-radius: 14px` but the contained
  `gtk::Picture` has no matching radius. GTK4 does not clip child widgets
  to a parent's border-radius, so the square poster corners poke past the
  card's rounded ones. The card's vertical content (195 poster + name +
  episode count) also slightly exceeds the 215px scroll height, clipping
  the bottom labels.
- The episode-card thumbnail is constructed with
  `gtk::Overlay::add_overlay(&thumb_picture)` but never sets a base child
  via `set_child`. The Overlay collapses to zero natural height, so the
  picture renders but doesn't push the title / meta / overview siblings
  down — they paint on top of the thumbnail.

The CSS file also contains properties that GTK4's CSS parser does not
support (`transform`, `transition`) which generate runtime warnings, and
the cast photo is meant to be circular (`border-radius: 50px`) but lives
inside an unclipped card the same way the season poster does.

## Key Decisions

**Hero contains the headline; content area starts at overview.** The
poster, title, metadata badges, credits, and Play button all live inside
the hero `gtk::Overlay` on top of the backdrop. The content section below
the hero begins with overview, then genres, then everything else (cast,
episodes for shows, technical panel, collections). This removes the
198px-spacer hack and sidesteps GTK4's no-negative-margins constraint,
because the poster never needs to span the hero/content boundary — it sits
fully inside the hero alongside the text content.

**Movie and show share the same hero shape.** Both detail pages get the
same hero structure with the same widget arrangement; only the per-type
metadata (movie: runtime, director, writer; show: season count, network)
differs in what populates the text column.

**Liquid Glass polish is a single coordinated pass.** The reported bugs,
the hero restructure, the warning-generating CSS, and the cast-card
clipping all share the same surface area. Fixing them together produces
one coherent visual update rather than three or four small follow-ups.

## Requirements

### Hero restructure

- R1. The hero is a `gtk::Overlay` containing, from back to front: the
  backdrop image, a gradient scrim for readability, and a horizontal
  headline row (poster on the left, text column on the right).
- R2. The text column inside the hero contains, top to bottom, the title,
  the metadata badge row (year, runtime/seasons, rating, content rating),
  optional director/writer (or creator) lines, and the Play button.
- R3. The poster inside the hero is left-aligned, bottom-aligned, with the
  same dimensions as today (170×255). The text column starts to its right
  with consistent spacing.
- R4. The content section below the hero starts with the overview, then
  genre chips, then the existing sections (cast / seasons / episodes /
  technical / collections).
- R5. When no backdrop image is available, the hero falls back to a
  solid dark gradient (or a blurred copy of the poster if a poster
  exists). The headline content remains legible without a backdrop.
- R6. The hero gradient scrim is strong enough that white text remains
  readable over any backdrop, including bright daytime imagery.

### Layout bug fixes

- R7. Season-card posters render fully inside the rounded card boundary.
  No square corners poke past the parent card's `border-radius`. The card
  height accommodates the poster plus the name label and episode-count
  label without bottom clipping.
- R8. Episode-card thumbnails establish the Overlay's natural size so that
  subsequent siblings (title, meta, overview, progress) render BELOW the
  thumbnail, not on top of it. The episode-number badge continues to
  float in the thumbnail's top-left corner.
- R9. Cast-photo circles render as actual circles. The photo content is
  clipped to the round shape, not just the widget background.

### CSS hygiene

- R10. Unsupported GTK4 CSS properties (`transform`, `transition`,
  `translateY`, `cursor`, `pointer-events`) are removed from `style.css`.
  No new warnings appear in `cargo run` output when the detail page
  loads.
- R11. Hover and selection effects on glass surfaces are achieved via
  background-color / box-shadow swaps (which GTK4 supports), not the
  removed transform/transition properties.

### Polish

- R12. Glass panels (`.detail-panel`, `.cast-card`, `.season-card`,
  `.episode-card`) share a consistent inner inset highlight
  (`inset 0 0 0 1px alpha(white, …)`) so they read as one family.
- R13. Section spacing inside the content area is consistent — section
  titles, panels, and horizontal scrollers use the same vertical rhythm.
- R14. Section titles ("Cast", "Seasons", "Episodes") share a single
  styled class with the same weight, size, and top/bottom margins.
- R15. Selected season treatment keeps a clear accent ring without
  shifting the card's allocated space (no layout reflow on
  select / deselect).

## Scope Boundaries

- Trailer / Favorite / "More info" secondary action buttons styled in
  CSS but not currently wired up — deferred. Adding them is a separate
  feature, not part of this fix-up pass.
- Animation polish that GTK4 CSS doesn't support (transforms, opacity
  transitions, scale on hover) — deferred until the project adopts a
  different animation strategy (e.g. Adw animations API).
- Drag-and-drop, keyboard focus rings beyond what GTK provides
  automatically, and other interaction-layer work — out of scope.
- Backdrop blur behind the hero text column (true Apple-style frosted
  blur) — not available in GTK4 CSS. The fallback is a gradient scrim.
- Other detail-adjacent screens (library grid, home view, search) — this
  brainstorm is scoped to the movie and show detail pages only.

## Dependencies / Assumptions

- The `gtk::Overlay` widget can host both a backdrop `gtk::Picture` and a
  horizontally-laid-out `gtk::Box` of headline content as overlay
  children, with the box positioned via `valign` / `halign` /
  `margin_*`. This is consistent with how the current hero already
  uses `add_overlay` for the poster + gradient.
- The existing `gtk::Overlay::set_child` vs `add_overlay` distinction is
  the right fix for the episode-card thumbnail; no widget-tree
  restructure beyond that one call site is required for R8.
- GTK4 honors `border-radius` on `gtk::Picture` widgets such that the
  rendered image content is clipped to the rounded shape. If this turns
  out to be untrue in practice for `ContentFit::Cover`, a wrapping
  container with a clip will be needed instead — flagged in Outstanding
  Questions.

## Outstanding Questions

### Resolve before planning

- None. The hero shape is pinned (Plex / Apple TV style, headline content
  inside the hero), the three reported bugs have known fixes, and the
  polish list is bounded.

### Deferred to planning

- Whether `border-radius` on `gtk::Picture` actually clips its rendered
  image in GTK4, or whether a wrapper container with a clip is needed
  for R7 and R9. Verify by inspection while implementing; both
  approaches produce the same visual outcome.
- Exact gradient stops for the hero scrim (R6) — tune visually during
  implementation rather than spec'ing percentages here.
- Whether the no-backdrop fallback (R5) uses a blurred poster or just a
  solid dark gradient. Try the blur first — if `gtk::Picture` plus a
  CSS filter doesn't produce an acceptable result in GTK4, fall back to
  the solid gradient.

## Sources / Research

- Current detail components: `src/components/detail/movie_detail.rs`,
  `src/components/detail/show_detail.rs`.
- Liquid Glass stylesheet section: `src/style.css` lines 336–625.
- Original redesign commit: `b33a5f9` ("feat: redesign detail pages with
  Infuse-inspired 'Liquid Glass' layout").
- GTK4 CSS subset limitations and supported properties:
  `CLAUDE.md` § "GTK4 CSS Rules".
