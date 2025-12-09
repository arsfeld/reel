---
id: task-472.09
title: Clean up unused sync code and simplify SyncService
status: To Do
assignee: []
created_date: '2025-12-09 18:51'
labels:
  - cleanup
  - refactor
dependencies: []
parent_task_id: task-472
priority: low
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
## Overview

After implementing hybrid cache, clean up the codebase by removing unused sync complexity.

## Items to Remove/Simplify

1. **Delete SyncStrategy** (`src/backends/sync_strategy.rs`):
   - Never used in practice
   - Replaced by CacheConfig

2. **Simplify SyncService** (`src/services/core/sync.rs`):
   - Remove full sync orchestration
   - Keep only: `refresh_library()`, `refresh_items()`, `refresh_home_sections()`
   - Remove complex progress estimation

3. **Simplify sync_status tracking**:
   - May not need separate sync_status table
   - Can track via `fetched_at` on items
   - Consider deprecating or simplifying

4. **Remove SyncCommands** if unused (`src/services/commands/sync_commands.rs`):
   - These were thin wrappers
   - May not be needed with new architecture

5. **Update tests**:
   - Remove tests for deleted code
   - Add tests for new TTL logic

## Files to Delete
- `src/backends/sync_strategy.rs`
- Possibly `src/services/commands/sync_commands.rs`

## Files to Simplify
- `src/services/core/sync.rs`
- `src/workers/sync_worker.rs`

## Acceptance Criteria
- [ ] Unused code is removed
- [ ] SyncService is simpler and focused
- [ ] No dead code warnings
- [ ] Tests updated for new architecture
- [ ] Code compiles and all tests pass
<!-- SECTION:DESCRIPTION:END -->
