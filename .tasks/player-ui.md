# Player UI Full Reactive Migration Plan

## Overview

This plan outlines the complete migration of the PlayerPage from a hybrid imperative/reactive system to a **fully reactive UI**. The migration builds on the existing 35% reactive architecture and targets **100% reactive** player controls by eliminating all polling timers, manual widget updates, and dead code.

**Current Status: ~85% Reactive (Phase 3 of 7 complete + significant Phase 4-6 groundwork)**
**Target: 100% Reactive Player UI**

## Architecture Analysis

### Current Reactive Components ✅
- **Basic ViewModel Integration**: `PlayerViewModel` initialized and connected to UI
- **Play/Pause State**: Reactive button icon updates via `playback_state` property (`bind_icon_to_property`)
- **Volume Controls**: Reactive slider value and mute visibility via `volume`/`is_muted` properties (`bind_value_to_property`, `bind_visibility_to_property`)  
- **Progress Bar**: ✅ Reactive position updates via custom dual-property binding (`position`/`duration`)
- **Time Displays**: ✅ Reactive formatted time via `bind_text_to_property` (`position`/`duration` → `format_duration()`)
- **Loading/Error States**: ✅ Reactive overlay visibility via `is_loading`/`error` properties (`bind_visibility_to_property`)
- **Track Management**: ✅ Reactive audio/subtitle button enable/disable via track availability (`bind_visibility_to_property`)
- **Track Selection**: ✅ Reactive dropdown menus populate from track properties (`bind_dropdown_to_property`)

### Non-Reactive Components ❌
- **Control Visibility**: Manual `glib::timeout_add_local` timers with fade animations (lines 250-380 in player.rs)
- **Skip Buttons**: Marker-based visibility with 500ms polling timers (`glib::timeout_add_local` lines 1423, 1593)

### Dead Code Items 🧹
- `PlaybackInfo` struct - Unused combined state representation (line 21 in player_view_model.rs)
- `AutoPlayState::Counting` and `AutoPlayState::Disabled` variants - Unimplemented features (lines 63-66)
- ViewModel property `show_controls` exists but not bound to UI (ready for Phase 4)

### Analysis Summary
**Current Implementation Status**: ~85% reactive (Phase 3 complete + most reactive infrastructure ready)
- ✅ **12 reactive bindings active**: play/pause icon, volume slider+visibility, progress bar, time displays, loading/error states, track button enable/disable, track dropdown menus with `bind_dropdown_to_property`
- ✅ **PlayerViewModel fully equipped**: All track management properties implemented, `show_controls` property exists
- ❌ **2 remaining non-reactive systems**: control visibility timers (lines 250-380), skip button polling (lines 1423, 1593)
- 🧹 **Dead code present**: PlaybackInfo struct, unused AutoPlayState variants 
- 📋 **Nearly complete**: Only control visibility binding and skip button reactive logic missing

## Migration Strategy

### Phase 2.5: Complete Basic Reactive Bindings ✅ COMPLETE
**Goal**: Complete the basic reactive bindings for progress and time displays
**Estimated Effort**: 2-3 hours  
**Files**: `src/platforms/gtk/ui/pages/player.rs`
**Status**: ✅ **COMPLETED** - All basic reactive bindings implemented

#### 2.5.1 Add Missing Progress Bar Reactive Binding
```rust
// Progress bar value binding (position/duration)
let progress_binding = bind_value_to_property(
    &controls.progress_scale,
    view_model.position().clone(),
    view_model.duration().clone(),
    |position, duration| {
        if duration.as_secs() > 0 {
            position.as_secs_f64() / duration.as_secs_f64()
        } else {
            0.0
        }
    },
);
```

#### 2.5.2 Add Time Display Reactive Bindings  
```rust
// Current time display
let current_time_binding = bind_text_to_property(
    &controls.current_time_label,
    view_model.position().clone(),
    |position| format_duration(*position),
);

// Total time display
let total_time_binding = bind_text_to_property(
    &controls.total_time_label,
    view_model.duration().clone(),
    |duration| format_duration(*duration),
);
```

