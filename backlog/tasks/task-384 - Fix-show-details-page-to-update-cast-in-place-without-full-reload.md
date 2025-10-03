---
id: task-384
title: Fix show details page to update cast in-place without full reload
status: Done
assignee:
  - '@claude'
created_date: '2025-10-03 18:27'
updated_date: '2025-10-03 18:33'
labels:
  - show-details
dependencies: []
priority: high
---

## Description

After lazy loading full cast metadata, the show details page currently does a full page reload which creates a jarring UX. Need to update just the cast section in-place like we fixed for movie details.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Update ShowDetailsCommand::FullMetadataLoaded to reload cast from database
- [x] #2 Rebuild cast box in-place without triggering LoadDetails
- [x] #3 Load person images for newly added cast members
- [x] #4 Update show.cast in the model without full reload
<!-- AC:END -->


## Implementation Plan

1. Add person_textures HashMap to ShowDetailsPage model to track loaded images
2. Add LoadPersonImage and PersonImageLoaded commands to ShowDetailsCommand enum
3. Add handler for LoadPersonImage command (similar to movie_details.rs)
4. Add handler for PersonImageLoaded command to store textures
5. Update FullMetadataLoaded handler to:
   - Query PeopleRepositoryImpl for updated cast
   - Update self.show.cast in place
   - Clear and rebuild cast_box
   - Trigger LoadPersonImage for each cast member
   - Avoid calling LoadDetails
6. Update initial cast box building in LoadDetails to use person_textures
7. Test the changes


## Implementation Notes

Implemented in-place cast update for show details page following the pattern from movie_details.rs.

Changes made:
1. Added person_textures HashMap to ShowDetailsPage to cache person images
2. Added LoadPersonImage and PersonImageLoaded commands to ShowDetailsCommand enum
3. Implemented handlers for LoadPersonImage and PersonImageLoaded commands
4. Updated LoadDetails to use person_textures when building initial cast box
5. Rewrote FullMetadataLoaded handler to:
   - Query PeopleRepositoryImpl for updated cast from database
   - Update show.cast in place without triggering LoadDetails
   - Clear and rebuild cast_box with new cast members
   - Load person images for each cast member without existing texture
6. Fixed missing PeopleRepository trait import in both show_details.rs and movie_details.rs

Files modified:
- src/ui/pages/show_details.rs: Added person_textures, new commands, and in-place cast update logic
- src/ui/pages/movie_details.rs: Added missing PeopleRepository trait import

The fix eliminates the jarring full page reload when lazy loading completes. Only the cast section updates smoothly in-place.
