---
id: task-027
title: Move database save location to XDG data folder instead of cache folder
status: Done
assignee:
  - '@claude'
created_date: '2025-09-15 14:56'
updated_date: '2025-09-15 15:03'
labels:
  - database
  - storage
dependencies: []
---

## Description

Currently the database is stored in the XDG cache folder, but it should be in the XDG data folder for proper persistence. The cache folder is meant for temporary data that can be regenerated, while the data folder is for persistent application data like databases.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Database location uses XDG data directory instead of cache directory
- [x] #2 Database connection logic updated to use new path
- [x] #3 Old cache location is not migrated (clean break)
<!-- AC:END -->


## Implementation Plan

1. Identify current database location in src/db/connection.rs
2. Replace dirs::cache_dir() with dirs::data_dir() for proper persistence
3. Test database connection with new path
4. Verify no migration logic exists (clean break)

## Implementation Notes

Changed database location from XDG cache directory to XDG data directory in src/db/connection.rs:84. Updated the db_path() function to use dirs::data_dir() instead of dirs::cache_dir(). This ensures the database is stored in a persistent location appropriate for application data. No migration was implemented as requested - users will start fresh with the new location.
