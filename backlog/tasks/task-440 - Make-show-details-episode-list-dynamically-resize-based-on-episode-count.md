---
id: task-440
title: Make show details episode list dynamically resize based on episode count
status: Done
assignee: []
created_date: '2025-10-23 00:27'
updated_date: '2025-10-23 00:38'
labels: []
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TV show details page episode list currently uses a fixed width layout regardless of how many episodes are displayed. This creates suboptimal use of screen space - shows with few episodes have wasted whitespace, while shows with many episodes may feel cramped. The episode list should dynamically resize based on the number of episodes being displayed, similar to how a similar issue was previously fixed on the homepage. This will provide a better viewing experience that adapts to the content.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Episode list width adjusts dynamically based on the number of episodes
- [x] #2 Shows with few episodes (1-5) use appropriate smaller width
- [x] #3 Shows with many episodes (10+) expand to utilize available screen space efficiently
- [x] #4 Layout remains responsive when window is resized
- [x] #5 Episode cards maintain proper aspect ratio and readability at all sizes
- [x] #6 Behavior is consistent with the dynamic sizing approach used on the homepage
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Analyze how the homepage implements dynamic sizing for media cards (✓)
2. Identify the issue in show_details.rs where episode grid uses fixed min/max children per line values
3. Update the `update_episode_grid` method to dynamically set min/max children based on episode count
4. Test with shows having different episode counts (few episodes vs many episodes)
5. Verify responsiveness when window is resized
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implementation complete. Updated show_details.rs to dynamically set min_children_per_line and max_children_per_line based on the actual number of episodes loaded, matching the approach used on the homepage. The episode grid now adapts its width based on episode count:

- Changed initial FlowBox configuration to use dynamic values (instead of fixed 100)
- Added logic in update_episode_grid() to set min/max children per line to the actual episode count
- This ensures shows with few episodes use less horizontal space while shows with many episodes expand appropriately
- The single-row layout is maintained while adapting to content

The fix ensures consistent behavior with the homepage media sections.
<!-- SECTION:NOTES:END -->
