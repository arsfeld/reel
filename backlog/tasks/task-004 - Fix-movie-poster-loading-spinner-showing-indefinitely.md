---
id: task-004
title: Fix movie poster loading spinner showing indefinitely
status: To Do
assignee: []
created_date: '2025-09-15 01:40'
labels:
  - ui
  - media
  - bug
dependencies: []
priority: medium
---

## Description

Movie poster loading spinners continue to show even after the poster image has loaded successfully. The MediaCard component shows a spinner overlay but the image loading completion is not properly detected or the spinner is not being hidden when loading completes.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Loading spinner is visible while poster is loading
- [ ] #2 Loading spinner disappears once poster has loaded
- [ ] #3 Failed image loads show appropriate error state instead of infinite spinner
- [ ] #4 Image loading state is properly tracked in MediaCard component
<!-- AC:END -->