#### 2.5.3 Add Loading/Error State Bindings
```rust
// Loading overlay visibility
let loading_binding = bind_visibility_to_property(
    &loading_overlay,
    view_model.is_loading().clone(),
    |is_loading| *is_loading,
);

// Error overlay visibility
let error_binding = bind_visibility_to_property(
    &error_overlay,
    view_model.error().clone(),
    |error| error.is_some(),
);
```

**Success Criteria**:
- ✅ **COMPLETE**: Progress bar updates reactively during playback
- ✅ **COMPLETE**: Time displays update reactively without manual updates  
- ✅ **COMPLETE**: Loading/error states show/hide reactively
- ✅ **COMPLETE**: Achieves ~40% reactive baseline before Phase 3

**Implementation Notes**:
- Used `bind_text_to_property` for time displays with `format_duration()` transform
- Created custom dual-property binding for progress bar (subscribes to `position`, uses `duration` for calculation)
- Implemented standard `bind_visibility_to_property` for loading/error overlays
- All bindings stored in `_binding_handles` vector for proper lifecycle management

### Phase 3: Track Management Reactive System ✅ COMPLETE
**Goal**: Replace direct player API calls with reactive track management
**Estimated Effort**: 8-12 hours  
**Files**: `src/core/viewmodels/player_view_model.rs`, `src/platforms/gtk/ui/pages/player.rs`
**Status**: ✅ **COMPLETED** - Track management fully reactive with button enable/disable

#### 3.1 Extend PlayerViewModel with Track Properties
```rust
pub struct PlayerViewModel {
    // Existing properties...
    
    // Track management properties
    audio_tracks: Property<Vec<AudioTrack>>,
    subtitle_tracks: Property<Vec<SubtitleTrack>>,
    selected_audio_track: Property<Option<usize>>,
    selected_subtitle_track: Property<Option<usize>>,
    quality_options: Property<Vec<QualityOption>>,
    selected_quality: Property<Option<usize>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioTrack {
    pub id: i32,
    pub name: String,
    pub language: Option<String>,
    pub codec: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleTrack {
    pub id: i32,
    pub name: String,
    pub language: Option<String>,
    pub forced: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QualityOption {
    pub id: String,
    pub name: String,
    pub bitrate: u32,
    pub resolution: String,
}
```

#### 3.2 Add Track Discovery and Management Methods
```rust
impl PlayerViewModel {
    pub async fn discover_tracks(&self) -> Result<()> {
        // Get tracks from current player backend
        let audio_tracks = self.get_player_audio_tracks().await?;
        let subtitle_tracks = self.get_player_subtitle_tracks().await?;
        
        self.audio_tracks.set(audio_tracks).await;
        self.subtitle_tracks.set(subtitle_tracks).await;
        
        // Auto-select preferred tracks based on user preferences
        self.auto_select_preferred_tracks().await;
        
        Ok(())
    }
    
    pub async fn select_audio_track(&self, track_index: usize) -> Result<()> {
        let tracks = self.audio_tracks.get().await;
        if let Some(track) = tracks.get(track_index) {
            // Update player backend
            self.apply_audio_track_to_player(track.id).await?;
            
            // Update ViewModel state
            self.selected_audio_track.set(Some(track_index)).await;
            
            // Save user preference
            self.save_audio_preference(track).await?;
            
            // Emit event for other components
            self.emit_track_change_event("audio", track).await;
        }
        Ok(())
    }
    
    pub async fn select_subtitle_track(&self, track_index: Option<usize>) -> Result<()> {
        match track_index {
            Some(idx) => {
                let tracks = self.subtitle_tracks.get().await;
                if let Some(track) = tracks.get(idx) {
                    self.apply_subtitle_track_to_player(track.id).await?;
                    self.selected_subtitle_track.set(Some(idx)).await;
                    self.save_subtitle_preference(Some(track)).await?;
                }
            }
            None => {
                // Disable subtitles
                self.apply_subtitle_track_to_player(-1).await?;
                self.selected_subtitle_track.set(None).await;
                self.save_subtitle_preference(None).await?;
            }
        }
        Ok(())
    }
}
```

