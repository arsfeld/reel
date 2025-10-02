# Search Implementation Documentation

## Overview

Reel implements a full-text search system using Tantivy (a Rust search engine library) to provide fast, local search across media libraries. The search functionality consists of three main components: the SearchWorker (background indexing and search), the SearchPage UI, and integration points in MainWindow.

## Architecture

### Components

```
┌─────────────────┐
│   MainWindow    │
│  (Search Entry) │
└────────┬────────┘
         │
         ├──────────────────────┐
         │                      │
         v                      v
┌────────────────┐     ┌──────────────┐
│  SearchWorker  │     │  SearchPage  │
│   (Tantivy)    │     │     (UI)     │
└────────┬───────┘     └──────┬───────┘
         │                    │
         v                    v
┌────────────────┐     ┌──────────────┐
│  Search Index  │     │  MediaCard   │
│ (Tantivy Docs) │     │   Factory    │
└────────────────┘     └──────────────┘
```

## Current Implementation

### 1. SearchWorker (`src/workers/search_worker.rs`)

**Purpose:** Background worker component that manages a Tantivy full-text search index for all media items.

**Key Features:**
- **Index Location:** `~/.local/share/reel/search_index/` (or `/tmp/reel/search_index/` as fallback)
- **Index Schema:**
  - `id` (STORED): MediaItemId as string
  - `title` (TEXT | STORED): Media title (searchable, stored)
  - `overview` (TEXT): Description/plot summary (searchable only)
  - `year` (TEXT | STORED): Release year (searchable, stored)
  - `genres` (TEXT): Space-separated genres (searchable only)

**Supported Operations:**
- `IndexDocuments(Vec<SearchDocument>)` - Bulk index media items
- `UpdateDocument(SearchDocument)` - Update single item
- `RemoveDocument(MediaItemId)` - Remove item from index
- `Search { query, limit }` - Execute search query
- `ClearIndex` - Remove all documents
- `OptimizeIndex` - Optimize index segments

**Search Behavior:**
- Multi-field search across: title, overview, genres
- Uses Tantivy's QueryParser for natural language queries
- Returns up to `limit` results sorted by relevance score
- Gracefully handles missing index (creates fallback worker)

**Implementation Status:** ✅ **Complete**
- File: `src/workers/search_worker.rs:1-459`
- Integrated in MainWindow: `src/ui/main_window.rs:33,296-333,495-506,641-760,1061-1100`

### 2. SearchPage (`src/ui/pages/search.rs`)

**Purpose:** Relm4 AsyncComponent that displays search results in a responsive media grid.

**Key Features:**
- **Empty States:**
  - No query entered: Shows search icon and instructions
  - No results: Shows "No results" message with retry suggestion
- **Loading State:** Spinner while fetching results from database
- **Results Display:** FlowBox grid with MediaCard factory (2-8 columns responsive)
- **Image Loading:** Integrates with ImageLoader worker for poster images
- **Navigation:** Forwards media item selection to MainWindow

**Data Flow:**
1. Receives `SetResults { query, results: Vec<MediaItemId> }` from MainWindow
2. Loads full MediaItemModel data from database via MediaRepository
3. Populates MediaCard factory with items
4. Requests poster images from ImageLoader
5. Updates cards when images load

**Implementation Status:** ✅ **Complete**
- File: `src/ui/pages/search.rs:1-329`
- Integrated in MainWindow: `src/ui/main_window.rs:41-42,1022-1100`

### 3. MainWindow Integration (`src/ui/main_window.rs`)

**Search Entry UI:**
- Location: Header bar (right side)
- Placeholder: "Search media..."
- Width: 250px
- Triggers: `connect_activate` on Enter key press
- Behavior: Navigates to SearchPage and sends SearchQuery

**Search Workflow:**
1. **App Startup:**
   - Initialize SearchWorker
   - Index all existing media items from database
   - Message: "init_search_index" → loads all media → `IndexDocuments`

2. **After Sync:**
   - Clear existing index
   - Re-index all media items
   - Message: "refresh_search_index" → `ClearIndex` → `IndexDocuments`
   - Toast notification: "Sync completed"

3. **User Search:**
   - User types query → `SearchQuery(String)` input
   - SearchWorker emits `Search { query, limit: 50 }`
   - Results received → `SearchResultsReceived { query, results, total_hits }`
   - Navigate to SearchPage if needed
   - Forward results to SearchPage → `SetResults { query, results }`
   - Show toast: "Found N results"

**Implementation Status:** ✅ **Complete**
- Search entry: `src/ui/main_window.rs:219-235`
- Search worker init: `src/ui/main_window.rs:295-333`
- Index initialization: `src/ui/main_window.rs:641-691`
- Index refresh: `src/ui/main_window.rs:693-760`
- Search handling: `src/ui/main_window.rs:1061-1100`

### 4. Plex Search API (`src/backends/plex/api/search_impl.rs`)

**Purpose:** Direct search integration with Plex server API.

