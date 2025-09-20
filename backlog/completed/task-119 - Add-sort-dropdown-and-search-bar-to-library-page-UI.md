---
id: task-119
title: Add sort dropdown and search bar to library page UI
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 23:10'
updated_date: '2025-09-16 23:27'
labels: []
dependencies: []
priority: high
---

## Description

The library page already has sorting and filtering logic implemented but provides no UI controls for users to access these features. Add a sort dropdown and make the search bar visible/accessible to expose existing functionality.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add sort dropdown menu to library header with existing options (Title, Year, DateAdded, Rating)
- [x] #2 Make search bar always visible or add search button to show it
- [x] #3 Connect sort dropdown to existing SetSortBy input handler
- [x] #4 Ensure search bar is properly styled and positioned
- [x] #5 Add visual indicator showing current sort option
- [x] #6 Test that existing sort and filter logic works with new UI controls
<!-- AC:END -->


## Implementation Plan

1. Review current library page implementation
   - Existing search bar implementation (lines 224-237)
   - SortBy enum and SetSortBy handler (lines 61-66, 414-418)
   - Keyboard shortcut for search (slash key)

2. Add header bar with controls
   - Create adw::HeaderBar at top of library page
   - Add sort dropdown (gtk::DropDown) to header
   - Keep existing search bar implementation

3. Implement sort dropdown
   - Use gtk::DropDown with StringList model
   - Options: "Title", "Year", "Date Added", "Rating"
   - Connect to selected_notify signal
   - Map selection to SortBy enum

4. Visual indicators
   - Show current sort option in dropdown
   - Use #[watch] macro for reactive updates

5. Styling
   - Use standard GNOME/libadwaita styling
   - Ensure proper spacing and alignment

6. Testing
   - Test all sort options work correctly
   - Verify search functionality remains intact
   - Check keyboard shortcuts still function


## Implementation Notes

## Implementation Notes

Added sort dropdown and search functionality to the library page UI:

1. **Removed duplicate header issue** - Initially added an adw::HeaderBar which caused duplicate headers with the main window. Fixed by integrating controls directly into the content area as a toolbar.

2. **Sort dropdown implementation** - Added gtk::DropDown with Title, Year, Date Added, and Rating options. Connected to existing SetSortBy handler which triggers library refresh with new sort order.

3. **Search functionality** - Kept existing floating search bar overlay design. Search button added to toolbar for visibility. Keyboard shortcuts (/ and Esc) remain functional.

4. **Visual indicators** - Dropdown automatically shows current sort option using #[watch] macro for reactive updates.

5. **Styling** - Used standard GTK controls with minimal styling. Toolbar positioned at top of content area with proper spacing.

Modified files:
- src/platforms/relm4/components/pages/library.rs

All acceptance criteria met and tested. Sorting works correctly, search bar is accessible via button or keyboard shortcut, and visual indicators update properly.
