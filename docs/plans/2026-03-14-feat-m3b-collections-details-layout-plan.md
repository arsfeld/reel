---
title: "M3b: Collections, Enhanced Details, List View, Grid Density, Adaptive Layout"
type: feat
status: completed
date: 2026-03-14
---

# M3b: Collections, Enhanced Details, List View, Grid Density, Adaptive Layout

## Overview

Complete the remaining M3 Library UX tasks: Plex collections browsing, enriched detail pages (cast, crew, technical info), list view mode, configurable grid density, and adaptive layout polish. These build on the M3a search/filter/sort foundation and require extending the Plex data layer first.

## Problem Statement

With search/filter/sort complete (M3a), the library is navigable but detail pages are sparse (no cast, no technical info, no collection context) and there is no way to browse Plex collections. Users also lack view customization -- no list view alternative and fixed poster sizes. These features are what separate a functional browser from the rich "Infuse for Linux" experience.

## Proposed Solution

### Key Design Decisions

1. **`MediaDetail` struct**: Create a new `MediaDetail` type returned by `metadata()` that wraps `MediaItem` with optional extended fields (cast, credits, technical info, collections). This keeps `MediaItem` lightweight for grid views while giving detail pages rich data.

2. **Collections in sidebar**: Add "Collections" as a third sidebar entry (after Movies, TV Shows). Each library type has its own collections. A dedicated `CollectionDetail` component shows collection items in a simple grid.

3. **View preferences**: Extend `WindowState` TOML with `view_mode` ("grid"/"list") and `grid_density` ("small"/"medium"/"large"). Global, not per-library.

4. **Grid density values**: Small=120x180, Medium=180x270 (current), Large=240x360.

## Technical Approach

### Architecture

```
Plex Serde Layer          Service Layer              UI Layer
─────────────────         ─────────────              ────────
PlexMetadata              MediaDetail                MovieDetail (cast, tech, credits)
  + Role[]         →      + cast: Vec<CastMember>    ShowDetail (thumbnails, descriptions)
  + Director[]             + directors: Vec<String>   CollectionDetail (poster grid)
  + Writer[]               + writers: Vec<String>     LibraryView (list mode, density)
  + Collection[]           + technical: TechnicalInfo
  + PlexMedia fields       + collections: Vec<...>
PlexClient
  + collections()   →     MediaSource trait           Sidebar (+ Collections entry)
  + collection_items()     + collections()
                           + collection_items()
```

### New Files

| File | Purpose |
|------|---------|
| `src/models/detail.rs` | `MediaDetail`, `CastMember`, `TechnicalInfo` types |
| `src/components/detail/cast_row.rs` | Horizontal scrollable cast section |
| `src/components/detail/collection_detail.rs` | Collection detail page (poster grid) |
| `src/components/library/list_row.rs` | List view row item (RelmListItem) |

### Modified Files

| File | Changes |
|------|---------|
| `src/services/plex/models.rs` | Add `PlexRole`, tech fields to `PlexMedia`, `Collection` + `Director` + `Writer` + `parentThumb` to `PlexMetadata` |
| `src/services/plex/convert.rs` | Convert new fields to `MediaDetail` |
| `src/services/plex/api.rs` | Add `collections()` and `collection_items()` methods |
| `src/services/media_source.rs` | Add `collections()`, `collection_items()` methods; change `metadata()` return to `MediaDetail` |
| `src/services/plex/source.rs` | Implement new `MediaSource` methods |
| `src/models/media.rs` | Add `MediaType::Collection` variant |
| `src/navigation.rs` | Add `CurrentView::CollectionDetail(String)` |
| `src/components/sidebar.rs` | Add "Collections" entry |
| `src/components/detail/movie_detail.rs` | Add cast section, credits, technical info, collection links |
| `src/components/detail/show_detail.rs` | Add episode thumbnails, descriptions, season artwork |
| `src/components/library/mod.rs` | Add list/grid toggle, density control, collection loading |
| `src/components/library/media_card.rs` | Parameterize card dimensions based on density |
| `src/app.rs` | Handle collection navigation, forward density/view changes |
| `src/services/window_state.rs` | Add `view_mode` and `grid_density` fields |
| `src/style.css` | Cast card styles, list row styles, density variants |

