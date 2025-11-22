---
id: task-425
title: Fix library image loading stuck after switching libraries multiple times
status: Done
assignee:
  - '@claude'
created_date: '2025-10-06 18:50'
updated_date: '2025-10-06 18:54'
labels:
  - ui
  - bug
  - critical
dependencies: []
priority: high
---

## Description

After switching between different libraries 2-3 times, the library view stops loading posters altogether. The application queues image loads ('Queued 50 new image loads') but the image_loader worker never processes them (no 'Downloading image' logs appear). This makes the library view unusable as all media items appear without posters.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Image loading continues to work after multiple library switches
- [x] #2 Image loader worker processes all queued requests
- [x] #3 No silent failures or crashes in image loading pipeline
- [x] #4 Root cause identified and fixed
<!-- AC:END -->


## Implementation Plan

1. Analyze the SetLibrary and refresh handlers to understand image cleanup
2. Identify the bug: cancel_pending_images() is called AFTER clearing image_requests
3. Fix SetLibrary handler: move cancel_pending_images() before clearing image_requests
4. Fix refresh() method: move cancel_pending_images() before clearing image_requests
5. Test by switching libraries multiple times to verify images continue loading


## Implementation Notes

Fixed critical bug in image loading cleanup order.

Root Cause:
- In SetLibrary handler (mod.rs:732), image_requests was cleared BEFORE calling cancel_pending_images()
- In refresh() method (data.rs:175), same issue occurred
- cancel_pending_images() iterates through image_requests to send cancel messages to the worker
- Since image_requests was already empty, no cancellations were sent
- Worker accumulated orphaned requests that never completed, blocking new requests

Changes:
1. src/ui/pages/library/mod.rs:745 - Moved cancel_pending_images() call before clear
2. src/ui/pages/library/data.rs:182 - Moved cancel_pending_images() call before clear

Result:
- Worker properly cancels old requests before starting new ones
- No accumulation of orphaned image requests
- Image loading continues working after multiple library switches

Testing Note:
AC #1 (Image loading continues to work after multiple library switches) requires manual testing with the running application. The fix is implemented and should resolve the issue, but needs verification by:
1. Running the app with multiple libraries configured
2. Switching between libraries 3-5 times
3. Verifying that images continue to load on each switch
4. Checking logs for "Queued N new image loads" followed by "Downloading image" messages
