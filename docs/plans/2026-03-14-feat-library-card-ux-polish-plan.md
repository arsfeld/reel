---
title: "feat: Library Card UX Polish (Infuse-style)"
type: feat
status: completed
date: 2026-03-14
---

# Library Card UX Polish (Infuse-style)

## Overview

Add visual polish to library grid cards — hover effects (scale + shadow lift), metadata overlay badges (resolution, rating), skeleton loading shimmer with poster fade-in, and watch progress bar infrastructure. Target aesthetic: Infuse (Apple-esque, clean, elegant).

## Problem Statement / Motivation

The library grid currently renders flat, static cards (poster + title + year). No hover feedback, no metadata-at-a-glance, no loading states. This makes the UI feel like a prototype rather than a polished media center experience. Infuse, Plex, and Jellyfin all provide rich card interactions that communicate metadata and state visually.

## Proposed Solution

Four layers of polish, implementable in phases:

### Phase 1: Hover Effects (CSS-only, no Rust changes)

Pure CSS hover treatment on the poster frame area (not the entire card — matches Infuse behavior where only the poster animates, title/year stay still).

```css
.media-card-frame {
    transition: transform 150ms cubic-bezier(0.25, 0.46, 0.45, 0.94),
                box-shadow 150ms cubic-bezier(0.25, 0.46, 0.45, 0.94);
}

.media-card-frame:hover {
    transform: scale(1.03);
    box-shadow: 0 0 0 1px rgba(0, 0, 6, 0.05),
                0 4px 8px 2px rgba(0, 0, 6, 0.12),
                0 8px 16px 4px rgba(0, 0, 6, 0.08);
}

.media-card-frame:active {
    transform: scale(0.97);
}
```

**Why scale only the poster frame:** Scaling the entire card (poster + title + year) causes title ellipsis to change and spacing between cards to look uneven. Infuse scales only the artwork. GTK4 CSS `transform` is purely visual and doesn't affect layout allocation, so adjacent cards won't shift.

**Why CSS `:hover` not EventControllerMotion:** GTK4's `:hover` pseudo-class is tracked by the toolkit based on pointer position — no manual class management needed, no recycling bugs. Use EventControllerMotion only if we need programmatic effects later.

### Phase 2: Skeleton Loading + Poster Fade-in (CSS + minimal Rust)

**Shimmer placeholder:**

Add a `.loading` CSS class to the poster when no texture is loaded. Remove it when texture arrives.

```css
@keyframes shimmer {
    0% { background-position: -200% 0; }
    100% { background-position: 200% 0; }
}

.media-card-poster.loading {
    background-image: linear-gradient(
        90deg,
        alpha(@card_bg_color, 0.3) 0%,
        alpha(@card_bg_color, 0.6) 50%,
        alpha(@card_bg_color, 0.3) 100%
    );
    background-size: 200% 100%;
    animation: shimmer 1.5s ease-in-out infinite;
}
```

**Poster fade-in:**

When the texture arrives, set it on the Picture widget and use a CSS opacity transition:

```css
.media-card-poster {
    opacity: 1;
    transition: opacity 200ms ease-out;
}
```

In Rust, set the Picture's opacity to 0 when no texture, then to 1 when texture arrives. The CSS transition handles the animation. Alternatively, use `adw::TimedAnimation` with `PropertyAnimationTarget::new(&picture, "opacity")` for more control.

**Failure state:** If artwork download fails, remove `.loading` class and show a fallback icon (`video-x-generic-symbolic`) or just the static background color. Do not let shimmer run indefinitely.

**Already-cached optimization:** In `bind()`, if `poster_texture` is `Some`, set opacity to 1 immediately and skip the `.loading` class — no shimmer for already-cached textures.

### Phase 3: Metadata Badges (Rust widget restructuring + CSS)

Restructure the card widget to use `gtk4::Overlay` for stacking badges on the poster.

**Widget hierarchy change:**