### Implementation Phases

#### Phase 1: Plex Data Layer Extensions

**Goal**: Extend Plex serde models, conversion, and API client so all data is available.

**Plex models (`models.rs`)**:

```rust
// New struct for cast members (richer than PlexTag)
#[derive(Debug, Clone, Deserialize)]
pub struct PlexRole {
    pub tag: String,           // Actor name
    pub role: Option<String>,  // Character name
    pub thumb: Option<String>, // Photo URL
}

// Extend PlexMetadata with:
pub struct PlexMetadata {
    // ... existing fields ...
    #[serde(rename = "Role", default)]
    pub roles: Vec<PlexRole>,
    #[serde(rename = "Director", default)]
    pub directors: Vec<PlexTag>,
    #[serde(rename = "Writer", default)]
    pub writers: Vec<PlexTag>,
    #[serde(rename = "Collection", default)]
    pub collections: Vec<PlexTag>,
    #[serde(rename = "parentThumb")]
    pub parent_thumb: Option<String>,
}

// Extend PlexMedia with:
pub struct PlexMedia {
    // ... existing Part field ...
    #[serde(rename = "videoResolution")]
    pub video_resolution: Option<String>,
    #[serde(rename = "videoCodec")]
    pub video_codec: Option<String>,
    #[serde(rename = "audioCodec")]
    pub audio_codec: Option<String>,
    #[serde(rename = "audioChannels")]
    pub audio_channels: Option<i32>,
    pub bitrate: Option<i64>,
    pub container: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}
```

**PlexClient (`api.rs`)**:

```rust
// Fetch collections for a library section
pub async fn collections(&self, library_key: &str) -> Result<Vec<PlexMetadata>, PlexError>
// GET /library/sections/{key}/collections

// Fetch items in a collection
pub async fn collection_items(&self, collection_key: &str) -> Result<Vec<PlexMetadata>, PlexError>
// GET /library/collections/{key}/children
```

**Tests**: Wiremock tests for both new endpoints + serde tests for new fields with fixture JSON. Test that existing JSON without the new fields still deserializes correctly (serde `#[serde(default)]`).

**Success criteria**: All existing 365 tests pass + ~15 new tests for serde + ~6 new wiremock tests.

#### Phase 2: MediaDetail Model + Conversion

**Goal**: Create the `MediaDetail` type and update the conversion layer.

**New types (`models/detail.rs`)**:

```rust
pub struct CastMember {
    pub name: String,
    pub character: Option<String>,
    pub photo_path: Option<String>, // Plex thumb path
}

pub struct TechnicalInfo {
    pub video_resolution: Option<String>, // "1080p", "4K"
    pub video_codec: Option<String>,      // "H.264", "HEVC"
    pub audio_codec: Option<String>,      // "AAC", "DTS"
    pub audio_channels: Option<i32>,      // 2, 6, 8
    pub container: Option<String>,        // "MKV", "MP4"
    pub bitrate_kbps: Option<i64>,
    pub file_size_bytes: Option<i64>,
}

pub struct CollectionRef {
    pub name: String,
}

pub struct MediaDetail {
    pub item: MediaItem,
    pub cast: Vec<CastMember>,
    pub directors: Vec<String>,
    pub writers: Vec<String>,
    pub technical: Option<TechnicalInfo>,
    pub collections: Vec<CollectionRef>,
}
```

**Conversion (`convert.rs`)**: Add `plex_metadata_to_media_detail()` that builds `MediaDetail` from `PlexMetadata`.

**MediaSource trait**: Change `metadata()` return type from `MediaItem` to `MediaDetail`. Add `collections()` and `collection_items()` with default implementations returning `Err(SourceError::NotSupported)`.

**Tests**: Unit tests for conversion of cast, credits, technical info, collections. Edge cases: empty cast, missing technical fields, multiple collections.

