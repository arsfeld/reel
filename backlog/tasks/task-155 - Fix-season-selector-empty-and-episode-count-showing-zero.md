---
id: task-155
title: Fix season selector empty and episode count showing zero
status: To Do
assignee: []
created_date: '2025-09-17 15:45'
labels:
  - bug
  - ui
  - frontend
dependencies: []
priority: high
---

## Description

The season selector dropdown in the TV show details page always shows '(None)' by default and is empty, preventing users from selecting different seasons. Additionally, the episode count always displays '0 episodes' regardless of the actual number of episodes in the show. These bugs prevent proper navigation and display of TV show content.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Debug why seasons are not populating in the dropdown selector
- [ ] #2 Ensure seasons are properly fetched from backend and stored
- [ ] #3 Fix season dropdown to show available seasons with proper labels (Season 1, Season 2, etc.)
- [ ] #4 Set first season as default selection when page loads
- [ ] #5 Fix episode count calculation to show correct number
- [ ] #6 Update episode count when switching between seasons
- [ ] #7 Handle special seasons (Season 0/Specials) appropriately
- [ ] #8 Test with shows having multiple seasons and varying episode counts
<!-- AC:END -->
