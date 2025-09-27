---
id: task-266
title: Add section metadata caching strategy
status: To Do
assignee: []
created_date: '2025-09-26 17:49'
labels: []
dependencies: []
---

## Description

Implement a caching strategy to track when home sections were last updated and whether they need refreshing. This optimizes API calls while maintaining fresh data.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Add last_updated timestamp to home_sections table
- [ ] #2 Implement stale detection logic (e.g., older than 5 minutes)
- [ ] #3 Add force_refresh parameter to get_home_sections_per_source
- [ ] #4 Skip API call if cache is fresh and not forcing refresh
- [ ] #5 Add background refresh for stale sections
- [ ] #6 Log cache hit/miss metrics for monitoring
<!-- AC:END -->
