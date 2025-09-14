# Library Synchronization & UI Reactivity Task List

## Overview
This document outlines tasks to improve library synchronization and UI reactivity in the Reel media player. The main goal is to create a fully reactive, message-driven sync system with proper UI feedback using the MessageBroker pattern.

**Note**: EventBus is deprecated and will be removed. All event handling should use the Relm4-native MessageBroker system.

## Priority Levels
- 🔴 **Critical**: Blocking issues that cause data inconsistency or poor UX
- 🟡 **High**: Important improvements for reliability and performance
- 🟢 **Medium**: Enhancements for better user experience
- 🔵 **Low**: Nice-to-have features

---

## 🔴 Critical: Message System Integration

### Task 0: Activate MessageBroker System
**Problem**: MessageBroker exists but is completely unused
**Location**: `src/platforms/relm4/components/shared/broker.rs`
- [x] Ensure BROKER static is properly initialized
- [x] Add helper methods for common message patterns
- [x] Create message conversion utilities for service layer
- [x] Add logging/debugging for message flow
- [x] Document MessageBroker usage patterns

### Task 1: Integrate SyncService with MessageBroker
**Problem**: SyncService bypasses the message system entirely, causing UI disconnection
**Location**: `src/services/core/sync.rs`
- [x] Remove `new_without_events()` calls from SyncRepositoryImpl (already done)
- [x] Add MessageBroker access to SyncService via BROKER static
- [x] Broadcast SourceMessage::SyncStarted at sync beginning
- [x] Broadcast DataMessage with progress during batch processing
- [x] Broadcast SourceMessage::SyncCompleted/SyncError at sync end
- [x] Update all SyncService callers to use MessageBroker (SyncService already uses BROKER)

### Task 2: Fix Progress Reporting
**Problem**: Sync progress always returns 0% due to missing total_items field
**Location**: `src/db/repository/sync_repository.rs`, `src/db/entities/sync_status.rs`
- [x] Add `total_items` column to sync_status table via migration
- [x] Update SyncStatus entity with total_items field
- [x] Implement item counting before sync starts (placeholder for now)
- [x] Calculate accurate progress percentage
- [x] Update progress during batch processing

### Task 3: Implement Repository Message Integration
**Problem**: Repository operations don't broadcast messages
**Location**: `src/db/repository/*.rs`
- [x] Remove EventBus from repository constructors
- [ ] Add MessageBroker access to repositories (when needed)
- [ ] Broadcast DataMessage::MediaUpdated from media_repository (TODO when needed)
- [ ] Broadcast DataMessage::LibraryUpdated from library_repository (TODO when needed)
- [ ] Broadcast SourceMessage for source updates (TODO when needed)
- [x] Update all repository instantiations to remove EventBus

---

## 🔴 Critical: UI Reactivity Fixes

### Task 4: Subscribe UI Components to Sync Messages
**Problem**: UI components don't listen to sync messages
**Location**: `src/platforms/relm4/components/pages/*.rs`
- [x] Subscribe components to BROKER in AsyncComponent::init() (SourcesPage done)
- [ ] Convert BrokerMessage to component Input messages
- [ ] Handle DataMessage::Loading/LoadComplete for UI state
- [ ] Handle SourceMessage::SyncStarted/SyncCompleted/SyncError
- [ ] Handle DataMessage::MediaUpdated/LibraryUpdated for refresh
- [ ] Unsubscribe from BROKER in component drop
- [ ] Use component ID for targeted messaging

### Task 5: Fix Race Conditions in Sources Page
**Problem**: Manual sync tracking with HashSet causes races
**Location**: `src/platforms/relm4/components/pages/sources.rs`
- [x] Replace `syncing_sources` HashSet with message-driven state
- [x] Use SourceMessage::SyncStarted to set loading state
- [x] Use SourceMessage::SyncCompleted to clear loading state
- [x] Remove manual LoadData calls after sync (kept only in SyncCompleted for data refresh)

### Task 6: Coordinate Background and User-Triggered Sync
**Problem**: No coordination between SyncWorker and manual sync
**Location**: `src/platforms/relm4/components/workers/sync_worker.rs`
- [ ] Create central SyncCoordinator service
- [ ] Implement sync queue with deduplication
- [ ] Add mutex to prevent concurrent syncs of same source
- [ ] Cancel background sync when user triggers manual sync
- [ ] Reset interval timer after manual sync

### Task 7: Bridge Service Layer to MessageBroker
**Problem**: Service layer can't directly use Relm4 MessageBroker
**Location**: `src/services/core/`, `src/platforms/relm4/components/workers/`
- [ ] Create ServiceBridgeWorker to relay service events to MessageBroker
- [ ] Implement async channel from services to worker
- [ ] Convert service events to BrokerMessage types
- [ ] Handle thread boundary between Tokio and GTK main loop
- [ ] Add batching for high-frequency service events

---

## 🟡 High Priority: Complete EventBus Removal