```
Before:
  Box.media-card
    Frame.media-card-frame
      Picture.media-card-poster
    Label.media-card-title
    Label.media-card-year

After:
  Box.media-card
    Frame.media-card-frame
      Overlay
        Picture.media-card-poster          (main child)
        Label.media-badge.resolution-badge (overlay, top-right)
        Label.media-badge.rating-badge     (overlay, top-left)
        ProgressBar.watch-progress         (overlay, bottom — hidden until M4)
    Label.media-card-title
    Label.media-card-year
```

**Badge design (Infuse-inspired):**

```css
.media-badge {
    background-color: alpha(black, 0.7);
    color: white;
    font-size: 10px;
    font-weight: bold;
    padding: 2px 6px;
    border-radius: 4px;
    min-height: 0;
}

.resolution-badge {
    /* e.g., "4K", "1080p", "720p" */
}

.rating-badge {
    /* e.g., "8.2" — numeric Plex rating */
}
```

**Badge positioning:**

```rust
let resolution_badge = gtk4::Label::builder()
    .halign(gtk4::Align::End)
    .valign(gtk4::Align::Start)
    .margin_top(8)
    .margin_end(8)
    .css_classes(["media-badge", "resolution-badge"])
    .visible(false)
    .build();

let rating_badge = gtk4::Label::builder()
    .halign(gtk4::Align::Start)
    .valign(gtk4::Align::Start)
    .margin_top(8)
    .margin_start(8)
    .css_classes(["media-badge", "rating-badge"])
    .visible(false)
    .build();
```

**Data pipeline changes:**

1. Add `video_resolution: Option<String>` to `MediaItem` (`src/models/media.rs`)
2. Propagate from `PlexMedia.video_resolution` in `plex_metadata_to_media_item()` (`src/services/plex/convert.rs`) — take from `metadata.media.first()`
3. Add `video_resolution: Option<String>` and `rating: Option<f64>` to `MediaCardData` (`src/components/library/media_card.rs`)
4. Copy these fields in `MediaCardData::from_media_item()`

**Badge visibility rules (in `bind()`):**

- Resolution badge: visible only when `video_resolution.is_some()` AND `poster_texture.is_some()` (no badges on empty placeholders)
- Rating badge: visible only when `rating.is_some()` AND rating > 0.0 AND `poster_texture.is_some()`
- Format resolution display: "4k" → "4K", "1080" → "1080p", "720" → "720p", "480" → "SD"
- Format rating display: one decimal place (e.g., `8.0`, `7.5`)

**TV Shows:** Plex show-level metadata has no `Media[]` array, so `video_resolution` will be `None`. Badges are simply hidden — no special handling needed.

### Phase 4: Watch Progress Bar (Infrastructure only — M4 provides data)

Add the `ProgressBar` widget to the overlay now, but keep it hidden (`visible: false`) until M4 implements watch state tracking.

```css
.watch-progress {
    min-height: 3px;
    margin: 0;
}

.watch-progress trough {
    min-height: 3px;
    border-radius: 0 0 8px 8px; /* match bottom corners of poster frame */
    background-color: transparent;
}

.watch-progress progress {
    min-height: 3px;
    border-radius: 0 0 8px 8px;
    background-color: @accent_color;
}
```

Position: `halign: Fill`, `valign: End`, inside the `Overlay`. The `Frame`'s `overflow: hidden` and `border-radius: 8px` should clip the bar to the rounded bottom corners.

## Technical Considerations

### GTK4 CSS Transform Behavior

- `transform: scale()` is purely visual — does NOT affect layout allocation
- Adjacent cards do not shift when a card is hovered
- Small scale values (1.02–1.05) work well; 1.03 is the sweet spot
- `transform-origin` defaults to widget center, which is correct for poster lift

### Widget Recycling (TypedGridView)

- GTK4's `:hover` pseudo-class is managed by the toolkit based on pointer position — it handles recycling correctly automatically
- Badge visibility and content MUST be fully reset in `bind()` (set visibility + label text for every badge, every time)
- The `.loading` class must be set/cleared in `bind()` based on `poster_texture` presence

### Performance

