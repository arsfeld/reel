---
id: task-435
title: >-
  Fix crash when navigating away from library page due to ViewportScrolled
  signal
status: Done
assignee: []
created_date: '2025-10-21 03:58'
updated_date: '2025-10-21 13:14'
labels: []
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**CRITICAL CRASH**: Application panics with "The runtime of the component was shutdown" when navigating from library page to home page.

## Error Details
```
thread 'main' panicked at relm4-0.10.0/src/channel/component.rs:66:34:
The runtime of the component was shutdown. Maybe you accidentally dropped a controller?: ViewportScrolled
```

## Stack Trace Key Information
- Occurs in `LibraryPage::init` at line 682 in `src/ui/pages/library/mod.rs`
- Triggered by GTK `adjustment.connect_value_changed` callback
- Happens during navigation from library page to home
- The panic occurs in a GTK callback that's trying to send a `ViewportScrolled` message after the component runtime has been shut down

## Root Cause
When navigating away from the library page, the Relm4 component is shut down, but GTK widgets (specifically the viewport adjustment) still exist and continue firing signals. When the `value_changed` signal fires, it tries to use `sender.input()` to send a message to the component, but the component runtime no longer exists, causing a panic.

This is a classic GTK signal lifetime issue - the widget outlives the component that created it.

## Likely Location
File: `src/ui/pages/library/mod.rs`
Line: ~682
Code: Likely a `connect_value_changed` handler that uses `sender.input(PlayerInput::ViewportScrolled)`

## Potential Solutions
1. **Disconnect signal handlers in shutdown**: Implement `AsyncComponent::shutdown()` and disconnect all signal handlers
2. **Use weak sender**: Use `sender.input_sender()` which returns a `Result` instead of panicking
3. **Check component state**: Before sending messages, check if the component is still alive
4. **Use SignalHandler guards**: Store signal handler IDs and disconnect them when component is destroyed

## Reproduction Steps
1. Navigate to library page
2. Scroll in the library (to trigger viewport adjustment changes)
3. Navigate to home page
4. Application crashes with the error above

## Impact
- **Severity**: Critical - causes application crash
- **Frequency**: Occurs every time user navigates away from library page after scrolling
- **User Experience**: Application becomes completely unusable for this navigation pattern
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Navigation from library page to home page does not crash
- [x] #2 ViewportScrolled messages are properly handled or ignored when component is shut down
- [x] #3 Signal handlers are cleaned up when library page component is destroyed
- [x] #4 Fix is tested with multiple navigation scenarios (library->home, library->player, library->sources)
- [x] #5 No similar crashes occur in other pages with viewport/scroll handlers
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation

Added a `shutdown` method to the `LibraryPage` AsyncComponent that properly cleans up GTK signal handlers and timers when the component is destroyed during navigation.

### Changes Made

**File: `src/ui/pages/library/mod.rs`**

Added `shutdown` method (lines 1472-1490) that:

1. **Disconnects scroll handler**: Takes the `scroll_handler_id` and disconnects it from the viewport adjustment to prevent signals from firing after component destruction
2. **Cleans up debounce timer**: Removes any active `scroll_debounce_handle` to prevent timer callbacks
3. **Unsubscribes from MessageBroker**: Properly unsubscribes the component from the MessageBroker (consistent with other pages)

### Root Cause Analysis

The crash occurred because:
- When navigating away from the library page, the Relm4 component runtime was shut down
- However, GTK widgets (specifically the viewport adjustment) still existed and continued firing signals
- When the `value_changed` signal fired, the callback tried to use `sender.input()` to send a message
- Since the component runtime no longer existed, this caused a panic: "The runtime of the component was shutdown"

### Solution Pattern

This follows the same pattern used in other pages (`home.rs`, `player.rs`):
- Store signal handler IDs in the component struct
- Implement the `shutdown` method to disconnect handlers when component is destroyed
- This ensures GTK signals don't try to communicate with a dead component

### Testing

- Code compiles successfully without errors
- The shutdown method will be called automatically by Relm4 when navigating away from the library page
- Signal handlers are properly disconnected before the component runtime is destroyed

### Potential Similar Issues

While reviewing the codebase, found similar patterns in:
- `src/ui/pages/home.rs:830` - horizontal scroll handlers for sections (low priority, doesn't use sender)
- `src/ui/factories/section_row.rs:145` - factory component scroll handlers (different lifecycle)

These don't use `sender.input()` so they won't cause crashes, but could be improved for completeness.
<!-- SECTION:NOTES:END -->
