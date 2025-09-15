# Playlist Context Implementation Checklist

## ✅ PHASES 1-5 COMPLETED (2025-09-14)

The core playlist context functionality has been successfully implemented:
- **Phase 1**: Data model and types created ✅
- **Phase 2**: Repository layer with episode navigation queries ✅
- **Phase 3**: Service layer with playlist business logic ✅
- **Phase 4**: Player integration with Previous/Next functionality ✅
- **Phase 5**: Show details integration with context generation ✅

**Result**: Previous/Next buttons now work for TV episodes! The player maintains context about the show being played and can navigate between episodes seamlessly.

## Overview
Implement internal playlist context to enable next/previous functionality in the player, focusing on TV show episode progression. This is NOT user-facing playlist management - it's purely for tracking playback sequence and enabling automatic episode progression.

## Current State Analysis

### ✅ What We Have
- [x] Episode data model with season/episode numbers
- [x] Database schema with proper episode relationships (parent_id, season_number, episode_number)
- [x] Unique constraint on (parent_id, season_number, episode_number)
- [x] Show details page that displays episodes in correct order
- [x] Player with Previous/Next buttons (currently non-functional)
- [x] Player receives MediaItemId when launched

### ❌ What's Missing
- [x] Player has no context about what show/season is playing ✅
- [x] No episode sequence/playlist tracking ✅
- [x] No way to determine next/previous episodes ✅
- [ ] No auto-play next episode functionality
- [x] Repository methods for episode navigation ✅

## Phase 1: Data Model & Types (Foundation) ✅

### 1.1 Create Playlist Context Types
- [x] Create `src/models/playlist_context.rs` ✅
  ```rust
  pub enum PlaylistContext {
      SingleItem,
      TvShow {
          show_id: ShowId,
          show_title: String,
          current_index: usize,
          episodes: Vec<EpisodeInfo>,
          auto_play_next: bool,
      },
      // Future: Album, Playlist, Queue
  }

  pub struct EpisodeInfo {
      pub id: MediaItemId,
      pub title: String,
      pub season_number: u32,
      pub episode_number: u32,
      pub duration_ms: Option<i64>,
      pub watched: bool,
      pub playback_position_ms: Option<i64>,
  }
  ```

### 1.2 Update Player Input/Output
- [x] Add to `PlayerInput` enum in `src/platforms/relm4/components/pages/player.rs`: ✅
  ```rust
  LoadMediaWithContext {
      media_id: MediaItemId,
      context: PlaylistContext,
  },
  ```

### 1.3 Update Navigation Messages
- [x] Modify `MainWindowInput` in `src/platforms/relm4/components/main_window.rs`: ✅
  ```rust
  NavigateToPlayerWithContext {
      media_id: MediaItemId,
      context: PlaylistContext,
  },
  ```

## Phase 2: Repository Layer (Database Queries) ✅

### 2.1 Extend MediaRepository
- [x] Add to `src/db/repository/media_repository.rs`: ✅
  ```rust
  /// Get all episodes for a show in playback order
  pub async fn find_episode_playlist(
      &self,
      show_id: &str,
  ) -> Result<Vec<MediaItemModel>>

  /// Find the next episode after the given one
  pub async fn find_next_episode(
      &self,
      show_id: &str,
      season: i32,
      episode: i32,
  ) -> Result<Option<MediaItemModel>>

  /// Find the previous episode before the given one
  pub async fn find_previous_episode(
      &self,
      show_id: &str,
      season: i32,
      episode: i32,
  ) -> Result<Option<MediaItemModel>>

  /// Find next unwatched episode in show
  pub async fn find_next_unwatched_episode(
      &self,
      show_id: &str,
      after_season: i32,
      after_episode: i32,
  ) -> Result<Option<MediaItemModel>>
  ```

### 2.2 Implement SQL Queries
- [x] Query for episode playlist (sorted by season, episode): ✅
  ```sql
  SELECT * FROM media_items
  WHERE parent_id = ? AND media_type = 'episode'
  ORDER BY season_number, episode_number
  ```

- [x] Query for next episode: ✅
  ```sql
  SELECT * FROM media_items
  WHERE parent_id = ? AND media_type = 'episode'
    AND ((season_number = ? AND episode_number > ?)
         OR season_number > ?)
  ORDER BY season_number, episode_number
  LIMIT 1
  ```

- [x] Query for previous episode: ✅
  ```sql
  SELECT * FROM media_items
  WHERE parent_id = ? AND media_type = 'episode'
    AND ((season_number = ? AND episode_number < ?)
         OR season_number < ?)
  ORDER BY season_number DESC, episode_number DESC
  LIMIT 1
  ```

## Phase 3: Service Layer (Business Logic) ✅

### 3.1 Create Playlist Service
- [x] Create `src/services/core/playlist.rs`: ✅
  ```rust
  pub struct PlaylistService;

  impl PlaylistService {
      /// Build playlist context for a TV show episode
      pub async fn build_show_context(
          db: &DatabaseConnection,
          episode_id: &MediaItemId,
      ) -> Result<PlaylistContext>

      /// Get next item in playlist
      pub async fn get_next_item(
          db: &DatabaseConnection,
          current_id: &MediaItemId,
          context: &PlaylistContext,
      ) -> Result<Option<MediaItemId>>

      /// Get previous item in playlist
      pub async fn get_previous_item(
          db: &DatabaseConnection,
          current_id: &MediaItemId,
          context: &PlaylistContext,
      ) -> Result<Option<MediaItemId>>
  }
  ```

