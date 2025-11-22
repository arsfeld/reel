---
id: task-464.04
title: Implement conflict resolution for local vs backend playback state
status: Done
assignee: []
created_date: '2025-11-22 20:10'
updated_date: '2025-11-22 20:56'
labels:
  - sync
  - conflict-resolution
  - logic
dependencies: []
parent_task_id: task-464
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Implement logic to handle conflicts when both local and backend have different playback progress or watch status for the same media item.

**Conflict Scenarios**:
1. **Position Mismatch**: Local has position 500s, backend has 300s
2. **Watch Status Mismatch**: Local marked as watched, backend shows unwatched
3. **Timestamp Mismatch**: Local change is newer/older than backend change

**Resolution Strategies**:

**Strategy 1: Local-Progressive (Recommended)**
- For position: Use whichever position is further along
- Rationale: User likely wants to continue from furthest point
- Example: If local=500s, backend=300s → sync 500s to backend

**Strategy 2: Last-Write-Wins**
- Use the most recent change based on timestamp
- Requires reliable timestamps from backend
- May lose progress if backend timestamp is stale

**Strategy 3: Always Local**
- Local changes always override backend
- Simpler but may lose legitimate backend changes (multi-device scenario)

**Implementation**:
- Add `ConflictResolver` trait with different strategy implementations
- Make strategy configurable (start with Local-Progressive as default)
- Log conflict resolutions for debugging
- Add metrics for conflict frequency

**Edge Cases**:
- Handle missing timestamps gracefully
- Deal with backend API limitations (some may not return timestamps)
- Consider watched vs unwatched transitions (unwatching should be respected)
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 ConflictResolver trait is defined with multiple strategy implementations
- [x] #2 Local-Progressive strategy is implemented and set as default
- [x] #3 Position conflicts are resolved correctly (furthest position wins)
- [x] #4 Watch status conflicts are resolved correctly
- [x] #5 Conflict resolutions are logged for debugging
- [x] #6 Tests cover all conflict scenarios (position, watch status, timestamps)
- [x] #7 Edge cases are handled (missing timestamps, API limitations)
<!-- AC:END -->
