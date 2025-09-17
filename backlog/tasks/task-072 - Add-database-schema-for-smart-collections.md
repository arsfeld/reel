---
id: task-072
title: Add database schema for smart collections
status: To Do
assignee: []
created_date: '2025-09-17 02:46'
labels:
  - database
  - schema
dependencies: []
priority: high
---

## Description

Create database table to store smart collection definitions for auto-generated collections like franchises, genre combinations, and quality-based groups

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 smart_collections table created with id, name, type, and query_params columns
- [ ] #2 Type field supports values: franchise, genre_combo, era, quality
- [ ] #3 Query_params field stores JSON criteria for collection filtering
- [ ] #4 Migration script created and tested
<!-- AC:END -->