### Task 8: Fully Remove EventBus and Old Event System from Codebase
**Problem**: EventBus and old event types are still used everywhere, preventing MessageBroker adoption
**Location**: Throughout codebase, especially repositories and services
- [x] Remove EventBus from all repository implementations
- [x] Remove event_bus field from BaseRepository struct
- [x] Update all repository constructors to remove event_bus parameter
- [ ] Remove EventBus from all ViewModels (not needed for Relm4)
- [ ] Delete entire src/events/ directory including:
  - [ ] src/events/event_bus.rs
  - [ ] src/events/types.rs (DatabaseEvent, EventType, EventPayload - old system)
  - [ ] src/events/mod.rs
- [ ] Remove service_bridge.rs (it bridges to old event system)
- [ ] Update all backend implementations to use MessageBroker directly
- [ ] Remove all EventBus::global() calls
- [ ] Update database connection to use MessageBroker
- [x] Remove new_without_events() methods - all repos now use single constructor
- [ ] Repositories should directly call BROKER methods when needed
- [ ] Ensure all events flow through MessageBroker BrokerMessage types only

---

## 🟡 High Priority: Data Consistency

### Task 9: Implement Database Transactions for Sync
**Problem**: Partial sync failures leave inconsistent state
**Location**: `src/services/core/sync.rs`
- [ ] Wrap sync_library in database transaction
- [ ] Implement rollback on sync failure
- [ ] Add transaction support to batch operations
- [ ] Ensure atomic updates for related entities
- [ ] Add transaction timeout handling

### Task 10: Implement Cache Invalidation
**Problem**: UI shows stale data after sync
**Location**: `src/services/data.rs`
- [ ] Add cache invalidation method to DataService
- [ ] Subscribe to SourceMessage::SyncCompleted via MessageBroker
- [ ] Invalidate affected cache entries
- [ ] Implement selective cache clearing by source/library
- [ ] Add cache versioning for consistency

### Task 11: Add Incremental Sync Support
**Problem**: Full sync is inefficient and disruptive
**Location**: `src/services/core/sync.rs`, `src/backends/traits.rs`
- [ ] Add `last_modified` tracking to media items
- [ ] Implement `get_changes_since()` in MediaBackend trait
- [ ] Add incremental sync logic to SyncService
- [ ] Store sync checkpoint in database
- [ ] Fall back to full sync on checkpoint failure

---

## 🟡 High Priority: Error Handling & Recovery

### Task 12: Implement Toast Notifications
**Problem**: No user feedback for sync errors
**Location**: `src/platforms/relm4/components/main_window.rs`
- [ ] Create ToastOverlay component
- [ ] Add toast message queue
- [ ] Implement toast types (error, warning, info, success)
- [ ] Add auto-dismiss with configurable timeout
- [ ] Support action buttons (retry, dismiss, details)

### Task 13: Add Retry Logic with Exponential Backoff
**Problem**: No automatic retry for failed syncs
**Location**: `src/services/core/sync.rs`
- [ ] Implement exponential backoff algorithm
- [ ] Add max retry count configuration
- [ ] Store retry count in sync_status
- [ ] Reset retry count on success
- [ ] Broadcast retry status via MessageBroker

### Task 14: Improve Error Messages
**Problem**: Generic error messages without context
**Location**: `src/backends/*.rs`, `src/services/core/sync.rs`
- [ ] Create detailed error types for sync failures
- [ ] Add context to error messages (source, library, operation)
- [ ] Implement user-friendly error translations
- [ ] Include suggested actions in error messages
- [ ] Log detailed errors for debugging

---

## 🟢 Medium Priority: Progress Visualization

### Task 15: Add Global Sync Indicator
**Problem**: No visible sync status in UI
**Location**: `src/platforms/relm4/components/sidebar.rs` or header
- [ ] Create SyncStatusWidget component
- [ ] Show sync icon with animation during sync
- [ ] Display source being synced
- [ ] Show progress percentage
- [ ] Add click handler to show sync details

### Task 16: Implement Progress Bars
**Problem**: No visual progress during long syncs
**Location**: `src/platforms/relm4/components/pages/sources.rs`
- [ ] Add ProgressBar widget to source items
- [ ] Update progress from DataMessage broadcasts
- [ ] Show items synced / total items
- [ ] Add time remaining estimation
- [ ] Support indeterminate progress for discovery phase

### Task 17: Create Sync History View
**Problem**: No visibility into sync history
**Location**: New component in `src/platforms/relm4/components/dialogs/`
- [ ] Create SyncHistoryDialog component
- [ ] Display last 10 syncs per source
- [ ] Show sync duration and item count
- [ ] Highlight failed syncs
- [ ] Add filter by source/status

---

## 🟢 Medium Priority: Performance Optimization

### Task 18: Implement Batch UI Updates
**Problem**: Frequent sync messages cause UI thrashing
**Location**: `src/platforms/relm4/components/pages/*.rs`
- [ ] Add update debouncing (100ms minimum)
- [ ] Batch multiple messages into single UI update
- [ ] Use Tracker pattern to minimize re-renders
- [ ] Implement virtual scrolling for large lists
- [ ] Add lazy loading for media cards

