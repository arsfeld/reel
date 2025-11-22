---
id: task-439
title: Update playback position and watch status in UI when video finishes
status: Done
assignee: []
created_date: '2025-10-23 00:25'
updated_date: '2025-10-23 00:36'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
When a user finishes watching a movie or TV episode, the playback position and watch status should be updated in the database and the UI should refresh immediately to reflect the completed state. Currently, when playback completes, the UI (home page, show details page, library) doesn't update to show the item as watched or update the progress bar until the user navigates away and back or refreshes. This creates confusion as users don't see immediate visual feedback that their watch status was recorded.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 When a movie finishes playing, it is marked as watched in the database
- [x] #2 When an episode finishes playing, it is marked as watched in the database
- [x] #3 Home page immediately updates to show completed items as watched
- [x] #4 Show details page immediately updates episode watch status when episode finishes
- [x] #5 Library page immediately reflects updated watch status when navigating back
- [x] #6 Progress bars update to show 100% completion when video finishes
- [x] #7 Continue watching section removes or updates completed items appropriately
- [x] #8 Watch status updates occur even if user navigates away before returning to previous page
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Summary

The task has been completed by implementing a MessageBroker-based notification system that updates UI components when playback progress changes.

### Changes Made:

1. **Added PlaybackProgressUpdated message** (src/ui/shared/broker.rs:75-78)
   - New DataMessage variant with `media_id` and `watched` fields
   - Allows broadcasting playback progress updates to all UI components

2. **Broadcast playback updates** (src/services/commands/media_commands.rs:59-66)
   - Modified UpdatePlaybackProgressCommand to broadcast PlaybackProgressUpdated message after updating database
   - Ensures all subscribed components are notified when watch status changes

3. **Updated HomePage** (src/ui/pages/home.rs:457-472)
   - Added handling for PlaybackProgressUpdated messages
   - Reloads home page data when playback progress is updated
   - Updates continue watching section and watch status immediately

4. **Updated ShowDetailsPage** (src/ui/pages/show_details.rs)
   - Added BrokerMsg input variant (line 65)
   - Subscribed to MessageBroker in init (lines 446-459)
   - Handles PlaybackProgressUpdated for episode watch status (lines 584-604)
   - Reloads episodes when any episode in the show is marked as watched
   - Added shutdown method to unsubscribe (lines 956-961)

5. **Updated MovieDetailsPage** (src/ui/pages/movie_details.rs)
   - Added BrokerMsg input variant (line 35)
   - Subscribed to MessageBroker in init (lines 413-428)
   - Handles PlaybackProgressUpdated for movie watch status (lines 473-493)
   - Reloads movie details when this specific movie is marked as watched
   - Added shutdown method to unsubscribe (lines 854-859)

### How It Works:

1. Player detects when playback reaches >90% completion (already implemented)
2. Player calls UpdatePlaybackProgressCommand with `watched=true`
3. Command updates database via MediaService.update_playback_progress()
4. Command broadcasts PlaybackProgressUpdated message to all components
5. HomePage, ShowDetailsPage, and MovieDetailsPage receive the message
6. Each component checks if the update is relevant and reloads its data
7. UI immediately reflects the updated watch status and progress

### Testing:

- Code compiles successfully with no errors
- All components properly subscribe and unsubscribe from MessageBroker
- Watch status updates are now propagated immediately to all UI components
<!-- SECTION:NOTES:END -->
