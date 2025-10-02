---
id: task-304
title: Implement actual disk space checking for cache limits
status: To Do
assignee: []
created_date: '2025-09-29 02:46'
updated_date: '2025-10-02 14:58'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The current FileCacheConfig.effective_max_size_bytes() returns a placeholder value. Implement platform-specific disk space checking to properly enforce percentage-based cache limits.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Add disk space checking for macOS using statvfs
- [ ] #2 Add disk space checking for Linux using statvfs
- [ ] #3 Update cache cleanup to use actual available space
- [ ] #4 Add disk space monitoring for cache directory
<!-- AC:END -->
