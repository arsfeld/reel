# Adwaita UI Implementation Checklist

## 🎯 Goal: Modern Adwaita Styling for Relm4

Ensure the Relm4 implementation matches the modern, polished appearance of the GTK version with proper Adwaita styling, dark theme support, and contemporary GNOME design patterns.

## 🎨 Core Adwaita Components

### Window Structure
- [ ] **AdwApplicationWindow** - Replace basic GtkWindow
- [ ] **AdwHeaderBar** - Modern header with integrated title/subtitle
- [ ] **AdwToolbarView** - Proper toolbar container with Raised/Flat styles
- [ ] **AdwNavigationSplitView** - Responsive sidebar/content split
- [ ] **AdwLeaflet** - Mobile-responsive navigation (if needed)
- [ ] **Window Controls** - Proper minimize/maximize/close integration

### Navigation & Layout
- [ ] **AdwNavigationPage** - Proper page container with titles
- [ ] **AdwViewStack** - Smooth page transitions with animations
- [ ] **AdwTabView** - Modern tab interface (if used)
- [ ] **AdwCarousel** - Smooth horizontal scrolling for media sections
- [ ] **AdwFlap** - Collapsible sidebar behavior
- [ ] **AdwSplitButton** - Dropdown buttons where appropriate

### Content Containers
- [ ] **AdwPreferencesWindow** - Settings/preferences dialog
- [ ] **AdwPreferencesPage** - Settings page container
- [ ] **AdwPreferencesGroup** - Grouped settings sections
- [ ] **AdwActionRow** - List items with actions
- [ ] **AdwExpanderRow** - Collapsible list sections
- [ ] **AdwComboRow** - Dropdown selections in lists
- [ ] **AdwSpinRow** - Number inputs in lists

### Cards & Content
- [ ] **AdwClamp** - Content width limiting for readability
- [ ] **AdwClampScrollable** - Scrollable content with width limits
- [ ] **AdwStatusPage** - Empty states and error pages
- [ ] **AdwToastOverlay** - Toast notifications
- [ ] **AdwBanner** - Informational banners

### Buttons & Controls
- [ ] **AdwSplitButton** - Action + dropdown combinations
- [ ] **AdwButtonContent** - Icons + labels in buttons
- [ ] **Proper button styles**: `.suggested-action`, `.destructive-action`, `.flat`
- [ ] **AdwSpinner** - Loading indicators

### Media-Specific
- [ ] **GtkOverlay** - Video player controls overlay
- [ ] **GtkRevealer** - Auto-hiding controls animation
- [ ] **GtkScale** - Seek bar and volume controls
- [ ] **OSD Controls** - On-screen display styling

## 🌙 Dark Theme Support

### Theme Detection
- [ ] **AdwStyleManager** - Automatic theme detection
- [ ] **gsettings integration** - Follow system theme preference
- [ ] **Manual theme toggle** - User override in preferences
- [ ] **Theme persistence** - Remember user preference

### Dark Theme Colors
- [ ] **Background colors** - Proper dark surface colors
- [ ] **Text colors** - High contrast on dark backgrounds
- [ ] **Card backgrounds** - Elevated surface colors
- [ ] **Border colors** - Subtle borders in dark mode
- [ ] **Accent colors** - System accent color integration
- [ ] **Selection colors** - Proper highlight colors

### Media Player Dark Theme
- [ ] **Video area** - Pure black background
- [ ] **OSD controls** - Semi-transparent dark overlay
- [ ] **Seek bar** - High contrast in dark mode
- [ ] **Control buttons** - Visible on dark video content
- [ ] **Text overlays** - White text on dark backgrounds

## 🎭 CSS Classes & Styling

### Typography Classes
- [ ] **`.title-1`** - Large page titles
- [ ] **`.title-2`** - Section headers
- [ ] **`.title-3`** - Subsection headers
- [ ] **`.title-4`** - Small headers
- [ ] **`.heading`** - General headings
- [ ] **`.body`** - Body text
- [ ] **`.caption`** - Small text
- [ ] **`.caption-heading`** - Small headings
- [ ] **`.dim-label`** - Secondary/muted text

### Component Classes
- [ ] **`.card`** - Elevated content cards
- [ ] **`.toolbar`** - Toolbar styling
- [ ] **`.sidebar`** - Sidebar specific styling
- [ ] **`.view`** - Main content view
- [ ] **`.navigation-sidebar`** - Navigation panel
- [ ] **`.content`** - Main content area

