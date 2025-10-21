---
id: task-435
title: >-
  Fix crash when navigating away from library page due to ViewportScrolled
  signal
status: To Do
assignee: []
created_date: '2025-10-21 03:58'
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
- [ ] #1 Navigation from library page to home page does not crash
- [ ] #2 ViewportScrolled messages are properly handled or ignored when component is shut down
- [ ] #3 Signal handlers are cleaned up when library page component is destroyed
- [ ] #4 Fix is tested with multiple navigation scenarios (library->home, library->player, library->sources)
- [ ] #5 No similar crashes occur in other pages with viewport/scroll handlers
<!-- AC:END -->
