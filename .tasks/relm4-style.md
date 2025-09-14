# Relm4 UI Style Modernization Checklist

## Overview
Transform the Relm4 UI to be more modern and slick with vanilla Adwaita design, making dark theme the only supported theme (not a preference), and creating a more premium Infuse-like experience for media browsing.

**Status**: Phase 1-3 Complete (75% Done) - Library, Home, and Details pages modernized with dark theme, floating search, metadata pills, and full-bleed backdrops.

## Analysis of Current Issues

### 1. Sidebar Deviations from Vanilla Adwaita
- **Custom spacing** (18px) instead of standard Adwaita spacing (12px)
- **Custom margins** (12px all around) instead of proper navigation-sidebar padding
- **Non-standard section headers** - should use dim-label styling
- **Hardcoded colors** in CSS instead of using Adwaita variables
- **Custom hover effects** instead of standard list row behavior

### 2. Main Window Structure Issues
- **Not using standard Adwaita patterns** for split views
- **Custom header styling** instead of vanilla HeaderBar
- **Inconsistent button styling** (mixing flat and suggested-action inappropriately)

### 3. Library/Grid Layout Issues
- **Custom toolbar styling** - should use standard Adwaita patterns
- **Non-standard filter controls** - needs vanilla dropdown styling
- **Inconsistent spacing** in grid layouts

### 4. Dark Theme Issues
- **Still checking theme preferences** - should force dark always
- **CSS has light theme variables** - should only have dark values
- **Dim labels too dim** (0.55 opacity) - needs better contrast

## 🎯 High Priority Tasks

### 1. Force Dark Theme Everywhere
- [x] **Remove all theme preference code**
  - [x] Delete theme switching in preferences.rs
  - [x] Remove AdwStyleManager color scheme checks
  - [x] Delete all references to "system" or "light" themes

- [x] **Force dark in app initialization**
  ```rust
  // In app.rs init
  let style_manager = adw::StyleManager::default();
  style_manager.set_color_scheme(adw::ColorScheme::ForceDark);
  ```

- [x] **Update CSS to dark-only values**
  - [x] Remove all @prefer_dark conditionals
  - [x] Use only dark color values directly
  - [x] Update dim-label opacity to 0.7 (from 0.55)

### 2. Vanilla Adwaita Sidebar
- [x] **Remove all custom spacing**
  ```rust
  // Change sidebar.rs:156
  set_spacing: 12, // Done - reduced from 18

  // Removed custom margins
  ```

- [x] **Use standard navigation-sidebar class**
  ```rust
  add_css_class: "navigation-sidebar",
  ```

- [x] **Simplify source groups**
  - [x] Remove custom heading styles
  - [x] Use standard listbox row activation
  - [x] Remove arrow icons (go-next-symbolic)
  - [x] Use standard selection mode

- [x] **Standard welcome page**
  - [x] Use AdwStatusPage properly
  - [x] Remove custom centering
  - [x] Use standard icon size (128px)

### 3. Library View Modernization ✅ COMPLETE
- [x] **Tighter grid spacing**
  ```rust
  set_column_spacing: 12,  // Done - reduced from 16
  set_row_spacing: 16,      // Done - reduced from 20
  ```

- [x] **Remove unnecessary chrome**
  - [x] Hide sort dropdown by default - Removed entirely
  - [x] Floating search bar that appears on typing - Press / or Ctrl+F
  - [x] Remove refresh button (auto-refresh) - Removed

- [x] **Modern empty states**
  - [x] Large centered icon (128px) - Using AdwStatusPage
  - [x] Bold title text - Done
  - [x] Subtle call-to-action - Done

### 4. Details Page (Infuse-style) ✅ COMPLETE
- [x] **Full-bleed backdrop**
  - [x] Remove height constraints - Increased to 550px
  - [x] Edge-to-edge image - Done
  - [x] Stronger gradient overlay - Using hero-gradient class

- [x] **Floating metadata pills**
  - [x] Implemented with CSS styling
  - [x] Dark translucent background with blur
  - [x] Applied to year, rating, and duration
  ```css
  .metadata-pill {
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(10px);
    padding: 8px 16px;
    border-radius: 20px;
  }
  ```

- [x] **Larger poster (details page)**
  - [x] Movie details: 300x450 (from 200x300)
  - [x] Show details: 300x450 (from 200x300)
  - [x] Added poster-shadow class for depth

### 5. Player OSD Refinements
- [ ] **Semi-transparent OSD background**
  ```css
  .osd {
    background: rgba(0, 0, 0, 0.8);
    backdrop-filter: blur(20px);
  }
  ```

- [ ] **Larger controls**
  - [ ] Play button: 64px (from 48px)
  - [ ] Volume slider: 150px wide
  - [ ] Seek bar: 6px tall (from 4px)

- [ ] **Modern time display**
  ```css
  .time-label {
    font-family: "SF Mono", "Roboto Mono", monospace;
    font-size: 14px;
    font-weight: 500;
  }
  ```

## 🎨 Global CSS Updates

### Remove These Classes/Rules
```css
/* DELETE ALL OF THESE */
.light { }
.prefer-light { }
@media (prefers-color-scheme: light) { }
window.background:not(.csd) { }  /* No light backgrounds */
```