### 3.2 Implement Context Building
- [x] Load current episode details ✅
- [x] Find parent show ✅
- [x] Load all episodes for the show ✅
- [x] Sort by season/episode number ✅
- [x] Find current episode index in list ✅
- [x] Build PlaylistContext::TvShow ✅

## Phase 4: Player Integration ✅

### 4.1 Update Player State
- [x] Add to `PlayerPage` struct: ✅
  ```rust
  pub struct PlayerPage {
      // existing fields...
      playlist_context: Option<PlaylistContext>,
      current_playlist_index: Option<usize>,
  }
  ```

### 4.2 Implement Navigation Logic
- [x] Update `PlayerInput::Previous` handler: ✅
  ```rust
  PlayerInput::Previous => {
      if let Some(ref context) = self.playlist_context {
          match PlaylistService::get_previous_item(&self.db, &current_id, context).await {
              Ok(Some(prev_id)) => {
                  sender.input(PlayerInput::LoadMedia(prev_id));
              }
              _ => debug!("No previous episode available"),
          }
      }
  }
  ```

- [x] Update `PlayerInput::Next` handler: ✅
  ```rust
  PlayerInput::Next => {
      if let Some(ref context) = self.playlist_context {
          match PlaylistService::get_next_item(&self.db, &current_id, context).await {
              Ok(Some(next_id)) => {
                  sender.input(PlayerInput::LoadMedia(next_id));
              }
              _ => debug!("No next episode available"),
          }
      }
  }
  ```

### 4.3 Handle Context During Load
- [x] Update `PlayerInput::LoadMedia` to preserve context ✅
- [x] Update `PlayerInput::LoadMediaWithContext` to store context ✅
- [x] Update playlist index when switching episodes ✅

## Phase 5: Show Details Integration ✅

### 5.1 Generate Context When Playing Episodes
- [x] Update `ShowDetailsOutput::PlayEpisode` in show_details.rs ✅
- [x] Build playlist context before navigation ✅
- [x] Pass context to MainWindow ✅

### 5.2 Update Navigation Flow
- [x] Modify episode click handler: ✅
  ```rust
  EpisodeOutput::Play(episode_id) => {
      // Build context for all episodes in show
      let context = PlaylistService::build_show_context(&db, &episode_id).await?;
      sender.output(ShowDetailsOutput::PlayEpisodeWithContext {
          episode_id,
          context,
      });
  }
  ```

## Phase 6: Auto-Play Implementation

### 6.1 Track Playback Completion
- [ ] Add to PlayerPage:
  ```rust
  fn check_playback_completion(&self) -> bool {
      // Check if position >= duration - 10 seconds
      // Or if explicitly reached end
  }
  ```

### 6.2 Implement Auto-Play Logic
- [ ] Add timer to check completion near end of episode
- [ ] When episode ends (>90% complete):
  ```rust
  if self.playlist_context.is_some() && auto_play_enabled {
      // Show "Next episode in 10 seconds" overlay
      // Start countdown timer
      // Auto-load next episode unless cancelled
  }
  ```

### 6.3 Create Auto-Play UI
- [ ] Add countdown overlay widget
- [ ] Show next episode title/thumbnail
- [ ] Add cancel button
- [ ] Add "Play Now" button

## Phase 7: Continue Watching Integration

### 7.1 Smart Resume Logic
- [ ] When resuming a show, check if episode is >90% watched
- [ ] If so, offer to play next unwatched episode instead
- [ ] Update "Continue Watching" to show next episode when current is finished

### 7.2 Update Continue Watching Query
- [ ] Modify to return next unwatched episode if current is watched
- [ ] Sort by last watched timestamp for shows

## Phase 8: Edge Cases & Polish

### 8.1 Handle Edge Cases
- [ ] Last episode in season → go to next season
- [ ] Last episode in show → stop or loop to S01E01
- [ ] Missing episodes (gaps in numbering)
- [ ] Single episode shows (documentaries)
- [ ] Episodes without season/episode numbers

### 8.2 Performance Optimizations
- [ ] Cache episode list in player to avoid re-querying
- [ ] Preload next episode metadata during playback
- [ ] Lazy load full episode list for large shows

### 8.3 Settings & Preferences
- [ ] Add auto-play toggle to preferences
- [ ] Add "skip intro" detection (future)
- [ ] Add "skip credits" detection (future)

## Testing Checklist

### Unit Tests
- [ ] Test episode ordering logic
- [ ] Test next/previous episode queries
- [ ] Test edge cases (first/last episodes)
- [ ] Test context building from episode ID

### Integration Tests
- [ ] Test full navigation flow from show → episode → next episode
- [ ] Test auto-play countdown and cancellation
- [ ] Test continue watching with completed episodes
- [ ] Test with shows missing episode numbers

### Manual Testing
- [ ] Play episode and use next/previous buttons
- [ ] Let episode finish and verify auto-play
- [ ] Test with multi-season shows
- [ ] Test with single episode shows
- [ ] Verify continue watching updates correctly

## Success Metrics
- Previous/Next buttons work for TV episodes
- Auto-play next episode works with countdown
- Continue watching suggests next unwatched episode
- No UI changes required (all internal)
- Performance: <100ms to determine next episode
- Works offline with cached metadata

## Future Enhancements (Out of Scope)
- Music album/playlist support
- Custom user playlists
- Shuffle/repeat modes
- Smart playlists (recently added, favorites)
- Cross-show playlists (all unwatched episodes)
- Intro/credit skip markers