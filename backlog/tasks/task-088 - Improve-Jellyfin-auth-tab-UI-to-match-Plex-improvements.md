---
id: task-088
title: Improve Jellyfin auth tab UI to match Plex improvements
status: Done
assignee:
  - '@claude'
created_date: '2025-09-16 17:50'
updated_date: '2025-09-16 17:58'
labels:
  - ui
  - auth
  - ux
  - jellyfin
dependencies: []
priority: high
---

## Description

The Jellyfin authentication tab currently only shows a basic hostname input field, which is not user-friendly. It needs similar improvements to what was done for Plex - better layout, clearer messaging, and a more polished UI that guides users through the authentication process.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Add a StatusPage with icon and clear title for Jellyfin connection
- [x] #2 Improve the server URL input with better labels and helper text
- [x] #3 Make Quick Connect the primary option with clear instructions
- [x] #4 Reorganize username/password login as a secondary option
- [x] #5 Add helpful descriptions explaining what each auth method does
- [x] #6 Ensure consistent styling with the improved Plex tab
- [x] #7 Maintain all existing Jellyfin authentication functionality
<!-- AC:END -->


## Implementation Plan

1. Analyze current Plex tab improvements to understand the new design patterns
2. Redesign Jellyfin tab to match Plex with StatusPage approach
3. Add better Quick Connect UI as primary option with clear code display
4. Improve username/password section as secondary option with cleaner layout
5. Add helpful descriptions for each auth method
6. Ensure consistent styling and proper spacing
7. Test all Jellyfin authentication paths


## Implementation Notes

## Implementation Summary

Successfully redesigned the Jellyfin authentication tab to match the improved Plex design patterns:

### Key Changes Made:

1. **Initial StatusPage**: Added a clean, welcoming StatusPage with Jellyfin icon and clear title when no server URL is entered yet, matching the Plex initial state

2. **Improved Server URL Input**: 
   - Added better helper text with example URLs (local IP and domain examples)
   - Used adw::Clamp to constrain width for better visual focus
   - Added input hints to disable spellcheck

3. **Quick Connect as Primary**:
   - Made Quick Connect the recommended option with "(Recommended)" label
   - Added clear step-by-step instructions on how to use Quick Connect
   - Improved code display with better styling (title-1 and accent classes)
   - Added visual instructions for where to enter the code in Jellyfin dashboard

4. **Username/Password as Secondary**:
   - Reorganized as a secondary option with clear labeling
   - Used consistent PreferencesGroup styling
   - Changed button text from "Connect" to "Sign In" for clarity

5. **Helpful Descriptions**:
   - Added descriptive subtitles for each auth method
   - Quick Connect: "The easiest way to connect - no password needed"
   - Username/Password: "Traditional login with your Jellyfin credentials"

6. **Consistent Styling**:
   - Used same StatusPage patterns as Plex
   - Applied consistent spacing (24px between major sections)
   - Used same CSS classes (dim-label, caption, heading, etc.)
   - Matched button styling (suggested-action, pill classes)

### Files Modified:
- `/src/platforms/relm4/components/dialogs/auth_dialog.rs`: Lines 316-542

The implementation maintains full backward compatibility with existing Jellyfin authentication while significantly improving the user experience to match the polished Plex authentication flow.
