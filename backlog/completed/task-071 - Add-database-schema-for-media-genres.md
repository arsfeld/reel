---
id: task-071
title: Add database schema for media genres
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

Create a new database table to store genre associations for media items to enable genre-based filtering in library views

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 media_genres table created with media_item_id and genre columns
- [ ] #2 Primary key constraint on (media_item_id, genre) prevents duplicates
- [ ] #3 Index on genre column for fast genre-based queries
- [ ] #4 Migration script created and tested
<!-- AC:END -->
