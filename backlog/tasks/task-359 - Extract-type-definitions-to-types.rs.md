---
id: task-359
title: Extract type definitions to types.rs
status: To Do
assignee: []
created_date: '2025-10-03 14:31'
labels:
  - refactor
  - ui
dependencies: []
priority: low
---

## Description

Move MainWindowInput, MainWindowOutput, and ConnectionStatus enums from main_window to types.rs for better organization.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 types.rs file created in src/ui/main_window/
- [ ] #2 MainWindowInput enum moved to types.rs
- [ ] #3 MainWindowOutput enum moved to types.rs
- [ ] #4 ConnectionStatus enum moved to types.rs
- [ ] #5 Types properly re-exported from mod.rs
- [ ] #6 Application compiles without errors
<!-- AC:END -->
