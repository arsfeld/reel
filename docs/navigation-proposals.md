# Navigation Proposals for Reel

## Current State

### What We Have
- **Sidebar with libraries**: Each source's libraries are listed in a collapsible sidebar
- **Home button**: Shows recently added content from all sources (being improved to show per-source sections)
- **Sorting**: Title, Year, Date Added, Rating (implemented in library view)
- **Text filtering**: Search within library by title
- **Source management**: Dedicated page for adding/managing servers

### What We Lost in GTK→Relm4 Transition
- **Unwatched indicators**: Need to restore with improved visual design

## Research: How Other Apps Handle Navigation

### Key Patterns from Major Apps

**Plex**: Tabs per library (Recommended | Library | Collections | Categories)
- Genre browsing built into navigation
- Unwatched counts on poster badges
- Smart filters remembered per library

**Jellyfin**: Home sections with Continue Watching and Next Up
- Collections span across libraries
- Filters for favorites and quality (4K, HD)

**Infuse**: Smart Filters create instant views
- Unwatched, Recently Added as one-click presets
- Genre/rating/year filters
- Saved filters become home screen shortcuts

**Netflix**: Simplified top-level navigation
- Categories moved under search to reduce clutter
- "My Netflix" consolidates personal content

## Proposed Improvements

### 1. Predefined Filter Tabs (Quick Access Views)

Add horizontal tabs at the top of each library view:

```
[All] [Unwatched] [Recently Added] [Genres] [Years]
```

**Implementation Details**:
- These are instant filters on cached data (not server queries)
- Work in combination with existing sort options
- Remember last selected tab per library
- Simple, non-configurable - same tabs for all libraries

**Genre and Year Sub-filters**:
- Clicking "Genres" shows a dropdown/popover with genre chips
- Clicking "Years" shows a decade selector → year selector
- Multiple genres can be selected (OR logic)

### 2. Smart Collections

Automatic collections based on metadata patterns:

**Auto-Generated Collections**:
- **By Franchise**: Star Wars, Marvel, Harry Potter (via TMDB collection data)
- **By Genre Combinations**: "Sci-Fi Thrillers", "Romantic Comedies"
- **By Era**: "80s Movies", "2020s Shows"
- **By Quality**: "4K Content", "HDR Content"
- **By Status**: "In Progress", "New Seasons Available"

**Implementation**:
- Collections appear in sidebar under a "Collections" section
- Generated during sync, stored in database
- Click to view as filtered library
- No manual creation/editing in Phase 1 (keep it simple)

### 3. Unwatched Indicator (Visual Enhancement)

Restore and enhance the unwatched indicator:

**Design Specification**:
- **Position**: Top-right corner of media card
- **Visual**: Glowing dot with animated pulse effect
- **Color**: Bright blue or green (theme-aware)
- **Shows**: Display count badge (e.g., "3" for 3 unwatched episodes)
- **Animation**: Subtle glow pulse using CSS animations

```css
.unwatched-indicator {
    background: radial-gradient(circle, rgba(52,199,89,1) 0%, rgba(52,199,89,0.6) 50%);
    box-shadow: 0 0 10px rgba(52,199,89,0.8), 0 0 20px rgba(52,199,89,0.4);
    animation: pulse-glow 2s infinite;
}

@keyframes pulse-glow {
    0%, 100% { transform: scale(1); opacity: 1; }
    50% { transform: scale(1.1); opacity: 0.9; }
}
```

### 4. Enhanced Home Page Structure

Home page will show sections from each source:

**Section Types** (per source):
1. **Continue Watching** - Items with progress > 0
2. **Recently Added** - Last 30 days
3. **New Episodes** - Shows with unwatched episodes
4. **Recommended** - Based on most watched genres

**Display Rules**:
- Each source gets its own section group
- Sections auto-hide when empty
- Maximum 10 items per section (horizontal scroll)
- Source name as section header

## Implementation Priority

### Phase 1: Core Improvements (Immediate)

1. **Restore Unwatched Indicators**
   - Add glowing dot with CSS animation
   - Show unwatched count for shows
   - Database already tracks watched status

2. **Add Predefined Filter Tabs**
   - Implement All, Unwatched, Recently Added
   - Store selection in preferences
   - Use existing filter infrastructure

3. **Complete Home Page Sections**
   - Fix section replacement bug
   - Add per-source section groups
   - Implement "New Episodes" section

### Phase 2: Smart Features (Next Sprint)

1. **Genre/Year Quick Filters**
   - Add genre dropdown to library view
   - Implement year range selector
   - Cache genre lists during sync

2. **Smart Collections**
   - Auto-generate during sync
   - TMDB collection integration
   - Add Collections section to sidebar

3. **Search Improvements**
   - Add filter chips to search results
   - Search across multiple sources
   - Include genres in search

## Design Decisions

### Why Tabs Instead of Dropdown?
- **Discoverability**: Users see all options immediately
- **Speed**: One click instead of two
- **Mobile-friendly**: Easier to tap than small dropdown

### Why No Configuration?
- **Simplicity**: Reduces decision fatigue
- **Consistency**: Same experience for all users
- **Maintenance**: Fewer edge cases to handle
- **GNOME Philosophy**: Opinionated defaults that work well

### Why Smart Collections?
- **Discovery**: Surface content users might miss
- **Organization**: Automatic grouping without manual work
- **Cross-source**: Collections work across all backends

## Technical Notes

### Performance Considerations
- All filters operate on cached SQLite data
- Genre lists built during sync, stored in database
- Unwatched status tracked in `playback_progress` table
- Smart collections generated as virtual views (not duplicated data)

### Database Schema Additions Needed
```sql
-- For genre filtering
CREATE TABLE media_genres (
    media_item_id TEXT,
    genre TEXT,
    PRIMARY KEY (media_item_id, genre)
);
CREATE INDEX idx_genre ON media_genres(genre);

-- For smart collections
CREATE TABLE smart_collections (
    id TEXT PRIMARY KEY,
    name TEXT,
    type TEXT, -- 'franchise', 'genre_combo', 'era', 'quality'
    query_params TEXT -- JSON with filter criteria
);
```

## Success Metrics

1. **Discoverability**: Users find content faster
2. **Engagement**: Increased use of filter tabs
3. **Performance**: Instant filter switching (<100ms)
4. **Clarity**: Unwatched content immediately visible

## Next Steps

1. Implement unwatched indicator with glow effect
2. Add filter tabs to library view component
3. Create database schema for genres and collections
4. Design popover UI for genre selection
5. Test performance with large libraries (10k+ items)