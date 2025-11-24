---
id: task-468.02
title: Fix unused and unnecessary mut variables
status: Done
assignee: []
created_date: '2025-11-24 19:57'
updated_date: '2025-11-24 20:05'
labels:
  - cleanup
dependencies: []
parent_task_id: task-468
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Remove unnecessary mut keywords and prefix unused variables with underscore:

**Unnecessary mut:**
- src/cache/chunk_manager.rs:96 - `let mut active` should be `let active`
- src/ui/pages/movie_details.rs:419 - `mut rx` should be `rx`
- src/ui/pages/show_details.rs:541 - `mut rx` should be `rx`

**Unused variables (prefix with underscore):**
- src/cache/proxy.rs:487 - `headers` → `_headers`
- src/services/core/connection.rs:156 - `backend` → `_backend`
- src/ui/pages/home.rs:254 - `sources_processed` → `_sources_processed`
- src/ui/pages/show_details.rs:803-804 - `synced` and `failed` → `synced: _` and `failed: _`
- src/workers/playback_sync_worker.rs:134 - `config` → `_config`
- src/backends/traits.rs:55 - `media_id` → `_media_id`
- src/cache/config.rs:172 - `stripped` → `_stripped`
<!-- SECTION:DESCRIPTION:END -->