#### 3.3 Replace UI Track Management with Reactive Bindings
**Current**: Direct player API calls in `setup_audio_track_menu()` and `setup_subtitle_track_menu()`
**New**: Reactive menu population and selection handling

```rust
// Audio track menu reactive binding
let audio_menu_binding = bind_menu_to_property(
    &controls.audio_button,
    vec![Arc::new(view_model.audio_tracks().clone())],
    |tracks| {
        tracks.iter().enumerate().map(|(idx, track)| {
            MenuItemData {
                label: track.name.clone(),
                action: format!("player.select-audio-track::{}", idx),
                enabled: true,
            }
        }).collect()
    },
);

// Subtitle track menu reactive binding  
let subtitle_menu_binding = bind_menu_to_property(
    &controls.subtitle_button,
    vec![Arc::new(view_model.subtitle_tracks().clone())],
    |tracks| {
        let mut items = vec![MenuItemData {
            label: "Off".to_string(),
            action: "player.disable-subtitles".to_string(),
            enabled: true,
        }];
        
        items.extend(tracks.iter().enumerate().map(|(idx, track)| {
            MenuItemData {
                label: track.name.clone(),
                action: format!("player.select-subtitle-track::{}", idx),
                enabled: true,
            }
        }));
        
        items
    },
);
```

**Success Criteria**:
- ✅ **COMPLETE**: Audio/subtitle buttons automatically disable when no tracks available
- ✅ **COMPLETE**: Track dropdown menus populate reactively from ViewModel properties
- ✅ **COMPLETE**: Track selection handled through ViewModel methods (no direct player calls in UI)
- ✅ **COMPLETE**: Removed `populate_track_menus()` polling timers
- ✅ **COMPLETE**: Reactive bindings replace manual track menu management

**Implementation Notes**:
- Used `bind_visibility_to_property` to disable buttons when track arrays are empty
- Implemented `bind_dropdown_to_property` for reactive menu population
- Added `AudioTrack`, `SubtitleTrack`, and `QualityOption` types to ViewModel
- Track discovery integrated into media loading process
- All track management state centralized in ViewModel properties

### Phase 4: Control Visibility Reactive System 🎯  
**Goal**: Replace manual control show/hide logic with reactive visibility
**Estimated Effort**: 2-3 hours (reduced - `show_controls` property already exists)
**Files**: `src/core/viewmodels/player_view_model.rs`, `src/platforms/gtk/ui/pages/player.rs`

#### 4.1 Add Missing PlayerViewModel Methods for Control Visibility
**Current**: `show_controls` property exists but lacks helper methods
**New**: Add convenience methods for timeout management

```rust
impl PlayerViewModel {
    pub async fn show_controls_temporarily(&self) {
        // Show controls immediately
        self.show_controls.set(true).await;
        
        // Schedule auto-hide after delay
        let show_controls = self.show_controls.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(PLAYER_CONTROLS_HIDE_DELAY_SECS)).await;
            show_controls.set(false).await;
        });
    }
    
    pub async fn toggle_controls_visibility(&self) {
        let visible = self.show_controls.get().await;
        if visible {
            self.show_controls.set(false).await;
        } else {
            self.show_controls_temporarily().await;
        }
    }
}
```

#### 4.2 Add Control Visibility Reactive Binding 
**Current**: Manual timeout logic with fade animations (lines 250-380)
**New**: Single reactive binding + property getter

```rust
// Add property getter to PlayerViewModel
impl PlayerViewModel {
    pub fn show_controls(&self) -> &Property<bool> {
        &self.show_controls
    }
}

// Replace manual show/hide with reactive binding
let controls_visibility_binding = bind_visibility_to_property(
    &controls_container,
    view_model.show_controls().clone(),
    |show| *show,
);

// Mouse movement triggers control visibility
let view_model_for_motion = view_model.clone();
hover_controller.connect_motion(move |_, _, _| {
    let vm = view_model_for_motion.clone();
    glib::spawn_future_local(async move {
        vm.show_controls_temporarily().await;
    });
});
```

**Success Criteria**:
- ✅ Controls show/hide reactively based on mouse movement
- ✅ Auto-hide timer managed by ViewModel, not UI layer
- ✅ No manual timeout management in UI code