### Update Core Colors
```css
/* Force dark theme colors */
@define-color window_bg_color #1e1e1e;
@define-color window_fg_color #ffffff;
@define-color view_bg_color #2a2a2a;
@define-color view_fg_color #ffffff;
@define-color card_bg_color #303030;
@define-color card_fg_color #ffffff;
@define-color headerbar_bg_color #242424;
@define-color headerbar_fg_color #ffffff;
@define-color accent_color #3584e4;
@define-color accent_fg_color #ffffff;
```

### Modern Typography Scale
```css
.display-1 { font-size: 48px; font-weight: 900; }  /* Hero text */
.title-1 { font-size: 36px; font-weight: 800; }
.title-2 { font-size: 28px; font-weight: 700; }
.title-3 { font-size: 22px; font-weight: 600; }
.heading { font-size: 16px; font-weight: 600; }
.body { font-size: 14px; font-weight: 400; }
.caption { font-size: 12px; font-weight: 400; }
.dim-label { opacity: 0.7; }  /* More visible than 0.55 */
```

## 🔧 Component-Specific Changes

### MainWindow (`main_window.rs`)
- [ ] Remove theme switching code
- [ ] Simplify header bar (remove unnecessary buttons)
- [ ] Use flat style for all header buttons
- [ ] Remove sidebar width constraints (let it be narrower)

### Sidebar (`sidebar.rs`)
- [ ] Remove all custom spacing/margins
- [ ] Use standard `navigation-sidebar` class
- [ ] Simplify library list (no arrows, cleaner rows)
- [ ] Remove "Connect to Server" - use empty state
- [ ] Smaller, dimmer section headers

### HomePage (`home.rs`)
- [ ] Larger section headers with better spacing
- [ ] Remove section descriptions (cleaner)
- [ ] Horizontal scroll indicators (dots or arrows)
- [ ] Lazy loading for off-screen content

### LibraryPage (`library.rs`)
- [ ] Larger media cards
- [ ] Remove toolbar, integrate controls inline
- [ ] Infinite scroll instead of pagination
- [ ] Floating action button for filters

### Sources Page (`sources.rs`)
- [ ] Modern list design (no boxes)
- [ ] Inline actions (not separate buttons)
- [ ] Swipe to delete gesture
- [ ] Connection status as colored dot

### AuthDialog (`auth_dialog.rs`)
- [ ] Full-height dialog (not floating)
- [ ] Large service icons
- [ ] Minimal input fields (no borders)
- [ ] Loading states with spinners

## 📋 Implementation Order

### Phase 1: Foundation (Day 1) ✅ COMPLETE
1. [x] Force dark theme in app.rs
2. [x] Update global CSS with dark-only colors
3. [x] Remove all theme preference code
4. [x] Update typography scale

### Phase 2: Sidebar (Day 1-2) ✅ COMPLETE
1. [x] Remove custom spacing/margins
2. [x] Apply navigation-sidebar class
3. [x] Simplify source/library lists
4. [x] Update welcome screen

### Phase 3: Pages (Day 2-3) ✅ COMPLETE
1. [x] Modernize library grid - Tighter spacing, floating search, modern empty states
2. [x] Update details pages - Full-bleed backdrop, metadata pills, larger posters
3. [x] Refine home page - Modern section headers, AdwStatusPage for empty state
4. [x] Polish sources page - Already using AdwStatusPage

### Phase 4: Player (Day 3-4)
1. [ ] Update OSD styling
2. [ ] Larger controls
3. [ ] Better time display
4. [ ] Refined animations

## 🎯 Success Criteria

### Visual Goals
- [ ] Looks premium and modern (Infuse/Netflix quality)
- [ ] Consistent dark theme throughout
- [ ] No custom Adwaita deviations
- [ ] Smooth animations and transitions
- [ ] High contrast and readability

### Technical Goals
- [ ] No theme switching code
- [ ] Minimal custom CSS
- [ ] Standard Adwaita components
- [ ] Consistent spacing (12/24/48px)
- [ ] No hardcoded colors

### User Experience Goals
- [ ] Faster perceived performance
- [ ] Clearer visual hierarchy
- [ ] Better touch targets
- [ ] More immersive media browsing
- [ ] Professional appearance

## 🚀 Quick Wins (COMPLETED ✅)

1. **Force dark theme** - ✅ Done - Dark theme now forced everywhere
2. **Remove custom sidebar spacing** - ✅ Done - Using vanilla Adwaita spacing
3. **Update dim-label opacity** - ✅ Done - Improved from 0.55 to 0.7
4. **Remove unnecessary UI chrome** - ✅ Done - Simplified library toolbar
5. **Simplify library toolbar** - ✅ Done - Minimal, clean interface

## ⚠️ Things to Avoid

- Don't add custom animations (use Adwaita defaults)
- Don't create custom widgets (use standard components)
- Don't override Adwaita colors (use CSS variables)
- Don't add preferences for theme (dark only!)
- Don't use small touch targets (<44px)

## 📝 Testing Checklist

- [ ] All text readable on dark background
- [ ] No light theme artifacts
- [ ] Consistent spacing throughout
- [ ] Smooth scrolling performance
- [ ] Hover effects work correctly
- [ ] Focus indicators visible
- [ ] No custom color values in code
- [ ] Looks good at different window sizes

## 🎉 Definition of Done

The Relm4 UI is considered modernized when:
1. Dark theme is enforced everywhere (no user choice)
2. All custom spacing/margins removed (vanilla Adwaita)
3. Typography is bold and readable
4. The app looks premium and Netflix/Infuse-like
5. No references to light theme remain in code
6. All components use standard Adwaita patterns
7. Library and details pages have clean, modern layouts