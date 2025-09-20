---
id: task-146
title: Batch fetch playback progress for media cards
status: Done
assignee: []
created_date: '2025-09-17 15:31'
updated_date: '2025-09-17 15:36'
labels: []
dependencies: []
priority: high
---

## Description

Implement efficient batch fetching of playback progress data before rendering media cards in home and library pages. Currently, the code has TODOs where it defaults to unwatched status because fetching individual progress in sync context is not possible.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Create a batch fetch method that retrieves all playback progress for a list of media IDs
- [x] #2 Call batch fetch method before display_source_sections in home page
- [x] #3 Call batch fetch method before RenderBatch in library page
- [x] #4 Update MediaCardInit creation to use the pre-fetched playback data
- [x] #5 Ensure watched indicators correctly reflect actual watched status
<!-- AC:END -->


## Implementation Notes

Implemented batch fetching of playback progress for media cards:

1. Created MediaService::get_playback_progress_batch method that fetches all playback records for a list of media IDs in a single database query

2. Updated HomePage::display_source_sections to:
   - Make it async to support batch fetching
   - Collect all media IDs from all sections
   - Batch fetch playback progress before creating media cards
   - Pass the fetched data to MediaCardInit creation

3. Updated LibraryPage::RenderBatch handler to:
   - Collect media IDs for each batch being rendered
   - Batch fetch playback progress for that batch
   - Use the fetched data when creating MediaCardInit

Now the unwatched indicators correctly show based on actual watched status from the database, not just defaulting to unwatched.