**Success criteria**: ~15 new conversion tests pass.

#### Phase 3: Enhanced Movie Detail Page

**Goal**: Add cast, credits, technical info, and collection membership to the movie detail page.

**Layout (top to bottom)**:
```
┌─────────────────────────────────────────┐
│ ← Movie Title                    Header │
├─────────────────────────────────────────┤
│ [Backdrop image]                        │
├─────────────────────────────────────────┤
│ Title                                   │
│ 2021 · 2h 35m · 8.0★ · PG-13          │
│ Directed by Denis Villeneuve            │  ← NEW
│ Written by Jon Spaihts, Denis V.        │  ← NEW
│ [Sci-Fi] [Adventure]                    │
│ [▶ Play]                               │
│ Synopsis text...                        │
├─────────────────────────────────────────┤
│ Cast                                    │  ← NEW
│ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐       │
│ │photo│ │photo│ │photo│ │photo│  →→    │ horizontal scroll
│ │Name │ │Name │ │Name │ │Name │        │
│ │Role │ │Role │ │Role │ │Role │        │
│ └─────┘ └─────┘ └─────┘ └─────┘       │
├─────────────────────────────────────────┤
│ Technical Info                          │  ← NEW
│ 1080p · H.264 · AAC 5.1 · MKV · 12 GB │
├─────────────────────────────────────────┤
│ Part of: Dune Collection ›              │  ← NEW (clickable)
└─────────────────────────────────────────┘
```

**Cast section**: Horizontal `ScrolledWindow` containing a `gtk4::Box` (horizontal) of cast cards. Each card: `Picture` (80x80 circle or rounded), name label, character label. Lazy-load photos via `ArtworkCache`.

**Credits**: Simple labels below the year/runtime row. Hidden if empty.

**Technical info**: Single row of dim labels. Hidden if no data.

**Collection links**: Buttons styled as links. Clicking navigates to `CollectionDetail`.

**Edge cases**: Hide sections when data is empty (no cast → no cast section). Show "No cast information" only if the item type is Movie and metadata was fetched but Role was empty.

**Success criteria**: Movie detail shows all new sections. Empty data handled gracefully.

#### Phase 4: Enhanced TV Show Detail Page

**Goal**: Add episode thumbnails, descriptions, season artwork.

**Changes**:
- **Episode rows**: Replace `AdwActionRow` with a richer custom row that includes a `Picture` (episode thumbnail, ~160x90) on the left, title + air_date in the middle, and description below.
- **Episode descriptions**: Show episode `overview` as a second line or expandable text below the title.
- **Season artwork**: Display season `poster_path` as a small image near the season dropdown.
- **Show backdrop**: Add a backdrop `Picture` at the top (like movie detail).

**Layout**:
```
┌─────────────────────────────────────────┐
│ [Show backdrop image]                   │  ← NEW
├─────────────────────────────────────────┤
│ Show Title                              │
│ 2022 · 8.5★ · TV-MA                    │
│ Synopsis text...                        │
├─────────────────────────────────────────┤
│ Season [▾ Season 1]  [season artwork]   │  ← artwork NEW
├─────────────────────────────────────────┤
│ ┌────────┬──────────────────────┬─────┐ │
│ │thumb   │ 1. Episode Title     │ ▶   │ │  ← thumb + desc NEW
│ │        │ Mar 1, 2024          │     │ │
│ │        │ Episode description..│     │ │
│ └────────┴──────────────────────┴─────┘ │
│ ┌────────┬──────────────────────┬─────┐ │
│ │thumb   │ 2. Episode Title     │ ▶   │ │
```

**Photo loading**: Episode thumbnails loaded via `ArtworkCache` as they appear. Placeholder shown while loading.

**Success criteria**: Episodes show thumbnails and descriptions. Season artwork visible. Show has backdrop.

#### Phase 5: Collections View

**Goal**: Browse Plex collections from sidebar, view collection detail page.