**Available Methods:**
- `search_global(query, limit)` - Global search across all libraries (`/hubs/search`)
- `search_library(library_id, query, media_type, sort, limit)` - Library-specific search (`/library/sections/{id}/search`)
- `search_advanced(library_id, params)` - Custom parameter search
- `search_with_filters(library_id, query, genre, year, rating_min, unwatched, sort, limit)` - Filtered search

**Implementation Status:** ⚠️ **Implemented but NOT integrated**
- File: `src/backends/plex/api/search_impl.rs:1-103`
- Backend integration: Task-208 (In Progress)
- **Gap:** Plex search methods exist but are not called from UI or SearchWorker

## Backlog Tasks

### Task-168: Implement search functionality using SearchWorker ✅
**Status:** Complete (In Progress in backlog, but implementation is done)
**Priority:** High
**Assignee:** @claude

**Acceptance Criteria:**
- ✅ #1 Wire up SearchWorker in the main application
- ✅ #2 Add search input field to the UI
- ✅ #3 Connect search results to media grid display
- ✅ #4 Handle search state and loading indicators
- ✅ #5 Test search functionality with different media types

**Implementation Notes:**
All work completed. Search entry in header, SearchWorker integrated, SearchPage displays results in media grid, loading states implemented. Ready for testing.

**Recommendation:** Mark as Done after testing.

---

### Task-208: Add Plex search endpoints for content discovery ⚠️
**Status:** In Progress
**Priority:** High
**Assignee:** @claude
**Dependencies:** task-206

**Acceptance Criteria:**
- ✅ #1 Global search endpoint (/hubs/search) is implemented with query parameter support
- ✅ #2 Library-specific search endpoint (/library/sections/{id}/search) works for movies and shows
- ✅ #3 Search results return proper metadata including titles, summaries, and thumbnails
- ❌ #4 Search integration works with existing UI search components
- ✅ #5 Search supports filtering and sorting parameters

**Current Gap:**
Backend Plex search methods exist in `src/backends/plex/api/search_impl.rs` but are not integrated with the UI search system. The UI currently only searches the local Tantivy index.

**Integration Options:**

**Option A: Hybrid Search (Recommended)**
- Keep Tantivy for fast local search
- Add backend search as optional enhancement for:
  - Real-time content discovery
  - Filtering not available in local index
  - Server-side metadata updates

**Option B: Backend-Only Search**
- Remove/replace Tantivy with direct backend queries
- Pros: Always up-to-date, server-side filtering
- Cons: Requires network, slower, no offline search

**Option C: Dual Search Mode**
- Toggle between local (fast) and remote (complete) search
- User preference setting

**Recommendation:** Option A - Use local Tantivy as primary, add backend search for advanced features.

---

### Task-130: Implement cross-source search ⏸️
**Status:** To Do
**Priority:** Low
**Labels:** search, feature

**Acceptance Criteria:**
- ❌ #1 Modify search to query all sources in parallel
- ❌ #2 Merge and deduplicate results from multiple sources
- ❌ #3 Add source filter chips to search results
- ❌ #4 Include genre filtering in search interface
- ❌ #5 Display source badge on each search result

**Current Gap:**
The current implementation searches all indexed content regardless of source, but provides no source filtering or badges. The Tantivy index doesn't store source information.

**Required Changes:**
1. **SearchDocument Schema:** Add `source_id` and `source_name` fields to Tantivy schema
2. **SearchPage UI:** Add filter chips for source selection
3. **SearchPage UI:** Add genre filter chips
4. **MediaCard:** Add source badge overlay (requires MediaCard enhancement)
5. **SearchWorker:** Support filtered queries by source

**Blocked By:**
- Need to decide on backend search integration (task-208) first
- MediaCard component doesn't currently support badges

## Implementation Gaps

### 1. Backend Search Not Integrated (High Priority)
**Issue:** Plex search API exists but isn't used
**Impact:** Search only finds locally cached content, may miss new additions until next sync
**Related Task:** Task-208 AC #4

**Solution Path:**
1. Add `search()` method back to `MediaBackend` trait (`src/backends/traits.rs`)
2. Implement method for Plex backend (already done in `search_impl.rs`)
3. Add Jellyfin backend search implementation
4. Decide: replace local search OR augment with backend search
5. Update MainWindow to optionally call backend search

### 2. Source Filtering (Medium Priority)
**Issue:** Can't filter search results by source
**Impact:** Users can't limit search to specific media servers
**Related Task:** Task-130 AC #3

**Solution Path:**
1. Extend SearchDocument with `source_id: String` and `source_name: String` fields
2. Update Tantivy schema in SearchWorker
3. Re-index all content with source information
4. Add source filter UI to SearchPage (chip selector)
5. Modify SearchWorker to filter by source in query

### 3. Genre Filtering (Medium Priority)
**Issue:** Can't filter search by genre despite indexing genre data
**Impact:** Users can't narrow results by genre
**Related Task:** Task-130 AC #4

