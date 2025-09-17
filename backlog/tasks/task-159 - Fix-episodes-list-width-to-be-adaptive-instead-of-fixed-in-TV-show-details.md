---
id: task-159
title: Fix episodes list width to be adaptive instead of fixed in TV show details
status: To Do
assignee: []
created_date: '2025-09-17 19:32'
labels: []
dependencies: []
priority: high
---

## Description

The episodes list in the TV show details page currently has a fixed width that doesn't adapt to the number of episodes displayed. This causes layout issues when there are few episodes (wasted space) or many episodes (cramped display). The list should dynamically adjust its width based on content.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Episodes list width adapts dynamically to the number of episodes
- [ ] #2 List uses available horizontal space efficiently for any episode count
- [ ] #3 Minimum and maximum width constraints are properly defined
- [ ] #4 Episodes grid/list reflows responsively based on available width
- [ ] #5 Layout remains visually balanced regardless of episode count
<!-- AC:END -->