**Sidebar**: Add "Collections" as a third `ListBoxRow` after Movies and TV Shows. Clicking it emits `SidebarOutput::Navigate(LibraryType::Collection)` (new variant) or a new `SidebarOutput::ShowCollections`.

**Navigation**: `AppMsg::ShowCollections` → fetch collections from `source.collections(library_key)` → display a grid of collection cards (use poster_path as collection poster). Clicking a collection card → push `CollectionDetail` onto `nav_view`.

**CollectionDetail component**: Similar to `LibraryView` but simpler — no search/filter/sort bar. Just a poster grid of collection items in the server's order, with a header showing collection name and item count.

**Navigation flow**:
```
Sidebar "Collections" → App fetches collections → CollectionList (grid of collection posters)
Click collection → App pushes CollectionDetail → Collection items grid
Click item → App pushes MovieDetail or ShowDetail
```

**Alternative approach**: Instead of a sidebar entry, show a "Collections" section at the top of the library view. This avoids sidebar changes but complicates the library view. Recommend sidebar entry for cleanliness.

**Success criteria**: Collections browsable from sidebar. Collection items displayed. Navigation to/from works.

#### Phase 6: List View Toggle

**Goal**: Toggle between poster grid and compact list view.

**Toggle button**: Add a segmented button (grid icon / list icon) in the library toolbar header bar, next to the settings button.

**List view**: Use `TypedListView<ListRowData, gtk4::SingleSelection>` where `ListRowData` renders as:
- Small poster thumbnail (60x90) on the left
- Title (primary label)
- Subtitle: year · runtime · rating★

**Shared state**: Both grid and list modes share the same `all_items`, `search_query`, `filter_state`, `sort_order`. Toggling just swaps which widget is visible in the stack and rebuilds.

**Persistence**: Save `view_mode: "grid"` or `view_mode: "list"` in `WindowState` TOML.

**Success criteria**: Toggle switches between grid and list. Filter/sort apply to both. Preference persists.

#### Phase 7: Grid Density Control

**Goal**: Small/medium/large poster sizes in grid view.

**Control**: Add a density selector (three-button segmented control or a dropdown) near the grid/list toggle.

**Density values**:
| Density | Card Width | Card Height | min_columns |
|---------|-----------|-------------|-------------|
| Small | 120 | 180 | 4 |
| Medium | 180 | 270 | 3 |
| Large | 240 | 360 | 2 |

**Implementation**: `MediaCardData::setup()` currently hardcodes 180x270. Change to read from a shared density state. Since `TypedGridView` factories share a single setup, changing density requires clearing and re-creating the grid (same as the rebuild_grid pattern).

**Persistence**: Save `grid_density: "medium"` in `WindowState` TOML.

**Success criteria**: Three density levels work. Grid reflows correctly. Preference persists.

#### Phase 8: Adaptive Layout Polish

**Goal**: Fine-tune responsive behavior with `AdwBreakpoint`.

**Already working**:
- `AdwNavigationSplitView` collapses sidebar on narrow windows
- `GridView` auto-adjusts columns via min_columns/max_columns
- `AdwClamp` constrains detail page width

**Improvements**:
- Detail pages: Stack credits/technical info vertically below 600px width
- Cast section: Reduce cast card size on narrow windows
- Grid density: Auto-switch to "small" on very narrow windows

**Implementation**: Add `AdwBreakpoint` to the `AdwApplicationWindow` with setters that modify widget properties at width thresholds. Example:
```rust
let breakpoint = adw::Breakpoint::new(
    adw::BreakpointCondition::parse("max-width: 600px").unwrap()
);
breakpoint.add_setter(&some_widget, "visible", &false.into());
root.add_breakpoint(breakpoint);
```

**Success criteria**: App looks good at 800px, 1280px, and 1920px widths. No visual breakage at any reasonable size.

## Alternative Approaches Considered

### 1. Extend MediaItem Instead of Creating MediaDetail (Rejected)

**Approach**: Add cast, credits, technical info directly to `MediaItem`.

