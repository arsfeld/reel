---
id: task-371
title: Display cast and crew information in movie and TV show details
status: To Do
assignee: []
created_date: '2025-10-03 17:01'
labels:
  - feature
  - ui
  - movie-details
  - show-details
dependencies: []
priority: high
---

## Description

Add UI components to display cast and crew information in movie details and show details pages. This includes actor names, character names, roles (director, writer, etc.), and profile images. The UI should show a horizontal scrollable row of cast members with photos and names, similar to other media players. Depends on cast/crew data being available in the database from task-370.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Design UI layout for cast display (horizontal scrollable row with cards)
- [ ] #2 Create cast member card component showing photo, name, and character/role
- [ ] #3 Fetch cast/crew data from database in MovieDetailsPage
- [ ] #4 Fetch cast/crew data from database in ShowDetailsPage
- [ ] #5 Display cast section in movie details page
- [ ] #6 Display cast section in show details page
- [ ] #7 Handle missing profile images gracefully with placeholder
- [ ] #8 Cast section is scrollable horizontally when there are many members
- [ ] #9 Shows appropriate roles (Actor as Character, Director, Writer, etc.)
<!-- AC:END -->
