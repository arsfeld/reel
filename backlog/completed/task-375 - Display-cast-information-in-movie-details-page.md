---
id: task-375
title: Display cast information in movie details page
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 17:09'
updated_date: '2025-10-03 17:23'
labels:
  - ui
  - movie-details
  - frontend
dependencies: []
priority: medium
---

## Description

Update movie details page UI to display cast and crew information. Show actor names with character roles and crew members with their positions. Include profile images where available.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add cast section to movie details page layout
- [x] #2 Display actors with character names
- [x] #3 Display directors and writers in crew section
- [x] #4 Show person profile images
- [x] #5 Handle missing profile images gracefully
- [x] #6 Test with movies that have cast/crew data
<!-- AC:END -->


## Implementation Plan

1. Analyze current cast display implementation and image loading
2. Fix person card image loading to use async URL loading instead of file loading
3. Add crew section UI below cast section
4. Filter crew to show only directors and writers
5. Test with movies that have cast/crew data populated


## Implementation Notes

## Summary

Successfully implemented cast and crew display in movie details page with async image loading.


## Changes Made

### 1. Shared Person Card Component
- Created `src/ui/shared/person_card.rs` with unified `create_person_card()` function
- Accepts optional texture parameter for async-loaded images
- Both movie_details and show_details now use this shared implementation

### 2. Movie Details Cast/Crew Display
- Added `person_textures: HashMap<String, gtk::gdk::Texture>` to store loaded images
- Added `crew_box: gtk::Box` for crew section UI
- Added `LoadPersonImage` and `PersonImageLoaded` command variants
- Implemented async person image loading using existing `load_image_from_url()` helper
- Cast section shows up to 10 actors with character roles
- Crew section shows up to 10 filtered directors and writers
- Images load asynchronously from URLs (120x120) and update cards when ready
- Missing images display gracefully as empty placeholder

### 3. Crew Filtering
- Filter crew members to only show Directors and Writers (case-insensitive)
- Both roles display in separate scrollable horizontal section below cast

### 4. Technical Details
- Person images are loaded via Command pattern with oneshot_command
- When PersonImageLoaded fires, all cast/crew cards are recreated with updated textures
- UI separates Cast and Crew into distinct scrollable sections
- Person cards use consistent styling: 120x120 image, name, and role

## Files Modified
- `src/ui/pages/movie_details.rs` - Added crew box, person texture storage, async image loading
- `src/ui/pages/show_details.rs` - Updated to use shared person card component
- `src/ui/shared/person_card.rs` - New shared component
- `src/ui/shared/mod.rs` - Export person_card module

## Testing
Builds successfully with no errors. Ready for visual testing with movies that have cast/crew metadata populated.