**Why rejected**: `MediaItem` is used everywhere — grid views, list views, filters, database. Making it heavier with Vec<CastMember> and TechnicalInfo adds memory overhead for contexts that never use those fields. The `MediaDetail` wrapper is cleaner: `MediaItem` stays lean, detail pages get rich data.

### 2. Collections as In-Library Section (Rejected for MVP)

**Approach**: Show collections as a horizontal banner at the top of the library grid.

**Why rejected**: This mixes collection browsing with regular browsing and complicates the library view which already has search/filter/sort. A sidebar entry is cleaner and matches the Plex/Infuse UX pattern.

### 3. ColumnView for List Mode (Considered)

**Approach**: Use `gtk4::ColumnView` with sortable columns (like a spreadsheet).

**Why rejected for now**: `ColumnView` is more complex to set up and doesn't have Relm4 typed wrapper support. `TypedListView` is simpler and provides the compact row UX needed. `ColumnView` could be a future enhancement.

## System-Wide Impact

### Interaction Graph

1. User clicks "Collections" in sidebar → `SidebarOutput::ShowCollections` → `App` fetches collections → pushes `CollectionList` page
2. User clicks collection → `App` fetches collection items → pushes `CollectionDetail` page
3. User opens movie detail → `App` calls `source.metadata(key)` → gets `MediaDetail` → `MovieDetail` renders cast, credits, tech, collection links
4. User clicks collection link on movie → `App` pushes `CollectionDetail` for that collection
5. User toggles grid/list → `LibraryView` swaps visible widget, rebuilds from same `all_items`
6. User changes density → `LibraryView` adjusts card size, rebuilds grid

### Error & Failure Propagation

- `collections()` network failure → toast "Failed to load collections" → show empty state
- Cast photo fetch failure → show placeholder silhouette (silent, no toast)
- `metadata()` returning `MediaDetail` with empty cast → hide cast section (not an error)
- Collection with 0 items → show empty state "This collection is empty"

### State Lifecycle Risks

- **View mode toggle during loading**: If library is loading and user toggles grid/list, the loaded data should apply to the current mode. The `LibraryLoaded` handler already calls `rebuild_grid` which respects current mode.
- **Density change during loading**: Same pattern — rebuild on load respects current density.
- **Collection items from different library types**: A Plex collection can span movies and shows. The detail page should handle both `MediaType::Movie` and `MediaType::Show` items.

### API Surface Parity

- `MediaSource` trait gains 2 methods: `collections()`, `collection_items()`
- `MediaSource::metadata()` return type changes from `MediaItem` to `MediaDetail`
- `PlexSource` implements all new methods
- Future `JellyfinSource`, `LocalSource` get default `NotSupported` implementations
- `SidebarOutput` gains `ShowCollections` variant
- `AppMsg` gains `ShowCollections`, `ShowCollectionDetail(MediaItem)`
- `CurrentView` gains `CollectionDetail(String)`

### Integration Test Scenarios

1. **Full collection flow**: Load collections → click collection → see items → click movie → see movie detail with cast → press back → back to collection → back to collection list
2. **MediaDetail enrichment**: Fetch movie metadata → verify cast, credits, technical info present → verify empty fields hidden
3. **View mode round-trip**: Toggle to list → apply filter → verify filter works in list mode → toggle back to grid → filter still active
4. **Density persistence**: Change to "large" → close app → reopen → verify density is "large"

## Acceptance Criteria

### Functional Requirements

