---
id: task-326
title: Redesign cache system with database-driven chunk management
status: Done
assignee:
  - '@claude'
created_date: '2025-10-01 15:40'
updated_date: '2025-10-02 14:51'
labels:
  - cache
  - architecture
  - database
dependencies: []
priority: high
---

## Description

The current cache system has fundamental architectural issues:

1. **Database schema exists but unused**: cache_chunks table tracks byte ranges but downloader/proxy don't use it\n2. **Sequential downloads only**: Downloader downloads entire files sequentially instead of prioritizing requested chunks\n3. **In-memory state**: State machine tracks state in memory instead of using database as source of truth\n4. **Proxy guessing**: Proxy checks file size on disk instead of querying database for available chunks\n5. **No range prioritization**: When proxy gets range requests for uncached data, it doesn't tell downloader to prioritize those chunks\n\n**Goal**: Redesign the cache system around the database as the single source of truth for what data is available.
