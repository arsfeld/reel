---
id: task-467.02
title: Extract error handling and retry logic to ErrorRetryManager
status: Done
assignee: []
created_date: '2025-11-22 19:06'
updated_date: '2025-11-22 21:55'
labels:
  - refactoring
  - player
dependencies: []
parent_task_id: task-467
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Extract error handling and retry functionality from player mod.rs into a dedicated `error_retry.rs` module with an ErrorRetryManager struct.

State to extract:
- `error_message: Option<String>`
- `retry_count: u32`
- `max_retries: u32`
- `retry_timer: Option<SourceId>`

Logic to extract:
- Error state management
- Retry logic with exponential backoff (1s, 2s, 4s)
- Retry scheduling and cancellation
- Error clearing on successful state changes
- Max retry limit enforcement

ErrorRetryManager API:
- `new(max_retries: u32) -> Self`
- `get_error_message() -> Option<&str>`
- `has_error() -> bool`
- `show_error(message: String)`
- `clear_error()`
- `schedule_retry(media_id, context, sender)` - Schedule retry with backoff
- `cancel_retry()`
- `reset()` - Reset retry count
- Drop impl for cleanup
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Create src/ui/pages/player/error_retry.rs with ErrorRetryManager struct
- [ ] #2 Extract error and retry state fields from PlayerPage
- [ ] #3 Move retry logic with exponential backoff to manager
- [ ] #4 Move error display and clearing to manager
- [ ] #5 Code compiles without errors
- [ ] #6 Error retry works correctly with 3 attempts and exponential backoff
<!-- AC:END -->