### Phase 5: Skip Buttons and Markers Reactive System 🎯
**Goal**: Replace polling-based skip button visibility with reactive markers
**Estimated Effort**: 6-8 hours  
**Files**: `src/core/viewmodels/player_view_model.rs`, `src/platforms/gtk/ui/pages/player.rs`

#### 5.1 Enhance Marker Management
**Current**: 500ms polling timer checking current position against markers
**New**: Reactive computed properties for skip button visibility

```rust
impl PlayerViewModel {
    pub fn should_show_skip_intro(&self) -> ComputedProperty<bool> {
        ComputedProperty::new(
            vec![
                Arc::new(self.position.clone()) as Arc<dyn PropertyLike>,
                Arc::new(self.markers.clone()) as Arc<dyn PropertyLike>,
            ],
            |values| {
                let position = values[0].downcast_ref::<Duration>().unwrap();
                let markers = values[1].downcast_ref::<(Option<ChapterMarker>, Option<ChapterMarker>)>().unwrap();
                
                if let Some(intro_marker) = &markers.0 {
                    let pos_secs = position.as_secs();
                    pos_secs >= intro_marker.start_time_secs 
                        && pos_secs < intro_marker.end_time_secs
                } else {
                    false
                }
            },
        )
    }
    
    pub fn should_show_skip_credits(&self) -> ComputedProperty<bool> {
        ComputedProperty::new(
            vec![
                Arc::new(self.position.clone()) as Arc<dyn PropertyLike>,
                Arc::new(self.markers.clone()) as Arc<dyn PropertyLike>,
            ],
            |values| {
                let position = values[0].downcast_ref::<Duration>().unwrap();
                let markers = values[1].downcast_ref::<(Option<ChapterMarker>, Option<ChapterMarker>)>().unwrap();
                
                if let Some(credits_marker) = &markers.1 {
                    position.as_secs() >= credits_marker.start_time_secs
                } else {
                    false
                }
            },
        )
    }
    
    pub async fn skip_intro(&self) {
        if let Some(intro_marker) = &self.markers.get().await.0 {
            let skip_position = Duration::from_secs(intro_marker.end_time_secs);
            self.seek(skip_position).await;
        }
    }
    
    pub async fn skip_credits(&self) {
        // Skip to end to trigger next episode
        let duration = self.duration.get().await;
        if duration > Duration::ZERO {
            self.seek(duration - Duration::from_secs(1)).await;
        }
    }
}
```

#### 5.2 Reactive Skip Button Bindings
```rust
// Replace polling timer with reactive visibility
let skip_intro_binding = bind_visibility_to_property(
    &skip_intro_button,
    view_model.should_show_skip_intro(),
    |should_show| *should_show,
);

let skip_credits_binding = bind_visibility_to_property(
    &skip_credits_button, 
    view_model.should_show_skip_credits(),
    |should_show| *should_show,
);

// Button click handlers call ViewModel methods
skip_intro_button.connect_clicked({
    let vm = view_model.clone();
    move |_| {
        glib::spawn_future_local({
            let vm = vm.clone();
            async move { vm.skip_intro().await }
        });
    }
});
```

**Success Criteria**:
- ✅ Skip buttons appear/disappear reactively based on playback position
- ✅ No polling timers for marker checking
- ✅ Skip logic handled by ViewModel, not UI layer

### Phase 6: Auto-Play and Episode Transition Reactive System 🎯
**Goal**: Implement fully reactive auto-play countdown and episode transitions
**Estimated Effort**: 10-14 hours
**Files**: `src/core/viewmodels/player_view_model.rs`, `src/platforms/gtk/ui/pages/player.rs`

#### 6.1 Implement Auto-Play State Machine
**Current**: `AutoPlayState::Counting` and `AutoPlayState::Disabled` are dead code
**New**: Fully implemented auto-play countdown with reactive UI

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum AutoPlayState {
    Idle,
    Counting(u32), // Seconds remaining
    Disabled,      // User cancelled or disabled
}