- CSS `@keyframes` shimmer runs on GPU compositor — performant even with 20-30 visible cards
- Shimmer stops when poster loads (`.loading` class removed) so only initially-loading cards animate
- `transform` and `box-shadow` transitions are GPU-accelerated in GTK4
- No new async operations — artwork loading is already async

### Accessibility

- Badge labels are accessible to screen readers by default (they're real `gtk4::Label` widgets)
- `:hover` effects don't fire on touch — `:active` provides tap feedback
- GTK4 respects `gtk-enable-animations` setting — CSS transitions/animations are automatically disabled when the user has reduce-motion enabled
- Focus indicators (keyboard navigation) are managed by libadwaita and work independently of hover CSS

## Acceptance Criteria

### Phase 1: Hover
- [x] Poster frame scales to 1.03 on hover with 150ms transition (`src/style.css`)
- [x] Shadow deepens on hover, creating a "lift" effect
- [x] Active/press state scales to 0.97
- [x] Transition uses `cubic-bezier(0.25, 0.46, 0.45, 0.94)` (libadwaita's standard curve)
- [ ] No visual glitches when scrolling through hovered cards

### Phase 2: Loading States
- [x] Cards without loaded textures show shimmer animation (`src/style.css`, `media_card.rs`)
- [x] Shimmer stops and poster fades in (200ms) when texture arrives
- [x] Already-cached textures appear immediately with no shimmer
- [x] Failed artwork downloads show static placeholder (no infinite shimmer)

### Phase 3: Metadata Badges
- [x] Card widget uses `gtk4::Overlay` for badge stacking (`media_card.rs`)
- [x] Resolution badge (top-right): "4K", "1080p", "720p", "SD" (`media_card.rs`, `media.rs`, `convert.rs`)
- [x] Rating badge (top-left): e.g., "8.2" (`media_card.rs`)
- [x] Badges hidden when data missing or no poster loaded
- [x] `MediaItem` has `video_resolution` field, populated from Plex API
- [x] `MediaCardData` carries `video_resolution` and `rating`

### Phase 4: Watch Progress (infra only)
- [x] `ProgressBar` widget exists in overlay, hidden by default (`media_card.rs`, `src/style.css`)
- [x] CSS styles for thin (3px) progress bar with accent color
- [x] Clipped to bottom rounded corners of poster frame

## Dependencies & Risks

| Risk | Mitigation |
|------|-----------|
| GTK4 `transform: scale()` clips at parent bounds | Keep scale factor small (1.03); verify clipping with `overflow: visible` on frame if needed |
| Shimmer CSS animation performance | Test with full library grid (500+ items); shimmer only runs on visible, unloaded cards |
| Badge overlap at small poster widths (10-column grid) | Test at `max_columns(10)`; hide badges if poster width < 120px via min-width check in `bind()` |
| `PlexMedia.video_resolution` not always populated | Graceful degradation — badge simply hidden when `None` |
| Progress bar corner clipping | Frame's `overflow: hidden` + `border-radius` should handle this; verify in testing |

## Key Files

| File | Changes |
|------|---------|
| `src/style.css` | Hover transitions, shimmer keyframes, badge styles, progress bar styles |
| `src/components/library/media_card.rs` | Overlay restructuring, badge widgets, loading class management |
| `src/components/library/mod.rs` | Update `ArtworkReady` handler for fade-in (remove `.loading` class) |
| `src/models/media.rs` | Add `video_resolution: Option<String>` to `MediaItem` |
| `src/services/plex/convert.rs` | Propagate resolution from `PlexMedia` to `MediaItem` |

## Sources & References

- [GTK4 CSS Properties (animatable flags)](https://docs.gtk.org/gtk4/css-properties.html)
- [libadwaita card styles (_misc.scss)](https://github.com/GNOME/libadwaita/blob/main/src/stylesheet/widgets/_misc.scss) — reference for `.card.activatable` hover patterns
- [AdwTimedAnimation API](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/class.Animation.html)
- [GTK4 CSS overview](https://docs.gtk.org/gtk4/css-overview.html)
- [Infuse for Apple TV](https://firecore.com/infuse) — visual reference for card hover behavior