### State Classes
- [ ] **`.suggested-action`** - Primary action buttons
- [ ] **`.destructive-action`** - Dangerous action buttons
- [ ] **`.flat`** - Borderless buttons
- [ ] **`.circular`** - Round buttons
- [ ] **`.pill`** - Pill-shaped buttons/badges
- [ ] **`.osd`** - On-screen display elements
- [ ] **`.toolbar`** - Toolbar elements
- [ ] **`.linked`** - Grouped controls

### Media-Specific Classes
- [ ] **`.media-card`** - Movie/show cards
- [ ] **`.media-grid`** - Grid layout for media
- [ ] **`.media-list`** - List layout for media
- [ ] **`.progress-bar`** - Watch progress indicators
- [ ] **`.overlay-controls`** - Video player overlays
- [ ] **`.seek-bar`** - Video seek controls
- [ ] **`.volume-control`** - Volume controls

## 📱 Responsive Design

### Breakpoints
- [ ] **Mobile (< 640px)** - Single column, compact spacing
- [ ] **Tablet (640-1024px)** - Two columns, medium spacing
- [ ] **Desktop (> 1024px)** - Multi-column, full spacing
- [ ] **Large Desktop (> 1440px)** - Max width clamping

### Adaptive Layouts
- [ ] **Sidebar collapse** - Auto-hide on small screens
- [ ] **Grid columns** - Responsive media grid (2-8 columns)
- [ ] **Card sizing** - Adaptive card dimensions
- [ ] **Touch targets** - Minimum 44px touch areas
- [ ] **Navigation** - Mobile-friendly navigation patterns

## 🎬 Media Player UI

### Player Chrome Management
- [ ] **Hide header bar** - Remove all window chrome during playback
- [ ] **Flat toolbar style** - Remove raised appearance
- [ ] **Window state preservation** - Save/restore size and position
- [ ] **Aspect ratio sizing** - Resize window to match video
- [ ] **Cursor auto-hide** - Hide cursor after 3 seconds inactivity

### OSD Controls
- [ ] **Semi-transparent overlay** - Dark overlay with proper opacity
- [ ] **Control visibility** - Auto-hide after 3 seconds
- [ ] **Seek bar styling** - Proper progress bar with hover effects
- [ ] **Button styling** - Circular OSD buttons
- [ ] **Time display** - Formatted time labels (H:MM:SS)
- [ ] **Volume controls** - Proper volume button and popover

### Player State Indicators
- [ ] **Loading spinner** - During stream initialization
- [ ] **Buffering indicator** - During stream buffering
- [ ] **Error states** - Clear error messaging
- [ ] **Quality indicator** - Stream quality display
- [ ] **Subtitle indicator** - Subtitle track display

## 🏠 Page-Specific Styling

### Main Window
- [ ] **AdwHeaderBar** - Replace GtkHeaderBar
- [ ] **Window title** - Dynamic title based on current page
- [ ] **Navigation buttons** - Proper back/forward styling
- [ ] **Search bar** - Integrated search in header
- [ ] **User menu** - Account/settings dropdown

### Sidebar
- [ ] **AdwNavigationPage** - Proper sidebar container
- [ ] **Source groups** - Collapsible source sections
- [ ] **Connection indicators** - Online/offline status icons
- [ ] **Library counts** - Badge numbers for item counts
- [ ] **Selected state** - Proper selection highlighting

### Homepage
- [ ] **Section headers** - Proper typography hierarchy
- [ ] **Media carousels** - Horizontal scrolling sections
- [ ] **Continue Watching** - Progress bars on cards
- [ ] **Recently Added** - Date badges
- [ ] **Empty states** - AdwStatusPage for no content

### Library Page
- [ ] **Filter toolbar** - Search and sort controls
- [ ] **View toggle** - Grid/list view switcher
- [ ] **Virtual scrolling** - Performance optimized lists
- [ ] **Loading states** - Skeleton loading for cards
- [ ] **Infinite scroll** - Progressive loading indicator

### Details Pages
- [ ] **Hero sections** - Large backdrop images
- [ ] **Metadata pills** - Genre, rating, year badges
- [ ] **Action buttons** - Play, add to list, mark watched
- [ ] **Cast grids** - Person cards with photos
- [ ] **Episode lists** - Season/episode navigation
- [ ] **Progress tracking** - Watch progress indicators