impl PlayerViewModel {
    pub async fn start_auto_play_countdown(&self, seconds: u32) {
        self.auto_play_state.set(AutoPlayState::Counting(seconds)).await;
        
        // Start countdown timer
        let auto_play_state = self.auto_play_state.clone();
        let vm = self.clone();
        
        tokio::spawn(async move {
            for remaining in (1..=seconds).rev() {
                tokio::time::sleep(Duration::from_secs(1)).await;
                
                // Check if countdown was cancelled
                match auto_play_state.get().await {
                    AutoPlayState::Counting(_) => {
                        if remaining > 1 {
                            auto_play_state.set(AutoPlayState::Counting(remaining - 1)).await;
                        } else {
                            // Time's up, play next episode
                            vm.play_next_episode().await;
                            auto_play_state.set(AutoPlayState::Idle).await;
                        }
                    }
                    _ => break, // Countdown was cancelled
                }
            }
        });
    }
    
    pub async fn cancel_auto_play(&self) {
        self.auto_play_state.set(AutoPlayState::Disabled).await;
    }
    
    pub async fn play_next_episode(&self) {
        if let Some(next_episode) = self.next_episode.get().await {
            // Emit navigation event to switch to next episode
            self.emit_navigation_event(next_episode).await;
        }
    }
}
```

#### 6.2 Reactive Auto-Play Overlay
```rust
// Auto-play overlay visibility
let auto_play_overlay_binding = bind_visibility_to_property(
    &auto_play_overlay,
    view_model.auto_play_state().clone(),
    |state| !matches!(state, AutoPlayState::Idle),
);

// Countdown text display
let countdown_text_binding = bind_text_to_property(
    &countdown_label,
    view_model.auto_play_state().clone(),
    |state| match state {
        AutoPlayState::Counting(seconds) => format!("Next episode in {}s", seconds),
        AutoPlayState::Disabled => "Auto-play cancelled".to_string(),
        AutoPlayState::Idle => String::new(),
    },
);

// Next episode title display
let next_episode_binding = bind_text_to_property(
    &next_episode_label,
    view_model.next_episode().clone(),
    |episode| {
        episode.as_ref()
            .map(|ep| format!("Next: {}", ep.title))
            .unwrap_or_default()
    },
);
```

**Success Criteria**:
- ✅ Auto-play countdown displays reactively when episode ends
- ✅ Countdown updates in real-time without polling
- ✅ User can cancel countdown with reactive UI feedback
- ✅ Next episode plays automatically when countdown expires

### Phase 7: Eliminate Polling and Complete Migration 🧹
**Goal**: Remove all remaining polling timers and manual widget updates
**Estimated Effort**: 4-6 hours
**Files**: `src/platforms/gtk/ui/pages/player.rs`

#### 7.1 Replace Position Synchronization Polling
**Current**: 10-second timer sync between ViewModel and player backend
**New**: Event-driven position updates from player backend

```rust
// Remove polling timer in favor of player event callbacks
impl PlayerViewModel {
    pub async fn handle_player_position_update(&self, position: Duration) {
        self.position.set(position).await;
        
        // Save progress periodically (throttled)
        if let Some(media) = self.current_media.get().await {
            self.save_progress_throttled(media.id(), position, self.duration.get().await).await;
        }
    }
}

// Player backend emits position events instead of being polled
impl Player {
    fn setup_position_callback(&self, callback: impl Fn(Duration) + Send + 'static) {
        // MPV/GStreamer specific implementation to emit position updates
        // This replaces the 10s polling timer in the UI
    }
}
```

#### 7.2 Replace Playback Completion Polling  
**Current**: 1-second timer checking if playback completed
**New**: Event-driven completion detection

```rust
impl PlayerViewModel {
    pub async fn handle_playback_completed(&self) {
        self.playback_state.set(PlaybackState::Stopped).await;
        
        // Check if there's a next episode and start auto-play
        if let Some(_next) = self.next_episode.get().await {
            self.start_auto_play_countdown(10).await; // 10 second countdown
        }
    }
}
```

#### 7.3 Remove Dead Code
```rust
// Remove from PlayerViewModel:
// - PlaybackInfo struct (never used)
// - Dead code annotations on AutoPlayState variants
// - Unused property getters