- [x] **Collections browse**: "Collections" sidebar entry shows collections for the current Plex library
- [x] **Collection detail**: Clicking a collection shows its items in a poster grid
- [x] **Collection navigation**: Can navigate collection → item detail → back
- [x] **Movie cast**: Movie detail shows horizontal scrollable cast list with photos and character names
- [x] **Movie credits**: Movie detail shows director and writer names
- [x] **Movie technical**: Movie detail shows resolution, codec, audio, container, file size
- [x] **Movie collection link**: Movie detail shows collection membership as clickable link
- [x] **TV episode thumbnails**: Episode rows show thumbnail images
- [x] **TV episode descriptions**: Episode rows show description text
- [x] **TV season artwork**: Season artwork displayed near season selector
- [ ] **List view toggle**: Button toggles between poster grid and compact list view (deferred — GridDensity covers the UX need)
- [ ] **List view works**: List shows title, year, rating, runtime per row with small poster (deferred)
- [x] **Grid density**: Three density levels (small/medium/large) change poster card size
- [x] **Density reflow**: Grid column count adjusts with density change
- [x] **View persistence**: Grid/list preference and density persist across app restarts
- [x] **Adaptive sidebar**: Sidebar collapses on narrow windows (AdwNavigationSplitView, verified)
- [x] **Adaptive grid**: Grid columns adjust to window width (min_columns/max_columns, verified)

### Non-Functional Requirements

- [x] **Test coverage**: 4 serde tests + 3 wiremock tests + 7 conversion tests + 11 detail model tests = 25 new tests
- [x] **Empty data graceful**: Cast/credits/technical sections hidden when data empty
- [x] **Clippy clean**: Zero warnings
- [x] **Existing tests pass**: All 390 tests pass

### Quality Gates

- [x] All tests pass: `nix develop -c cargo test`
- [x] No clippy warnings: `nix develop -c cargo clippy`
- [x] Formatted: `nix develop -c cargo fmt --check`
- [x] Compiles with zero warnings

## Success Metrics

- Collections browsable in 1-2 clicks from any library view
- Movie detail pages show cast, credits, and technical info when available
- TV show episodes show thumbnails and descriptions
- Grid/list toggle works seamlessly with existing search/filter/sort
- App looks correct at common window sizes (800px, 1280px, 1920px)

## Dependencies & Prerequisites

| Dependency | Status | Impact |
|-----------|--------|--------|
| M3a search/filter/sort | Done | Grid rebuild pattern reused for list/density |
| Plex API collections endpoint | Available | Standard Plex endpoint |
| Plex metadata cast/crew fields | Available | Already returned by Plex, just not parsed |
| libadwaita AdwBreakpoint | Available (v1_4) | Already enabled in Cargo.toml |
| TypedListView from relm4 | Available | For list view mode |

## Risk Analysis & Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| PlexMetadata serde changes break existing parsing | Low | High | All new fields use `#[serde(default)]`; run all existing tests |
| Cast photo loading burst overwhelms Plex server | Medium | Medium | Lazy load as cast cards scroll into view; limit concurrent requests |
| MediaSource trait change breaks compilation | Certain | Low | Use default method implementations returning NotSupported |
| TypedListView factory setup is unfamiliar | Medium | Low | Follow same pattern as TypedGridView; Relm4 docs cover this |
| AdwBreakpoint behavior varies across GTK versions | Low | Low | Only use well-documented breakpoint conditions (max-width) |

## Future Considerations

- **Person detail page**: Click cast member → see their filmography within the library (M7)
- **Collection sorting**: Allow user to re-sort collection items locally (M5 polish)
- **Multiple media versions**: Show version picker when a movie has multiple files (M5)
- **Search across cast/crew**: Extend search to match cast names (requires FTS5)
- **ColumnView**: More powerful table-style list with sortable columns

## Sources & References

### Internal References

- Tech.md component hierarchy: `tech.md:185-202` (planned CastRow, CollectionStrip, EpisodeList)
- Tech.md database schema: `tech.md:854-867` (people, media_people tables)
- Tech.md MediaSource trait: `tech.md:257-305` (planned collections, search methods)
- Product spec detail pages: `product.md:199-228` (F3.1-F3.3)
- Product spec responsive: `product.md:255-259` (F4.4)
- M3a plan: `docs/plans/2026-03-14-feat-m3-library-search-filter-sort-plan.md`
- Current movie detail: `src/components/detail/movie_detail.rs`
- Current show detail: `src/components/detail/show_detail.rs`
- Current Plex models: `src/services/plex/models.rs`
- Plex API client: `src/services/plex/api.rs`
- Window state persistence: `src/services/window_state.rs`
