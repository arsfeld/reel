---
id: task-164
title: Sync episode descriptions from backend servers
status: Done
assignee:
  - '@claude'
created_date: '2025-09-18 02:32'
updated_date: '2025-10-03 17:02'
labels:
  - sync
  - backend
  - metadata
dependencies: []
priority: high
---

## Description

Episode descriptions are currently empty in the application. Need to implement proper syncing of episode descriptions from Plex and Jellyfin backends during the sync process. This metadata is available from the servers but not being properly extracted and stored in the database.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Episode descriptions are fetched from Plex backend during sync
- [x] #2 Episode descriptions are fetched from Jellyfin backend during sync
- [x] #3 Descriptions are properly stored in the database media_items table
- [x] #4 Episode descriptions display correctly in the UI episode lists
- [x] #5 Sync process updates existing empty descriptions without duplicating episodes
<!-- AC:END -->


## Implementation Plan

1. Verify the code path: Backends → Mapper → Database
2. Test with actual Plex/Jellyfin data to see if descriptions are being fetched
3. Check if existing episodes in DB have descriptions
4. Check UI display code to ensure descriptions are shown
5. If needed, add migration to re-sync episodes with descriptions


## Implementation Notes

## Investigation Findings

After investigating the codebase, I discovered that the backend sync code was already working correctly:

### Backend Implementations (Already Correct)
1. **Plex** (`src/backends/plex/api/library.rs:235`): Maps `meta.summary` to `episode.overview`
2. **Jellyfin** (`src/backends/jellyfin/api.rs`): Maps `item.overview` to `episode.overview`

### Database Storage (Already Correct)
- **Mapper** (`src/mapper/media_item_mapper.rs:240`): Correctly includes `episode.overview` when converting to database model
- **Entity** (`src/db/entities/media_items.rs:13`): Has `overview: Option<String>` field
- **Sync Process** (`src/services/core/media.rs:11-12`): Uses `repo.update()` for existing episodes, which updates all fields including overview

### The Actual Issue: UI Display

The problem was NOT with syncing or database storage. Episode descriptions WERE being fetched and stored correctly. The issue was that the UI was not displaying them.


## Changes Made

### 1. UI: Added Episode Description Display
**File**: `src/ui/pages/show_details.rs:973-986`

Added description label to the `create_episode_card` function:
- Shows episode overview/description below the title and duration
- Wraps text with 2-line limit and ellipsis
- Only displays if description is available

### 2. CSS: Added Styling
**File**: `src/styles/details.css:546-551`

Added `.episode-description` style:
- Slightly dimmed color for secondary information
- Smaller font size (11px)
- Proper line height for readability

## Testing Notes

Users should re-sync their libraries to ensure all episodes have descriptions:
1. The sync process will update existing episodes with descriptions
2. New syncs will include descriptions automatically
3. Episode cards will now show the description below the title
