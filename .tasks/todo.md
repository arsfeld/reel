# TODO

## Completed Today
- [x] Fixed media card watched status - loads from database
- [x] Connected watched status toggle to database
- [x] Implemented preferences persistence
- [x] Added error logging for sources page

## High Priority
- [x] Fix PlaylistContext implementation for episode navigation
  - Added NavigateToMovie and NavigateToShow inputs to MainWindow
  - Properly route shows to ShowDetailsPage which creates PlaylistContext
  - Episodes now play with full navigation context
- [x] Fix show details page episode list and season navigation
  - Made episode cards clickable with GestureClick controller
  - Added hover cursor feedback for better UX
  - Connected episodes to play with playlist context
- [x] Auto-jump to latest unwatched episode in show details
  - Show details page automatically selects the season containing the next unwatched episode
  - Episode list scrolls to highlight and focus the next episode to watch
  - Queries database for playback progress to find unwatched episodes
- [x] Wire up ImageWorker to MediaCard
  - Created shared ImageLoader worker in LibraryPage
  - Tracks image requests by card index
  - Sends loaded textures to specific cards via factory.send()
  - MediaCard now receives and displays actual poster images
- [ ] Implement proper toast notifications (needs view restructuring)

## Medium Priority
- [ ] Implement advanced search functionality (basic search works)

## Relm4 Migration - Remaining Tasks

### Critical (Blocking User Experience)
- [ ] Preferences not persisting - Preferences page exists but doesn't save to config/database
- [ ] Image loading disconnected - MediaCards don't use ImageWorker for thumbnails
- [ ] Source page creation placeholder - Line 676-677 in main_window.rs needs implementation
- [ ] Library item counts - Sidebar shows placeholder values (lines 95-96 in sidebar.rs)

### Important (Feature Gaps)
- [ ] Cache clearing non-functional - Button exists but doesn't work (line 228 in preferences.rs)
- [ ] Error toasts missing - Sources page errors not shown to user (line 580 in sources.rs)
- [ ] AuthProvider creation incomplete - Line 699 in auth_dialog.rs needs proper implementation
- [ ] Watched status toggle - Movie/show details pages don't update database (lines 356, 368)

### Nice to Have (Polish)
- [ ] View mode switch - Library page toggle doesn't update FlowBox layout (line 272)
- [ ] Search placeholder generic - Could be more contextual (line 177 in library.rs)
- [ ] Genres not populated - Search worker doesn't extract genres (lines 48, 55 in search_worker.rs)
- [ ] Continue watching loading - Homepage section needs proper implementation (line 266)
- [ ] Episode navigation - Player previous/next not implemented

### Documentation & Testing
- [ ] Document tracker usage patterns
- [ ] Document factory best practices
- [ ] Document worker communication patterns
- [ ] Document command patterns
- [ ] Create component templates
- [ ] Mark GTK implementation as deprecated/reference-only

### Code Quality
- [ ] Replace unwrap() with proper error handling
- [ ] Use tracing instead of eprintln!
- [ ] Add Result types to navigation handlers
- [ ] Implement graceful error recovery

### Performance Optimizations
- [ ] Virtual scrolling optimization
- [ ] Image caching strategy improvements
- [ ] Memory profiling
- [ ] Measure startup time (<500ms target)
- [ ] Measure page transitions (<100ms target)

### Future Enhancements
- [ ] Component library package
- [ ] Design system with CSS
- [ ] Plugin architecture
- [ ] Theme system improvements
- [ ] Accessibility features

## Notes
- Project compiles successfully with Relm4 feature
- Database integration working for playback progress
- Preferences now save/load from config file
- Continue watching section functional with real data