### Task 19: Optimize Sync Queries
**Problem**: Inefficient database queries during sync
**Location**: `src/services/core/sync.rs`
- [ ] Add database indexes for sync queries
- [ ] Implement bulk insert/update operations
- [ ] Use prepared statements for repeated queries
- [ ] Add query result caching
- [ ] Profile and optimize slow queries

### Task 20: Add Sync Scheduling
**Problem**: All sources sync simultaneously
**Location**: `src/platforms/relm4/components/workers/sync_worker.rs`
- [ ] Implement sync queue with priority
- [ ] Stagger sync start times
- [ ] Add bandwidth throttling option
- [ ] Respect server rate limits
- [ ] Add quiet hours configuration

---

## 🔵 Low Priority: Enhanced Features

### Task 21: Add Sync Cancellation UI
**Problem**: Can't cancel running sync from UI
**Location**: `src/platforms/relm4/components/pages/sources.rs`
- [ ] Add cancel button during sync
- [ ] Implement sync cancellation token
- [ ] Clean up partial sync on cancel
- [ ] Show cancellation in sync history
- [ ] Add confirmation dialog for cancel

### Task 22: Implement Selective Library Sync
**Problem**: Can't choose which libraries to sync
**Location**: `src/platforms/relm4/components/pages/sources.rs`
- [ ] Add library selection checkboxes
- [ ] Store sync preferences per source
- [ ] Implement selective sync in SyncService
- [ ] Add "sync all" toggle
- [ ] Remember user preferences

### Task 23: Add Sync Conflict Resolution
**Problem**: No handling for sync conflicts
**Location**: `src/services/core/sync.rs`
- [ ] Detect conflicts (local vs remote changes)
- [ ] Implement conflict resolution strategies
- [ ] Add UI for manual conflict resolution
- [ ] Store conflict history
- [ ] Add auto-resolution preferences

### Task 24: Implement Offline Content Sync
**Problem**: No offline content management
**Location**: New service in `src/services/`
- [ ] Create OfflineContentService
- [ ] Add download queue management
- [ ] Implement storage quota handling
- [ ] Add automatic cleanup policies
- [ ] Create offline content UI

---

## Testing Requirements

### Unit Tests
- [ ] Test message broadcasting from SyncService
- [ ] Test progress calculation accuracy
- [ ] Test transaction rollback scenarios
- [ ] Test retry logic with backoff
- [ ] Test cache invalidation

### Integration Tests
- [ ] Test full sync flow with MessageBroker
- [ ] Test concurrent sync handling
- [ ] Test error recovery
- [ ] Test UI update batching
- [ ] Test sync cancellation

### UI Tests
- [ ] Test loading states during sync
- [ ] Test error toast display
- [ ] Test progress bar updates
- [ ] Test sync history display
- [ ] Test sync cancellation flow

---

## Implementation Order

### Phase 1: Foundation (Week 1)
1. Task 0: Activate MessageBroker system
2. Task 1: MessageBroker integration in SyncService
3. Task 2: Fix progress reporting
4. Task 3: Repository message broadcasting
5. Task 4: UI message subscriptions
6. Task 7: Bridge service layer to MessageBroker
7. Task 8: Fully remove EventBus from codebase

### Phase 2: Stability (Week 2)
8. Task 5: Fix race conditions
9. Task 6: Sync coordination
10. Task 9: Database transactions
11. Task 12: Toast notifications

### Phase 3: UX (Week 3)
12. Task 15: Global sync indicator
13. Task 16: Progress bars
14. Task 13: Retry logic
15. Task 14: Better error messages

### Phase 4: Optimization (Week 4)
16. Task 10: Cache invalidation
17. Task 11: Incremental sync
18. Task 18: Batch UI updates
19. Task 19: Query optimization

### Phase 5: Polish (Week 5+)
17. Remaining medium priority tasks
18. Low priority enhancements
19. Comprehensive testing
20. Documentation updates

---

## Success Metrics

- **Sync Reliability**: < 1% sync failure rate
- **UI Responsiveness**: < 100ms UI update latency
- **Progress Accuracy**: ±5% progress reporting accuracy
- **Error Recovery**: 95% automatic recovery rate
- **User Satisfaction**: Clear sync status visibility

---

## Notes

- All database schema changes require migrations
- MessageBroker changes affect multiple components
- **EventBus AND src/events/ directory MUST be completely removed from the codebase**
- **Do NOT use DatabaseEvent, EventType, or EventPayload - these are part of the old system**
- Use only MessageBroker's BrokerMessage types (DataMessage, SourceMessage, etc.)
- GTK frontend is disabled and does not need to be fixed
- Relm4 components should use MessageBroker exclusively
- Repositories should directly call BROKER helper methods
- Consider backward compatibility for existing data
- Performance testing required for large libraries (10k+ items)
- Coordinate with backend team for API optimizations