---
id: task-138
title: >-
  Fix empty sections appearing at top of home page after offline-first
  implementation
status: To Do
assignee: []
created_date: '2025-09-17 04:00'
labels: []
dependencies: []
priority: high
---

## Description

After implementing offline-first homepage loading (task-134), empty sections are now appearing at the top of the home page. This happens because the cached data loading creates sections that may not have items, or the section clearing/updating logic isn't working properly when fresh data arrives.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Empty sections do not appear on the home page
- [ ] #2 Sections only display when they contain items
- [ ] #3 Cached sections are properly replaced when fresh data arrives
- [ ] #4 Section ordering is consistent between cached and fresh data
<!-- AC:END -->
