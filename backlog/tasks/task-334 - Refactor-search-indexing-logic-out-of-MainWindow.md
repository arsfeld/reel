---
id: task-334
title: Refactor search indexing logic out of MainWindow
status: Done
assignee:
  - '@claude'
created_date: '2025-10-02 19:16'
updated_date: '2025-10-02 19:32'
labels:
  - refactoring
  - architecture
  - search
dependencies: []
priority: medium
---

## Description

MainWindow currently contains ~100 lines of database querying and search indexing logic that violates separation of concerns. Move indexing to happen automatically at the repository level via MessageBroker, so SearchWorker incrementally updates the index whenever items are saved.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 MediaRepository broadcasts MediaUpdated/MediaBatchSaved broker messages after insert/update/bulk_insert
- [x] #2 SearchWorker subscribes to broker and incrementally indexes media items
- [x] #3 SearchWorker loads initial index from database on startup
- [x] #4 Remove duplicate indexing code from MainWindow (init_search_index, refresh_search_index)
- [x] #5 Search index updates automatically during sync without explicit refresh
- [x] #6 Verify search functionality works end-to-end after refactor
<!-- AC:END -->


## Implementation Plan

1. Add MediaBatchSaved message to BrokerMessage::Data enum
2. Update MediaRepository to broadcast MediaUpdated/MediaBatchSaved after insert/update/bulk_insert
3. Make SearchWorker subscribe to broker messages and handle incremental indexing
4. Add initial index load on SearchWorker startup from database
5. Remove duplicate indexing code from MainWindow (init_search_index, refresh_search_index)
6. Test incremental indexing during sync and search functionality end-to-end


## Implementation Notes

## Current State (Violations)

**MainWindow has 2 duplicate code blocks (~100 lines each):**

1. **init_search_index** (lines ~842-891)
   - Directly calls `MediaRepositoryImpl::new(db)`
   - Queries database with `repo.find_all().await`
   - Parses JSON genres: `serde_json::from_value::<Vec<String>>(json)`
   - Deduplicates with HashSet
   - Sends to SearchWorker

2. **refresh_search_index** (lines ~913-970)
   - Exact same logic as above
   - Adds ClearIndex call before re-indexing

**Problems:**
- UI layer doing database queries
- UI layer parsing JSON data
- Duplicate code (DRY violation)
- MainWindow knows about MediaRepository internals
- No testability - can't unit test indexing logic

Implemented repository-level incremental search indexing:

- Added MediaBatchSaved message to DataMessage enum for batch save notifications
- MediaRepository now broadcasts MediaUpdated after insert/update and MediaBatchSaved after bulk_insert
- SearchWorker subscribes to MessageBroker and handles incremental indexing
- SearchWorker loads initial index from database on startup
- Removed ~200 lines of duplicate indexing code from MainWindow (init_search_index, refresh_search_index)
- Search index now updates automatically as items are synced, no manual refresh needed

Next: Test search functionality end-to-end to verify incremental indexing works correctly

**Issue discovered during verification:**
Search results show duplicates because the refactored code removed the HashSet deduplication that was in the old MainWindow implementation. Created task-337 (high priority) to fix this by adding deduplication to SearchWorker.index_documents() method.


## Proposed Solution

### Option A: Add to SearchWorker (Recommended)
```rust
// In SearchWorker
pub enum SearchWorkerInput {
    LoadAndIndex { db: DatabaseConnection },  // New
    RefreshIndex { db: DatabaseConnection },  // New
    // ... existing variants
}

impl Worker for SearchWorker {
    fn update(&mut self, msg: Self::Input) {
        match msg {
            SearchWorkerInput::LoadAndIndex { db } => {
                // All the indexing logic here
                let repo = MediaRepositoryImpl::new(db);
                let items = repo.find_all().await?;
                // dedupe, parse, index...
            }
        }
    }
}

// In MainWindow - becomes simple:
"init_search_index" => {
    self.search_worker.emit(SearchWorkerInput::LoadAndIndex { 
        db: self.db.clone() 
    });
}
```

### Option B: Command Pattern
Create `src/services/commands/search_commands.rs`:
```rust
pub struct IndexMediaCommand {
    pub db: DatabaseConnection,
    pub search_worker: WorkerSender<SearchWorker>,
}

impl Command for IndexMediaCommand {
    async fn execute(&self) -> Result<()> {
        // All indexing logic here
    }
}
```

### Recommendation
**Use Option A** - SearchWorker should own its indexing logic since:
- It already manages the Tantivy index
- Keeps all search-related code together
- Worker can be async without blocking UI
- Follows the existing worker pattern in the project

## Files to Modify
- `src/workers/search_worker.rs` - Add LoadAndIndex/RefreshIndex inputs
- `src/ui/main_window.rs` - Remove ~150 lines of indexing code, replace with simple emits
- Tests if needed

## Success Criteria
- MainWindow's "init_search_index" handler: **≤ 5 lines**
- MainWindow's "refresh_search_index" handler: **≤ 5 lines**
- No database imports in main_window.rs
- No JSON parsing in main_window.rs
- Search still works on startup and after sync
