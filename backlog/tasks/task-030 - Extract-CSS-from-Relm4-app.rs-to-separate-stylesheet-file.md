---
id: task-030
title: Extract CSS from Relm4 app.rs to separate stylesheet file
status: To Do
assignee: []
created_date: '2025-09-15 15:13'
labels:
  - refactoring
  - relm4
  - css
dependencies: []
priority: medium
---

## Description

The Relm4 app.rs file contains over 300 lines of inline CSS that should be moved to a separate stylesheet file for better maintainability and separation of concerns. This will make the CSS easier to manage and the app.rs file cleaner.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Create a new CSS file at src/platforms/relm4/styles/app.css (or similar location)
- [ ] #2 Move all CSS from the relm4::set_global_css() call to the new CSS file
- [ ] #3 Update app.rs to load CSS from the external file instead of inline string
- [ ] #4 Verify all styles still apply correctly after the extraction
- [ ] #5 Ensure the CSS file is properly included in the build process
<!-- AC:END -->
