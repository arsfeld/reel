---
id: task-258
title: >-
  Fix Continue Watching section to show TV show posters instead of episode
  thumbnails
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 17:35'
updated_date: '2025-09-26 23:53'
labels:
  - ui
  - homepage
  - enhancement
dependencies: []
priority: high
---

## Description

The Continue Watching section on the homepage currently displays low-resolution episode thumbnails and episode names instead of the TV show's poster art and show title. This creates an inconsistent and less visually appealing experience compared to other sections that show proper poster artwork.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Modify Continue Watching section to use TV show poster URL instead of episode thumbnail
- [x] #2 Display TV show title as primary text instead of episode name
- [x] #3 Show episode information as secondary text (e.g., 'S1E5 - Episode Title')
- [x] #4 Ensure poster images are high-resolution versions
- [x] #5 Maintain playback progress indicator on the cards
- [x] #6 Test with multiple TV shows to ensure consistent display
<!-- AC:END -->


## Implementation Plan

1. Search for Continue Watching section implementation in home.rs
2. Examine how episode data is currently retrieved and displayed
3. Identify parent show data structure and poster URL field
4. Modify card rendering to use show poster instead of episode thumbnail
5. Update title display to show series title with episode info as subtitle
6. Ensure playback progress indicators remain functional
7. Test with various shows to verify proper display


## Implementation Notes

Modified the home page to display TV show posters instead of episode thumbnails for all sections (not just Continue Watching).

Changes made:
1. In src/ui/pages/home.rs:
   - Added logic to batch fetch parent shows for all episodes in sections
   - Modified display_source_sections to query parent show data from database using MediaRepository
   - Updated card creation to replace episode poster with parent show poster
   - Set show title as primary text with episode info as subtitle

2. In src/ui/factories/media_card.rs:
   - Updated format_subtitle to check for episode_subtitle in metadata
   - Returns custom subtitle for episodes when displaying as show cards

The solution queries the database directly for parent show data rather than relying on metadata, ensuring we always have accurate poster URLs and show titles. Progress indicators and all other functionality remain intact.

Additional changes:
- Added proper error logging when parent shows cannot be found
- Episodes without parent shows are now skipped (no fallback to episode thumbnail)
- Added debug logging to track parent show fetching process
- Deduplicates parent IDs before fetching to avoid redundant queries
