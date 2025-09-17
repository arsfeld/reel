---
id: task-122
title: Implement recently added filter logic
status: To Do
assignee: []
created_date: '2025-09-17 02:51'
labels:
  - filtering
  - backend
  - date
dependencies: []
---

## Description

Create filtering logic to show content added in the last 30 days when Recently Added tab is selected, using date_added metadata

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Recently Added filter shows items with date_added within last 30 days
- [ ] #2 Filter handles missing or null date_added fields gracefully
- [ ] #3 Filter works with existing sort options
- [ ] #4 30-day cutoff is configurable in the future but hardcoded initially
- [ ] #5 Filter performance is optimized for large libraries
- [ ] #6 Filter integrates with existing repository layer
<!-- AC:END -->
