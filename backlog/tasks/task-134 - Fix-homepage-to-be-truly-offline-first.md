---
id: task-134
title: Fix homepage to be truly offline-first
status: To Do
assignee: []
created_date: '2025-09-17 03:20'
labels: []
dependencies: []
priority: high
---

## Description

The homepage currently doesn't follow the offline-first architecture principle. It should load instantly from the SQLite cache without waiting for backend API calls. Currently, the homepage may show loading states or empty sections when backends are unavailable, instead of displaying cached content immediately.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Homepage loads instantly from SQLite cache on app startup
- [ ] #2 No loading spinners or empty states when cached data exists
- [ ] #3 Background sync updates content without blocking UI
- [ ] #4 Homepage displays cached content even when all backends are offline
- [ ] #5 Proper fallback to cache when API calls fail
<!-- AC:END -->
