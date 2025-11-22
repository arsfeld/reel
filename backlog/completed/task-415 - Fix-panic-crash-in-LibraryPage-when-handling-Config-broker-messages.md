---
id: task-415
title: Fix panic crash in LibraryPage when handling Config broker messages
status: Done
assignee:
  - '@claude'
created_date: '2025-10-06 14:54'
updated_date: '2025-10-06 17:30'
labels:
  - bug
  - crash
  - ui
dependencies: []
priority: high
---

## Description

The app crashes with a panic at src/ui/pages/library.rs:844:26 when processing Config::Updated broker messages. The error shows 'called Result::unwrap() on an Err value: BrokerMsg(Config(Updated {...}))'. This appears to be an unwrap() on a failed message send/receive operation, causing the component runtime to shut down.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify the unwrap() call at library.rs:844 causing the panic
- [x] #2 Replace unwrap() with proper error handling (match or ?)
- [x] #3 Ensure Config::Updated messages are handled gracefully
- [x] #4 Test that switching tabs and updating config does not crash
<!-- AC:END -->


## Implementation Plan

1. Examine the unwrap() at line 844 in library.rs
2. Replace unwrap() with proper error handling (use if let or match)
3. Break the loop if send fails (component shutting down)
4. Test the fix by switching tabs and updating config


## Implementation Notes

Fixed panic crash in LibraryPage broker subscription handling.

Replaced unwrap() with proper error handling at library.rs:844. When broker_sender.send() fails (component shutting down), the loop now breaks gracefully instead of panicking.

Tested: App runs without crashes when switching tabs and updating config.
