---
id: task-135
title: Display sync status in sidebar with library spinners and status text
status: To Do
assignee: []
created_date: '2025-09-17 03:22'
labels: []
dependencies: []
priority: high
---

## Description

The sidebar needs to show active sync status both at the library level (with spinner indicators) and in the existing status area below the sidebar that currently shows 'No sources connected'. This will give users clear visibility of sync operations happening for each library and overall sync status.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Spinner indicator appears next to each library name when that library is syncing
- [ ] #2 Spinner disappears when library sync completes
- [ ] #3 Status text below sidebar shows overall sync status instead of just 'No sources connected'
- [ ] #4 Status text shows messages like 'Syncing Plex...', 'Sync complete', 'Sync failed', etc.
- [ ] #5 Multiple concurrent sync operations are properly reflected in status text
- [ ] #6 Status area properly updates when sources connect/disconnect
<!-- AC:END -->
