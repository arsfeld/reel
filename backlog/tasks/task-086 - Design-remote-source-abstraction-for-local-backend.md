---
id: task-086
title: Design remote source abstraction for local backend
status: To Do
assignee: []
created_date: '2025-09-16 17:40'
labels:
  - backend
  - local-files
  - architecture
dependencies: []
priority: low
---

## Description

Create an abstraction layer that will allow the local backend to support both local directories and remote sources (SMB, NFS, WebDAV) in the future. Design the interface without implementing remote sources yet.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Define RemoteSource trait for future extensibility
- [ ] #2 Separate file access logic from LocalBackend core
- [ ] #3 Create FileAccessor abstraction that works for local files
- [ ] #4 Ensure design supports async I/O for network sources
- [ ] #5 Document planned support for SMB, NFS, WebDAV protocols
<!-- AC:END -->