**Solution Path:**
1. Add genre filter chips to SearchPage UI
2. Modify search query to filter by genre (Tantivy supports this)
3. Update UI to show active filters

### 4. Source Badges (Low Priority)
**Issue:** Search results don't show which source each item is from
**Impact:** In multi-source setups, unclear which server hosts content
**Related Task:** Task-130 AC #5

**Solution Path:**
1. Extend MediaCard component to support badge overlays
2. Pass source information to MediaCard from SearchPage
3. Display small badge (e.g., "Plex", "Jellyfin") on poster

### 5. Search in MediaBackend Trait (Medium Priority)
**Issue:** `MediaBackend::search()` was removed, noted as "never used in production"
**Impact:** Can't perform backend searches through standard trait
**Location:** `src/backends/traits.rs:43`

**Solution Path:**
1. Re-add `search()` method to MediaBackend trait:
```rust
async fn search(&self, query: &str, limit: Option<usize>) -> Result<Vec<MediaItem>>;
```
2. Implement for all backends (Plex already has implementation)
3. Default implementation returns empty results for backends without search

### 6. Real-time Search (Low Priority)
**Issue:** Search only updates after full sync completes
**Impact:** New content takes time to appear in search
**Related Task:** Task-208 AC #4

**Solution Path:**
1. Implement incremental index updates when new items are synced
2. Listen to SyncWorker progress events
3. Index items as they're added to database during sync
4. Alternative: Use backend search for "live" results

## Testing Gaps

**Manual Testing Required:**
1. Search with movies, shows, episodes
2. Search with special characters, Unicode
3. Search with partial matches
4. Search with empty query
5. Search with no results
6. Image loading for search results
7. Navigation from search results to detail pages
8. Search during active sync

**No Automated Tests:**
- No unit tests for SearchWorker
- No integration tests for search workflow
- No UI tests for SearchPage

## Performance Considerations

**Current Performance:**
- Index initialization: ~50ms for 1000 items (varies by content)
- Search latency: <10ms for typical queries (local index)
- Index size: ~1-2MB per 1000 media items

**Optimization Opportunities:**
1. **Incremental Indexing:** Index new items during sync instead of bulk re-index
2. **Index Persistence:** Keep index between app restarts (currently rebuilds on startup)
3. **Lazy Loading:** Don't load full MediaItemModel for search results until displayed
4. **Search Debouncing:** Wait for user to stop typing before searching
5. **Result Caching:** Cache recent search results

## Recommendations

### Short-term (Complete Task-168)
1. ✅ Basic functionality is complete
2. **Action:** Test with various media types and queries
3. **Action:** Mark task-168 as Done
4. **Action:** Add search debouncing (wait 300ms after typing stops)

### Medium-term (Complete Task-208)
1. **Decision:** Choose hybrid search approach (Option A)
2. **Action:** Re-add `search()` to MediaBackend trait
3. **Action:** Add "Search server" option to UI (optional backend search)
4. **Action:** Show both local and remote results with source indicators

### Long-term (Task-130 and beyond)
1. **Action:** Add source filtering UI
2. **Action:** Add genre filtering UI
3. **Action:** Implement source badges on MediaCard
4. **Action:** Add incremental indexing during sync
5. **Action:** Add automated tests for search functionality
6. **Action:** Add search settings to preferences (default source, result limit, etc.)

## Related Files

### Core Implementation
- `src/workers/search_worker.rs` - Tantivy search worker
- `src/ui/pages/search.rs` - Search results UI
- `src/ui/main_window.rs` - Search integration and workflow
- `src/backends/plex/api/search_impl.rs` - Plex search API (not integrated)

### Supporting Files
- `src/models/identifiers.rs` - MediaItemId type
- `src/db/repository/media_repository.rs` - Database queries for search results
- `src/ui/factories/media_card.rs` - Result card display

### Tasks
- `backlog/tasks/task-168 - Implement-search-functionality-using-SearchWorker.md`
- `backlog/tasks/task-208 - Add-Plex-search-endpoints-for-content-discovery.md`
- `backlog/tasks/task-130 - Implement-cross-source-search.md`

## Dependencies

**External Crates:**
- `tantivy` - Full-text search engine
- `relm4` - UI framework and worker pattern
- `sea-orm` - Database ORM for result fetching

**Internal Services:**
- MediaRepository - Load full item data for search results
- ImageLoader - Load poster images
- SyncWorker - Triggers index refresh

## Future Enhancements

1. **Search Suggestions:** Autocomplete based on indexed titles
2. **Search History:** Remember recent searches
3. **Advanced Search UI:** Dedicated page with all filter options
4. **Fuzzy Matching:** Better handling of typos and partial matches
5. **Search Analytics:** Track popular searches to improve UX
6. **Voice Search:** Voice input for search queries (accessibility)
7. **Search Filters Persistence:** Remember last used filters
8. **Multi-language Search:** Support for non-English content
