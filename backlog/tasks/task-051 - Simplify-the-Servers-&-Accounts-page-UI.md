---
id: task-051
title: Simplify the Servers & Accounts page UI
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 02:41'
updated_date: '2025-09-16 03:13'
labels:
  - ui
  - ux
  - refactor
dependencies: []
priority: high
---

## Description

The Servers & Accounts page needs simplification to improve user experience. The current layout may be cluttered or confusing with unnecessary elements. Streamline the interface to focus on essential server management functionality with a cleaner, more intuitive design.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Review current Servers & Accounts page layout and identify unnecessary elements
- [x] #2 Remove or consolidate redundant UI components
- [x] #3 Improve visual hierarchy to highlight important actions (add/remove servers)
- [x] #4 Simplify server status display and connection indicators
- [x] #5 Ensure all essential functionality remains accessible
- [x] #6 Test that server management operations work correctly after simplification
<!-- AC:END -->


## Implementation Plan

1. Analyze current UI and identify redundant elements
2. Remove "Test Connection" button (rarely used, can be part of sync)
3. Simplify source type display (remove debug format)
4. Consolidate server info into cleaner layout
5. Improve visual hierarchy with better spacing and typography
6. Test all server management operations still work

## Implementation Notes

Simplified the Servers & Accounts page UI by:

1. **Removed redundant elements**:
   - Eliminated the Test Connection button (rarely used, connection status shown via icon)
   - Removed TestConnection enum variant and handling code

2. **Improved visual hierarchy**:
   - Reduced icon size from 40px to 32px for better proportion
   - Simplified server type display (Plex/Jellyfin instead of debug format)
   - Extracted just hostname from URLs for cleaner display
   - Reduced spacing between title and subtitle (4px to 2px)

3. **Consolidated UI components**:
   - Moved connection status to a simple icon with tooltip
   - Removed duplicate connection status text labels
   - Streamlined action buttons to just Sync and Remove

4. **Maintained functionality**:
   - All essential operations (add, remove, sync) remain accessible
   - Connection status still clearly visible via icon color
   - Sync progress animation preserved

The simplified UI is cleaner, more focused, and easier to understand while maintaining all critical functionality for server management.
