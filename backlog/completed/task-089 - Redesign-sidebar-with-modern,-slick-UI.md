---
id: task-089
title: 'Redesign sidebar with modern, slick UI'
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 17:54'
updated_date: '2025-09-16 18:38'
labels:
  - ui
  - sidebar
  - enhancement
dependencies: []
priority: high
---

## Description

Create a modern, visually appealing sidebar design that matches contemporary media applications like Netflix, Disney+, or Apple TV+. Focus on visual polish, smooth interactions, and a premium feel that complements the existing modern player and details pages.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Replace text-based navigation with icon + text combination for better visual hierarchy
- [x] #2 Add hover effects with subtle background color changes and smooth transitions
- [x] #3 Implement active/selected state with accent color highlight or left border indicator
- [x] #4 Add subtle separators between sections (navigation, sources, settings)
- [x] #5 Use modern typography with proper font weights (bold for headers, regular for items)
- [x] #6 Add smooth expand/collapse animations for source groups
- [x] #7 Include source type icons (Plex, Jellyfin, Local) with branded colors
- [x] #8 Polish spacing and padding for a more premium, less cramped feel
<!-- AC:END -->


## Implementation Plan

1. Examine current sidebar implementation
2. Research modern design patterns from Netflix/Disney+
3. Add icon support for navigation items
4. Implement hover and active state styles
5. Add section separators and improve spacing
6. Add source type branding (icons/colors)
7. Implement smooth animations
8. Polish typography and overall visual hierarchy


## Implementation Notes

## Implementation Summary

Successfully redesigned the sidebar with a modern, Netflix/Disney+ inspired UI featuring:

### Visual Design:
- Created new sidebar.css with comprehensive modern styling
- Dark gradient background with subtle borders
- Modern typography with proper font weights and sizes
- Premium spacing and padding throughout

### Navigation:
- Icon + text combination for all navigation items
- Active state with red accent color and left border indicator
- Smooth hover effects with background color transitions
- Home button only shown when sources exist

### Source Groups:
- Expandable/collapsible source groups with smooth animations
- Branded source icons with gradient backgrounds (Plex=gold, Jellyfin=purple, Local=blue)
- Clean library listings with icons and item counts
- Hover effects on all interactive elements

### Layout:
- Clear section headers ("Navigation", "Libraries")
- Subtle separators between sections
- Status section at bottom with connection information
- Sticky "Servers & Accounts" button with modern styling

### Technical Changes:
- Updated sidebar.rs component with new CSS classes
- Added expand/collapse state management for source groups
- Integrated sidebar.css into app initialization
- Fixed compilation issues with SourceType matching

## Revision Summary

Revised the sidebar to be more GNOME-compliant and Infuse-like after initial implementation was too Netflix-heavy:

### Simplified Styling:
- Removed excessive gradients, colors, and effects
- Used standard GNOME CSS variables (@sidebar_bg_color, @card_bg_color, etc.)
- Kept minimal hover effects with subtle alpha backgrounds
- Used standard GNOME classes (dim-label, caption, pill)

### Fixed Layout Issues:
- Fixed icon/text spacing overlap by setting consistent 12px spacing
- Aligned Home button with Source headers using same margins (8px)
- Set consistent icon sizes (16px for main icons)
- Fixed hover backgrounds to respect rounded corners (6px border-radius)

### Key Improvements:
- Collapsible source groups with smooth rotation animation
- Subtle card background for source groups for visual hierarchy
- Library selection styling (though selection tracking needs backend work)
- Clean, recognizable GNOME appearance
- Proper spacing throughout with no overlapping elements

### Final Design:
- Minimal, clean sidebar matching GNOME HIG
- Icons with proper spacing (no overlap)
- Collapsible sections with expand/collapse indicators
- Subtle backgrounds and hover effects
- Consistent alignment and spacing throughout
