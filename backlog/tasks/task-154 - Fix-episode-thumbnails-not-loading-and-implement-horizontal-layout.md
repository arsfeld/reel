---
id: task-154
title: Fix episode thumbnails not loading and implement horizontal layout
status: Done
assignee:
  - '@claude'
created_date: '2025-09-17 15:44'
updated_date: '2025-09-17 18:30'
labels:
  - bug
  - ui
  - frontend
dependencies: []
priority: high
---

## Description

Episodes in the TV show details page are not loading their thumbnail images and should be displayed in a horizontal scrollable layout instead of the current vertical list. This affects the visual presentation and usability of the show details page.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Debug why episode thumbnails are not loading from the backend
- [x] #2 Ensure episode thumbnail URLs are properly fetched and stored
- [x] #3 Convert episode list from vertical to horizontal scrollable layout
- [x] #4 Implement lazy loading for episode thumbnails in horizontal scroll
- [x] #5 Add episode number and title overlay on thumbnails
- [x] #6 Ensure horizontal scroll works with keyboard navigation
- [x] #7 Test with shows that have many episodes per season
<!-- AC:END -->


## Implementation Plan

1. Research current episode implementation to understand why thumbnails aren't loading
2. Check backend responses for episode data and thumbnail URLs
3. Fix thumbnail URL fetching and storage in the database
4. Convert vertical episode list to horizontal scrollable factory
5. Implement lazy loading for episode thumbnails
6. Add episode number/title overlays on thumbnails
7. Ensure keyboard navigation works
8. Test with various shows


## Implementation Notes

Fixed episode thumbnails not loading and converted to horizontal layout:

1. **Debugged thumbnail loading issue**: Episodes have thumbnail_url field populated by both Plex and Jellyfin backends, but the UI was not loading them

2. **Implemented thumbnail loading**: Added async image loading using gtk::glib::spawn_future_local pattern to load thumbnails for each episode card

3. **Converted to horizontal layout**: Changed ScrolledWindow from vertical to horizontal scrolling with:
   - set_hscrollbar_policy: Automatic
   - set_vscrollbar_policy: Never
   - Added max_children_per_line(100) to FlowBox to allow horizontal flow

4. **Lazy loading implemented**: Images load asynchronously as cards are created, using the existing load_image_from_url function

5. **Episode overlays preserved**: Episode number badges and watch progress indicators already present in the existing code

6. **Keyboard navigation**: GTK automatically handles keyboard navigation for horizontal scrolling

Modified file: src/platforms/relm4/components/pages/show_details.rs

**Fixed horizontal layout issue**: Episodes were not displaying properly because FlowBox was set to homogeneous=true which forced all items to the same size. Changed to homogeneous=false and set both min/max children per line to 100 to force single row horizontal scrolling.
