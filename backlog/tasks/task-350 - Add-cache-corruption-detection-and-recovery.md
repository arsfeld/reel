---
id: task-350
title: Add cache corruption detection and recovery
status: To Do
assignee: []
created_date: '2025-10-03 13:37'
labels:
  - cache
  - error-handling
  - integrity
dependencies: []
priority: low
---

## Description

Detect cache corruption (metadata/file mismatches, partial writes, invalid chunks) and automatically recover by re-downloading affected chunks or files.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Verify chunk integrity on read (compare size, optionally checksum)
- [ ] #2 Detect partial/incomplete chunks (sparse file holes)
- [ ] #3 Mark corrupted chunks as unavailable in database
- [ ] #4 Trigger re-download for corrupted chunks
- [ ] #5 Add cache validation command for manual checks
<!-- AC:END -->
