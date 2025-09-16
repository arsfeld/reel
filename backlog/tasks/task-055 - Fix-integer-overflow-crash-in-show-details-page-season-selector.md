---
id: task-055
title: Fix integer overflow crash in show details page season selector
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 03:00'
updated_date: '2025-09-16 03:03'
labels:
  - bug
  - crash
  - ui
dependencies: []
priority: high
---

## Description

The application crashes with an integer overflow when navigating to a show details page. The crash occurs at show_details.rs:302:61 with 'attempt to add with overflow'. This happens when the season dropdown is being set up, likely when calculating the selected season index from the season list.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Identify the arithmetic operation causing overflow at line 302
- [x] #2 Add proper bounds checking before arithmetic operations
- [x] #3 Handle edge cases like empty season lists or invalid indices
- [x] #4 Use checked arithmetic operations (checked_add) instead of direct addition
- [x] #5 Test with shows that have many seasons or unusual season numbering
- [x] #6 Ensure season selector works correctly after fix
<!-- AC:END -->


## Implementation Plan

1. Analyze the overflow at line 302 where selected + 1 occurs
2. Replace direct addition with checked_add to prevent overflow
3. Handle the None case from checked_add gracefully
4. Test with edge cases like empty season lists
5. Verify season selector works after the fix


## Implementation Notes

Fixed integer overflow crash in show_details.rs at line 302.

The issue occurred when the dropdown's selected index was u32::MAX and adding 1 caused an overflow.

Solution:
- Replaced direct arithmetic (selected as u32 + 1) with checked_add
- Used unwrap_or(1) to default to season 1 if overflow occurs
- This prevents the crash while maintaining correct season selection behavior

The fix is minimal and focused, using Rust's safe arithmetic operations to handle edge cases gracefully.
