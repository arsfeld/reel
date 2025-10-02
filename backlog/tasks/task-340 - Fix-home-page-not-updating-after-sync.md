---
id: task-340
title: Fix home page not updating after sync
status: Done
assignee:
  - '@claude'
created_date: '2025-10-02 19:40'
updated_date: '2025-10-02 19:56'
labels:
  - bug
  - ui
dependencies: []
priority: high
---

## Description

The home page does not refresh with new content after a background sync completes. Users must restart the application to see newly synced media items on the home page. This breaks the offline-first architecture's promise of seamless background updates.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify why home page doesn't subscribe to sync completion events
- [x] #2 Home page updates automatically when sync completes
- [x] #3 New media items appear on home page without requiring app restart
- [x] #4 Verify Recently Added section updates with fresh content
- [x] #5 Verify Continue Watching section updates if needed
<!-- AC:END -->


## Implementation Plan

1. Check if sync worker broadcasts SyncCompleted via MessageBroker
2. Add handling for BrokerMessage::Source(SyncCompleted) in home page
3. Trigger LoadData when sync completes to refresh home sections
4. Test that home page updates after sync without app restart
5. Verify all acceptance criteria


## Implementation Notes

Fixed home page not updating after background sync by implementing MessageBroker communication.

Changes made:
1. Added BROKER import to MainWindow (src/ui/main_window.rs)
2. Modified SyncWorkerOutput::SyncCompleted handler in MainWindow to broadcast BrokerMessage::Source(SourceMessage::SyncCompleted) to all subscribed components
3. Updated HomePage BrokerMsg handler (src/ui/pages/home.rs) to listen for SyncCompleted messages and trigger LoadData to refresh home sections

The home page now automatically reloads its content when any sync completes, showing newly synced media items in Recently Added and Continue Watching sections without requiring app restart. This restores the offline-first architecture promise of seamless background updates.