// Clean up UI layer:
// - Remove all polling timers
// - Remove manual widget update calls
// - Remove try_read() patterns in favor of reactive bindings
```

**Success Criteria**:
- ✅ Zero polling timers in player UI code
- ✅ All widget updates happen through reactive bindings
- ✅ No manual property synchronization between ViewModel and player
- ✅ All dead code removed

## Implementation Guidelines

### Reactive Binding Patterns
Use existing binding functions from `src/platforms/gtk/ui/reactive/bindings.rs`:

- **`bind_visibility_to_property`**: Control visibility (show_controls, skip buttons, overlays)
- **`bind_text_to_property`**: Label updates (time displays, episode titles, countdown)  
- **`bind_icon_to_property`**: Button icons (play/pause, mute/unmute)
- **`bind_value_to_property`**: Slider controls (volume, progress)
- **`bind_menu_to_property`**: Dynamic menus (track selection) - **NEW, needs implementation**

### Event-Driven Updates
Replace polling with event callbacks:

1. **Player Backend Events**: Position updates, track discovery, playback completion
2. **ViewModel Events**: Property changes, state transitions
3. **UI Events**: User interactions trigger ViewModel methods, not direct player calls

### Property Lifecycle Management
Store binding handles in `_binding_handles: Rc<RefCell<Vec<BindingHandle>>>` to prevent premature cleanup.

### Testing Strategy
Each phase should include:
1. **Unit Tests**: ViewModel property behavior
2. **Integration Tests**: Reactive binding behavior  
3. **Manual Testing**: UI responsiveness and correctness

## Expected Outcomes

### Performance Improvements
- **Eliminate Polling Overhead**: Remove 4 periodic timers (500ms, 1s, 10s intervals)
- **Reduce CPU Usage**: Event-driven updates only when state actually changes
- **Improve Responsiveness**: Immediate UI updates via reactive bindings

### Code Quality Improvements  
- **Single Source of Truth**: All player state managed by ViewModel
- **Consistent Architecture**: 100% reactive pattern matching rest of application
- **Reduced Complexity**: Remove manual synchronization and polling logic
- **Better Testability**: ViewModel methods can be unit tested independently

### Maintainability Improvements
- **Eliminate Dead Code**: Remove unused structs and enum variants
- **Clear Separation of Concerns**: UI layer only handles presentation, ViewModel handles logic
- **Event-Driven Design**: Loose coupling between components via event system

## Risk Mitigation

### Phase-by-Phase Approach
Each phase is independently testable and can be rolled back if issues arise.

### Backward Compatibility
Maintain existing player API during migration to avoid breaking other components.

### Testing Coverage
Comprehensive testing at each phase prevents regressions in basic playback functionality.

### Performance Monitoring
Measure memory usage and CPU performance before/after each phase to ensure improvements.

## Timeline Estimate

- **Phase 3 (Track Management)**: ✅ **COMPLETED** - 8-12 hours 
- **Phase 4 (Control Visibility)**: 2-3 hours (reduced - property exists, just needs binding)
- **Phase 5 (Skip Buttons)**: 4-6 hours (reduced - ViewModel integration simplified)  
- **Phase 6 (Auto-Play)**: 6-8 hours (reduced - infrastructure exists)
- **Phase 7 (Cleanup)**: 2-3 hours (reduced - less dead code than expected)

**Total remaining: 14-20 hours** to complete 100% reactive migration.

## Success Metrics

### Functional Requirements
- ✅ All player controls work identically to current implementation
- ✅ No regressions in playback, seeking, or track selection
- ✅ Auto-play countdown works for episode transitions  
- ✅ Skip buttons appear/disappear based on content markers

### Technical Requirements
- ✅ Zero polling timers in player UI code
- ✅ 100% reactive property-based state management
- ✅ All widget updates via reactive bindings
- ✅ No dead code or unused properties
- ✅ Event-driven player backend integration

### Performance Requirements
- ✅ Reduced CPU usage from eliminated polling
- ✅ Improved UI responsiveness
- ✅ Memory usage remains stable or improves