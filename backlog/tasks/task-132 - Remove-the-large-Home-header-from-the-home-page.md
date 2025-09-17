---
id: task-132
title: Remove the large Home header from the home page
status: In Progress
assignee:
  - '@claude'
created_date: '2025-09-17 03:14'
updated_date: '2025-09-17 03:31'
labels: []
dependencies: []
priority: high
---

## Description

The home page currently displays a large 'Home' header that takes up unnecessary space and doesn't add value to the user experience. This header should be removed to provide more room for content and create a cleaner interface.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Large Home header is completely removed from the home page
- [ ] #2 Page layout adjusts properly after header removal
- [ ] #3 No visual artifacts or spacing issues after removal
<!-- AC:END -->

## Implementation Plan

1. Locate the Home header in the home.rs component
2. Remove the header Box container and its Label
3. Adjust the main container margins if needed
4. Test that the layout renders properly without the header