### Settings/Preferences
- [ ] **AdwPreferencesWindow** - Modern preferences dialog
- [ ] **Grouped sections** - AdwPreferencesGroup containers
- [ ] **Switch controls** - Boolean preferences
- [ ] **Dropdown selections** - ComboRow for options
- [ ] **File choosers** - Path selection controls

## 🔧 Implementation Status

### Current Relm4 Component Status
- [ ] **ReelApp** - Needs AdwApplication integration
- [ ] **MainWindow** - Replace GtkWindow with AdwApplicationWindow
- [ ] **Sidebar** - Add AdwNavigationPage wrapper
- [ ] **HomePage** - Add proper section styling
- [ ] **Library** - Implement responsive grid
- [ ] **PlayerPage** - Add OSD controls styling
- [ ] **MovieDetails** - Add hero section styling
- [ ] **ShowDetails** - Add episode grid styling

### Component Auditing Tasks
- [ ] **Audit MainWindow** - Check for missing Adwaita components
- [ ] **Audit Sidebar** - Verify modern list styling
- [ ] **Audit HomePage** - Check section headers and carousels
- [ ] **Audit Library** - Verify grid responsiveness
- [ ] **Audit Player** - Check OSD control styling
- [ ] **Audit Details** - Verify hero section implementation

### CSS Integration
- [ ] **Load Adwaita CSS** - Ensure proper stylesheet loading
- [ ] **Custom CSS file** - Create media-specific styles
- [ ] **CSS class application** - Apply classes to all components
- [ ] **Theme switching** - Implement runtime theme changes

## 🧪 Testing & Validation

### Visual Testing
- [ ] **Screenshot comparison** - Before/after GTK vs Relm4
- [ ] **Theme switching** - Test light/dark theme transitions
- [ ] **Responsive testing** - Test all breakpoints
- [ ] **Component states** - Test hover, focus, disabled states

### Functionality Testing
- [ ] **Window chrome hiding** - Test player chrome management
- [ ] **Navigation** - Test all navigation transitions
- [ ] **Responsive behavior** - Test window resizing
- [ ] **Theme persistence** - Test theme setting survival

### Performance Testing
- [ ] **Rendering performance** - Ensure smooth animations
- [ ] **Memory usage** - Check for CSS-related leaks
- [ ] **Startup time** - Verify no styling overhead

## 🎯 Success Criteria

### Visual Parity
- [ ] **Identical appearance** - Relm4 looks exactly like GTK version
- [ ] **Proper dark theme** - Full dark mode support
- [ ] **Modern chrome** - Contemporary GNOME appearance
- [ ] **Responsive design** - Works on all screen sizes

### User Experience
- [ ] **Smooth transitions** - All animations work properly
- [ ] **Consistent behavior** - All interactions match GTK version
- [ ] **Accessibility** - Proper focus indicators and navigation
- [ ] **Touch support** - Works well on touch devices

### Code Quality
- [ ] **Clean CSS** - Organized and maintainable stylesheets
- [ ] **Proper component usage** - Correct Adwaita widget usage
- [ ] **Performance** - No styling-related slowdowns
- [ ] **Documentation** - Clear styling guidelines

## 📝 Implementation Notes

### Key Files to Update
- `src/platforms/relm4/app.rs` - AdwApplication setup
- `src/platforms/relm4/components/main_window.rs` - AdwApplicationWindow
- All component files - Add proper Adwaita widgets
- CSS files - Create comprehensive stylesheets
- Theme handling - Add AdwStyleManager integration

### Dependencies to Add
- Ensure `libadwaita-1-dev` in Nix environment
- Verify GTK4/Adwaita Rust bindings
- Check for any missing Adwaita widget bindings

### Migration Strategy
1. **Audit current components** - Identify styling gaps
2. **Replace basic widgets** - Upgrade to Adwaita equivalents
3. **Add CSS classes** - Apply proper styling classes
4. **Implement dark theme** - Add theme detection and switching
5. **Test visual parity** - Compare with GTK version
6. **Optimize performance** - Ensure no styling overhead

---

## 🏁 Completion Checklist

- [ ] All Adwaita components implemented
- [ ] Dark theme fully functional
- [ ] CSS classes properly applied
- [ ] Responsive design working
- [ ] Visual parity with GTK version achieved
- [ ] Performance validated
- [ ] User testing completed

**Target**: Modern, polished Adwaita-styled Relm4 implementation that matches or exceeds the GTK version's visual quality and user experience.