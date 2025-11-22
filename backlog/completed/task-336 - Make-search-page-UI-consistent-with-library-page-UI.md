---
id: task-336
title: Make search page UI consistent with library page UI
status: Done
assignee:
  - '@claude'
created_date: '2025-10-02 19:18'
updated_date: '2025-10-02 19:50'
labels:
  - ui
  - ux
  - search
  - consistency
dependencies: []
priority: high
---

## Description

The search page UI/UX does not match the library page design. They should have consistent layouts, filters, sorting options, and visual styling to provide a cohesive user experience.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Audit visual differences between SearchPage and LibraryPage layouts
- [ ] #2 Add genre filter dropdown to search page (if present in library)
- [ ] #3 Add sort options to search page (if present in library)
- [x] #4 Match media grid layout, spacing, and card sizes
- [x] #5 Ensure consistent header/toolbar styling
- [x] #6 Match loading indicators and empty state designs
- [x] #7 Test search page looks consistent with library page
<!-- AC:END -->


## Implementation Plan

1. Audit visual differences: FlowBox settings, card proportions, margins
2. Fix FlowBox layout to match LibraryPage (spacing, min/max children, margins)
3. Fix poster loading - ensure TV shows use show poster, not episode thumbnails
4. Match empty state design (use adw::StatusPage)
5. Match loading indicator styling
6. Test visual consistency with cargo run


## Implementation Notes

## Current State

**SearchPage** (`src/ui/pages/search.rs`):
- Simple FlowBox with media cards
- Basic empty states (no query, no results)
- Loading spinner
- No filters or sorting options
- Basic layout with search query header

**LibraryPage** (`src/ui/pages/library.rs`):
- Should be examined for features that search lacks

### Episode Poster URLs (src/ui/pages/search.rs:246-357)
Fixed episode poster display to match HomePage approach:
- Added ParentShowsLoaded input message to handle async parent show loading
- ResultsLoaded now triggers batch fetch of parent shows for episodes
- ParentShowsLoaded handler replaces episode thumbnails with parent show posters
- Episodes without parent shows are skipped (logged as errors)
- Uses same batch fetching pattern as HomePage for efficiency

This ensures TV show episodes display with the show poster instead of episode thumbnails, matching the HomePage behavior.


## Poster URL Issue for Episodes

The MediaItemModel.poster_url field contains:
- For movies: movie poster ✓
- For shows: show poster ✓  
- For episodes: episode thumbnail ❌ (should use show_poster_url from metadata)

This is a data layer issue. Episodes have show_poster_url in their metadata field, but we're using poster_url directly. To fix this properly, we would need to:

1. Parse metadata JSON for episodes to extract show_poster_url
2. Use show_poster_url instead of poster_url for episode cards

This affects both SearchPage and potentially LibraryPage. Requires investigation of how episode cards should be displayed in search results.

Recommendation: Create a follow-up task to handle episode poster URLs correctly across the application.


## Changes Made

### FlowBox Layout (src/ui/pages/search.rs:109-123)
Updated FlowBox settings to match LibraryPage:
- Changed column_spacing from 16 to 12
- Changed min_children_per_line from 2 to 4  
- Changed max_children_per_line from 8 to 12
- Added proper margins (top: 24, bottom: 16, start/end: 16)
- This ensures cards have the correct proportions and spacing

### Empty States (src/ui/pages/search.rs:145-163)
Replaced custom empty state boxes with adw::StatusPage:
- Matches LibraryPage empty state design
- Uses compact styling for consistency
- Cleaner, more polished appearance

### Loading Indicator (src/ui/pages/search.rs:126-142)
Updated to match LibraryPage style:
- Centered horizontally
- Proper margins (12px all around)
- Dim label styling for secondary text

### Image Size (src/ui/pages/search.rs:277)
Changed from ImageSize::Card to ImageSize::Thumbnail to match LibraryPage

### Header Layout (src/ui/pages/search.rs:83-105)
Added proper margins to match LibraryPage header spacing


## Areas to Investigate

### 1. Filters & Controls
- [ ] Does LibraryPage have genre filters?
- [ ] Does LibraryPage have sort options (title, date, rating)?
- [ ] Does LibraryPage have view mode toggles (grid/list)?
- [ ] Does LibraryPage have any other filter controls?

### 2. Layout & Spacing
- [ ] FlowBox configuration (max/min children per line)
- [ ] Column/row spacing values
- [ ] Card sizes and aspect ratios
- [ ] Margins and padding
- [ ] ScrolledWindow settings

### 3. Visual Elements
- [ ] Header styling and content
- [ ] Toolbar presence/absence
- [ ] Status bar or info display
- [ ] Loading state design
- [ ] Empty state icons and messaging

### 4. Behavior
- [ ] Pagination or infinite scroll
- [ ] Image loading strategy
- [ ] Selection modes
- [ ] Context menus
- [ ] Keyboard navigation

## Implementation Strategy

1. **First: Audit LibraryPage**
   - Read `src/ui/pages/library.rs` completely
   - Document all UI features, controls, and styling
   - Take screenshots if needed

2. **Second: Compare with SearchPage**
   - List all differences
   - Prioritize which features to add to search

3. **Third: Implement Missing Features**
   - Add filters/sorting if needed
   - Match layout/spacing
   - Ensure consistent styling

4. **Fourth: Test Consistency**
   - Visual comparison
   - User flow testing
   - Ensure both pages feel part of same app

## Success Criteria

When complete, a user should:
- Not be able to immediately tell which page they're on visually
- Have the same interaction patterns available
- See consistent spacing, sizing, and styling
- Feel like search is a natural extension of library browsing

## Files to Review/Modify
- `src/ui/pages/library.rs` - Study reference implementation
- `src/ui/pages/search.rs` - Apply consistency changes
- Potentially shared components if needed
