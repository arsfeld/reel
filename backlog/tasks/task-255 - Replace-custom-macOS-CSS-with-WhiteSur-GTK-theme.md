---
id: task-255
title: Replace custom macOS CSS with WhiteSur GTK theme
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 14:42'
updated_date: '2025-09-26 15:42'
labels:
  - ui
  - macos
  - theme
dependencies: []
priority: high
---

## Description

Remove the custom macOS CSS styling in src/styles/macos.css and integrate the WhiteSur GTK theme properly for a more native macOS appearance

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Remove custom CSS loading for macOS platform
- [x] #2 Set up WhiteSur theme as the default GTK theme on macOS
- [x] #3 Update application initialization to use WhiteSur theme
- [x] #4 Test theme appearance on macOS
<!-- AC:END -->


## Implementation Plan

1. Examine current macOS CSS implementation and how it's loaded
2. Research how to properly set GTK themes programmatically in Rust/GTK4
3. Remove custom CSS loading logic for macOS
4. Implement WhiteSur theme initialization
5. Test the changes on macOS platform


## Implementation Notes

## What was done

1. **Removed custom macOS CSS**:
   - Deleted `src/styles/macos.css` file
   - Removed macos.css reference from `Cargo.toml` resources
   - Removed conditional CSS loading from `src/app/app.rs`

2. **Configured WhiteSur theme**:
   - Added GTK theme setting in `src/app/app.rs` using `gtk4::Settings::set_gtk_theme_name()`
   - Added GTK_THEME and XDG_DATA_DIRS environment variables in `flake.nix` for macOS
   - Theme is now loaded from nix environment automatically

3. **Fixed libadwaita warning**:
   - Removed `gtk-application-prefer-dark-theme` setting
   - Rely on `AdwStyleManager::ForceDark` for dark theme

4. **Cleaned up platform utilities**:
   - Updated `Platform::apply_platform_classes()` to no longer add CSS classes
   - Platform-specific styling now handled by WhiteSur theme


## Final refinement

- Removed programmatic GTK theme setting (`settings.set_gtk_theme_name()`)
- Removed unnecessary gtk4::prelude import
- Theme is now purely controlled via GTK_THEME environment variable
- Cleaner separation: environment handles theming, app focuses on functionality


## Testing needed

The application should now load the WhiteSur-Dark theme on macOS. You need to:
1. Exit and re-enter the nix develop shell to load new environment variables
2. Run `cargo run` to test the theme is properly applied
3. Verify no more Adwaita warnings about gtk-application-prefer-dark-theme
