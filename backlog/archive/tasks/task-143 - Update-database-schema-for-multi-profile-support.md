---
id: task-143
title: Update database schema for multi-profile support
status: To Do
assignee: []
created_date: '2025-09-17 15:30'
labels:
  - backend
  - database
dependencies: []
priority: high
---

## Description

Extend the database schema to support multiple Plex profiles per source, including storing user tokens, profile metadata, and PIN protection status.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Add profiles table with source_id, profile_id, name, token, is_protected fields
- [ ] #2 Add profile_id column to playback_progress table
- [ ] #3 Add profile_id column to sync_status table
- [ ] #4 Create migration script for schema changes
- [ ] #5 Update repository layer to handle profile-scoped queries
- [ ] #6 Add indexes for efficient profile-based lookups
- [ ] #7 Ensure backward compatibility for existing single-profile data
<!-- AC:END -->
