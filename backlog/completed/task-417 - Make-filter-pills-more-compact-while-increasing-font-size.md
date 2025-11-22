---
id: task-417
title: Make filter pills more compact while increasing font size
status: Done
assignee:
  - '@assistant'
created_date: '2025-10-06 14:56'
updated_date: '2025-10-06 17:22'
labels:
  - design
dependencies: []
priority: high
---

## Description

Filter pills are currently too large and take up excessive space. They need to be more compact, but the font size should be INCREASED (not decreased) as it's currently too small to read comfortably. Focus on reducing padding, margins, and spacing while making text more readable.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Filter pills have significantly reduced padding and margins
- [x] #2 Filter pill font size is increased for better readability
- [x] #3 Pills take up less overall space despite larger text
- [x] #4 Close buttons remain appropriately sized and clickable
- [x] #5 Overall filter bar is more compact
<!-- AC:END -->


## Implementation Plan

1. Review current .metadata-pill-modern CSS in details.css
2. INCREASE font-size (currently too small)
3. Reduce padding to make pills more compact
4. Reduce margins in library.rs pill creation code
5. Test visual appearance - pills should be compact but readable
6. Verify close buttons remain clickable


## Implementation Notes

Filter pills made more compact and readable with the following changes:

**CSS Changes (src/styles/details.css):**
- Increased font-size from 12px to 14px for better readability
- Reduced padding from 4px 10px to 2px 8px for more compact appearance

**Rust Changes (src/ui/pages/library.rs):**
- Reduced chip margins from margin_end(6)/margin_bottom(4) to margin_end(4)/margin_bottom(2)
- Reduced label internal margins from 8/8/3/3 to 6/6/2/2
- Close button margins preserved at 4/2/2 for clickability

Result: Pills are now more compact overall while having larger, more readable text. The reduced padding and margins compensate for the increased font size, resulting in less overall space usage.
