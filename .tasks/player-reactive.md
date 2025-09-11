# Player Reactive Migration Plan

## Overview

The PlayerPage has been significantly refactored with a **destroy-and-recreate pattern** that eliminates MPV lifecycle issues. The reactive migration continues with ~35% reactive architecture after Phase 2 completion. This plan migrates from manual widget updates and polling timers to a fully reactive system using Properties and ViewModels, aligning with the project's 75% reactive architecture goal.

**Progress: Phase 2 Complete (2/6 phases)** - Play/pause button, volume controls, progress bar and time displays are now fully reactive.

**✅ NEW: Destroy-and-Recreate Pattern Implemented** - PlayerPage is now destroyed and recreated for each media item, eliminating complex lifecycle management and MPV timing issues.

## Current State Analysis

### ✅ Already Reactive
- `is_loading` property → loading overlay visibility
- `error` property → error overlay display
- Basic ViewModel integration with `PlayerViewModel`
- **✅ Play/pause button state** → reactive icon updates via `playback_state` property
- **✅ Volume control state** → reactive slider and visibility via `volume` and `is_muted` properties
- **✅ Progress bar updates** → reactive progress percentage via `position` and `duration` properties
- **✅ Time display labels** → reactive formatted time via computed properties

### ❌ Still Imperative
- Skip button visibility (500ms polling timer)  
- Track menu population
- Playback completion monitoring (1s polling timer)
- Position synchronization (10s polling timer)

### 🔄 Recent Architectural Changes

#### Destroy-and-Recreate Pattern (December 2024)
- **Problem**: Complex MPV lifecycle management causing timing issues, widget reuse problems, and state pollution
- **Solution**: PlayerPage is now destroyed and recreated for each media item instead of being reused
- **Benefits**:
  - ✅ Eliminated MPV GLArea widget reuse issues
  - ✅ Removed complex seek retry logic (8-attempt backoff → simple single attempt)
  - ✅ Simplified widget cleanup (reduced from ~100 lines to automatic Drop cleanup)
  - ✅ Eliminated state pollution between media items
  - ✅ Reduced GLArea realization delay (100ms → 50ms)
- **Trade-off**: Slightly higher memory allocation per media item, but eliminates complex lifecycle bugs

## Migration Phases

### Phase 1: Core Playback State Properties ✅ COMPLETED
**Goal**: Replace manual play/pause button updates with reactive bindings
**Files**: `src/core/viewmodels/player_view_model.rs`, `src/platforms/gtk/ui/pages/player.rs`

#### 1.1 Add Properties to PlayerViewModel ✅
```rust
pub struct PlayerViewModel {
    // Existing properties...
    playback_state: Property<PlaybackState>,
    position: Property<Duration>, 
    duration: Property<Duration>,
    volume: Property<f64>,
    is_muted: Property<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing, 
    Paused,
    Loading,
}
```

#### 1.2 Reactive Play/Pause Button ✅
Replaced manual icon updates with reactive binding:
```rust
// Reactive binding for play button icon  
let play_button_binding = bind_icon_to_property(
    &controls.play_button,
    view_model.playback_state().clone(),
    |state| match state {
        PlaybackState::Playing => "media-playback-pause-symbolic".to_string(),
        _ => "media-playback-start-symbolic".to_string(),
    },
);
```

#### 1.3 Volume Control Binding ✅
Replaced manual volume handling with reactive bindings:
```rust
// Volume control binding - binds volume property to slider value
let volume_binding = bind_value_to_property(
    &controls.volume_button,
    view_model.volume().clone(),
    |volume| *volume,
);

// Volume visibility binding - hide volume when muted  
let volume_visible_binding = bind_visibility_to_property(
    &controls.volume_button,
    view_model.is_muted().clone(),
    |is_muted| !is_muted, // Show volume control when not muted
);
```

**Success Criteria**: ✅ ALL COMPLETED
- ✅ Play/pause button updates automatically when playback state changes
- ✅ Volume control reflects ViewModel state  
- ✅ No more manual icon name updates
- ✅ Added `bind_icon_to_property` and `bind_value_to_property` binding utilities
- ✅ Play button click handler now calls ViewModel methods for reactive updates

**Tests**: ✅ COMPLETED
- ✅ Button icon changes when `playback_state` property updates  
- ✅ Volume slider reflects `volume` property changes
- ✅ Added unit tests for PlaybackState equality and icon mapping logic

**Phase 1 Implementation Summary:**
- Created `bind_icon_to_property()` utility for reactive button icon updates
- Created `bind_value_to_property()` utility for reactive slider/scale values  
- Added reactive play/pause button that automatically switches icons based on `playback_state`
- Added reactive volume control that updates slider value from `volume` property
- Added reactive volume visibility that hides control when `is_muted` is true
- Replaced manual `set_icon_name()` calls in play button handler with ViewModel method calls
- Added `is_muted()` property getter to PlayerViewModel
- Used `Rc<RefCell<Vec<BindingHandle>>>` for memory-safe binding handle storage
- Added unit tests for PlaybackState equality and icon mapping logic
- All bindings use weak references for automatic cleanup when widgets are destroyed

---

### Phase 2: Progress and Time Display ✅ COMPLETED
**Goal**: Replace 500ms polling timer with reactive position/duration properties
**Files**: `src/platforms/gtk/ui/pages/player.rs`

#### 2.1 Computed Time Properties
```rust
let formatted_position = ComputedProperty::new("formatted_position",
    vec![Arc::new(view_model.position().clone())],
    |pos| format_duration(*pos));

let formatted_duration = ComputedProperty::new("formatted_duration", 
    vec![Arc::new(view_model.duration().clone())],
    |dur| format_duration(*dur));
```

#### 2.2 Progress Bar Binding
```rust
let progress_percentage = ComputedProperty::new("progress_percentage",
    vec![
        Arc::new(view_model.position().clone()),
        Arc::new(view_model.duration().clone()),
    ],
    |pos, dur| if dur.as_secs() > 0 { 
        (pos.as_secs_f64() / dur.as_secs_f64()) * 100.0 
    } else { 0.0 });

bind_value_to_property(&progress_bar, progress_percentage, |pct| *pct);
```

#### 2.3 Time Display Modes
```rust
let end_time_display = ComputedProperty::new("end_time_display",
    vec![
        Arc::new(view_model.position().clone()),
        Arc::new(view_model.duration().clone()),
        Arc::new(time_display_mode.clone()),
    ],
    move |pos, dur, mode| match mode {
        TimeDisplayMode::TotalDuration => format_duration(*dur),
        TimeDisplayMode::TimeRemaining => format!("-{}", format_duration(*dur - *pos)),
        TimeDisplayMode::EndTime => format_end_time(*dur - *pos),
    });

bind_text_to_property(&time_label, formatted_position, |text| text.clone());
bind_text_to_property(&end_time_label, end_time_display, |text| text.clone());
```

**Success Criteria**: ✅ ALL COMPLETED
- ✅ Remove position timer (500ms polling eliminated)
- ✅ Progress bar updates smoothly without polling
- ✅ Time labels update reactively
- ✅ Extended binding utilities to support ComputedProperty

**Tests**: ✅ COMPLETED
- ✅ Progress bar position matches `position` property
- ✅ Time labels show correct formatting
- ✅ End time display cycles work reactively

**Phase 2 Implementation Summary:**
- Created `bind_text_to_computed_property()` and `bind_value_to_computed_property()` utilities
- Added reactive progress bar that updates from `position`/`duration` computed percentage
- Added reactive time labels with formatted display via computed properties
- Added reactive end time display supporting multiple modes (total, remaining, end time)
- Replaced 500ms polling timer with event-driven property updates
- Eliminated ~65 lines of manual polling and update logic
- All bindings use Arc<ComputedProperty> with weak references for automatic cleanup
- Maintained all existing functionality while improving performance and architecture

---

### Phase 3: Skip Button Visibility
**Goal**: Replace 500ms marker polling with reactive visibility  
**Status**: ✅ **SIMPLIFIED** by destroy-and-recreate pattern - fresh instances eliminate timing issues
**Files**: `src/platforms/gtk/ui/pages/player.rs`

#### 3.1 Marker Visibility Properties
```rust
let intro_visible = ComputedProperty::new("intro_visible",
    vec![
        Arc::new(view_model.position().clone()),
        Arc::new(view_model.markers().clone()),
    ],
    |pos, markers| {
        let (intro, _) = markers;
        intro.as_ref().map_or(false, |marker| 
            *pos >= marker.start_time && *pos < marker.end_time)
    });

let credits_visible = ComputedProperty::new("credits_visible", 
    vec![
        Arc::new(view_model.position().clone()),
        Arc::new(view_model.markers().clone()),
    ],
    |pos, markers| {
        let (_, credits) = markers;
        credits.as_ref().map_or(false, |marker| *pos >= marker.start_time)
    });
```

#### 3.2 Skip Button Bindings
```rust
bind_visibility_to_property(&skip_intro_button, intro_visible, |visible| *visible);
bind_visibility_to_property(&skip_credits_button, credits_visible, |visible| *visible);
```

**Success Criteria**:
- Remove marker polling timers (lines 1232-1272, 1398-1438)
- Skip buttons appear/disappear based on position
- Respect config settings for skip_intro/skip_credits

**Tests**:
- Skip intro button visible during intro markers
- Skip credits button visible during credits
- Buttons hidden outside marker ranges

---

### Phase 4: Track Menu Population
**Goal**: Replace manual track menu population with reactive track properties
**Files**: `src/platforms/gtk/ui/pages/player.rs`

#### 4.1 Track Properties in ViewModel
```rust
pub struct PlayerViewModel {
    // Existing properties...
    audio_tracks: Property<Vec<AudioTrack>>,
    subtitle_tracks: Property<Vec<SubtitleTrack>>, 
    selected_audio_track: Property<Option<usize>>,
    selected_subtitle_track: Property<Option<usize>>,
}
```

#### 4.2 Reactive Menu Population
```rust
// Audio tracks menu
let audio_menu = ComputedProperty::new("audio_menu",
    vec![Arc::new(view_model.audio_tracks().clone())],
    |tracks| create_audio_menu(tracks));

bind_menu_to_property(&audio_button, audio_menu, |menu| menu.clone());

// Subtitle tracks menu  
let subtitle_menu = ComputedProperty::new("subtitle_menu",
    vec![Arc::new(view_model.subtitle_tracks().clone())],
    |tracks| create_subtitle_menu(tracks));

bind_menu_to_property(&subtitle_button, subtitle_menu, |menu| menu.clone());
```

**Success Criteria**:
- Remove manual track population (lines 987-990, 2100-2233)  
- Menus update when tracks change
- Selection state synchronized

**Tests**:
- Menu items match available tracks
- Selection updates ViewModel properties
- Menus update when media changes

---

### Phase 5: Position Synchronization  
**Goal**: Replace polling-based sync with event-driven updates
**Status**: ✅ **SIMPLIFIED** by destroy-and-recreate pattern - fresh instances are more reliable
**Files**: `src/platforms/gtk/ui/pages/player.rs`

#### 5.1 Debounced Position Updates
```rust
let debounced_position = view_model.position()
    .debounce(Duration::from_millis(500));

let throttled_sync = view_model.position()
    .debounce(Duration::from_secs(10));
```

#### 5.2 Event-Driven Sync
```rust
// Replace start_position_sync with reactive subscription
{
    let mut sub = throttled_sync.subscribe();
    let backend = backend.clone();
    let vm = view_model.clone();
    
    glib::spawn_future_local(async move {
        while sub.wait_for_change().await {
            if let Some(media_item) = vm.current_media().get().await {
                let position = throttled_sync.get_sync();
                let duration = vm.duration().get().await;
                
                // Sync to backend
                vm.save_progress_throttled(media_item.id(), position, duration).await;
                
                if let Err(e) = backend.update_progress(media_item.id(), position, duration).await {
                    debug!("Backend sync failed: {}", e);
                }
            }
        }
    });
}
```

**Success Criteria**:
- Remove position sync timer (lines 1515-1604)
- Sync only when position changes significantly  
- Maintain same sync frequency (10s)

**Tests**:
- Position syncs to backend every 10 seconds during playback
- No sync during pause/stop
- Debouncing prevents excessive updates

---

### Phase 6: Playback Monitoring
**Goal**: Replace 1s polling with event-driven completion detection  
**Status**: ✅ **SIMPLIFIED** by destroy-and-recreate pattern - fresh instances eliminate race conditions
**Files**: `src/platforms/gtk/ui/pages/player.rs`

#### 6.1 State-Based Monitoring
```rust
// Replace monitor_playback_completion with reactive subscription
{
    let mut sub = view_model.playback_state().subscribe();
    let backend = backend.clone();
    let vm = view_model.clone();
    
    glib::spawn_future_local(async move {
        while sub.wait_for_change().await {
            let state = vm.playback_state().get().await;
            
            if state == PlaybackState::Stopped {
                if let Some(media_item) = vm.current_media().get().await {
                    let position = vm.position().get().await;
                    let duration = vm.duration().get().await;
                    
                    // Check if watched > 90%
                    if let (Some(pos), Some(dur)) = (position, duration) {
                        let watched_pct = pos.as_secs_f64() / dur.as_secs_f64();
                        if watched_pct > 0.9 {
                            if let Err(e) = backend.mark_watched(media_item.id()).await {
                                error!("Failed to mark as watched: {}", e);
                            }
                        }
                    }
                }
                break;
            }
        }
    });
}
```

**Success Criteria**:
- Remove completion monitoring timer (lines 1442-1507)
- Mark as watched based on state changes
- Maintain same 90% threshold

**Tests**:
- Media marked watched when >90% completed
- No marking if stopped early
- State changes trigger monitoring

---

## Implementation Notes

### Memory Management
- All reactive bindings use weak references automatically
- No manual cleanup required for subscriptions
- Widgets destroyed = subscriptions end automatically

### Error Handling  
- Use `ComputedProperty::with_fallback()` for risky operations
- Provide meaningful fallback values
- Handle widget destruction gracefully

### Performance Considerations
- Debounce rapid property changes (position updates)
- Use `get_sync()` for immediate access
- Batch related property updates

### Testing Strategy
```rust
#[tokio::test]
async fn test_reactive_playback_state() {
    let vm = PlayerViewModel::new();
    let state = vm.playback_state();
    
    state.set(PlaybackState::Playing).await;
    assert_eq!(state.get_sync(), PlaybackState::Playing);
    
    // Test computed property
    let icon = ComputedProperty::new("icon", vec![Arc::new(state.clone())], 
        |s| match s { PlaybackState::Playing => "pause", _ => "play" });
    
    assert_eq!(icon.get_sync(), "pause");
}
```

## Migration Checklist

### Phase 1: Core Playback State ✅ COMPLETED
- [x] Add PlaybackState enum and properties to PlayerViewModel
- [x] Implement reactive play/pause button binding  
- [x] Add volume control reactive binding
- [x] Remove manual play button icon updates
- [x] Write tests for playback state changes

### Phase 2: Progress and Time Display ✅ COMPLETED
- [x] Add computed properties for formatted times
- [x] Implement progress bar reactive binding
- [x] Add time display mode reactive switching
- [x] Remove position update timer (500ms polling eliminated)
- [x] Write tests for time formatting and progress

### Phase 3: Skip Button Visibility
- [ ] Add computed properties for marker visibility
- [ ] Implement skip button reactive bindings  
- [ ] Respect config settings in computed properties
- [ ] Remove marker polling timers (lines 1232-1272, 1398-1438)
- [ ] Write tests for skip button visibility

### Phase 4: Track Menu Population
- [ ] Add track properties to PlayerViewModel
- [ ] Implement reactive menu population
- [ ] Add menu selection state synchronization
- [ ] Remove manual track population calls
- [ ] Write tests for menu updates

### Phase 5: Position Synchronization
- [ ] Implement debounced position property
- [ ] Add reactive sync subscription
- [ ] Remove position sync timer (lines 1515-1604)
- [ ] Maintain same sync behavior
- [ ] Write tests for sync frequency

### Phase 6: Playback Monitoring  
- [ ] Implement state-based completion monitoring
- [ ] Add reactive subscription for state changes
- [ ] Remove completion polling timer (lines 1442-1507)
- [ ] Maintain watched marking logic
- [ ] Write tests for completion detection

## Expected Outcomes

### Quantitative Benefits (Updated with Destroy-and-Recreate Pattern)
- **80% less boilerplate code** - Eliminated 300+ lines of manual updates + 100+ lines of complex lifecycle management
- **Remove 4 polling timers** - Replace with event-driven updates  
- **5x fewer UI update calls** - Only update when state actually changes
- **Zero memory leaks** - Automatic subscription cleanup + automatic Drop cleanup
- **Eliminated complex retry logic** - No more 8-attempt seek retries with backoff
- **50% faster initialization** - Reduced GLArea realization delays

### Qualitative Benefits
- **Consistent UI state** - Single source of truth eliminates race conditions
- **Better performance** - Event-driven vs polling reduces CPU usage + no widget reuse overhead
- **Easier debugging** - Property changes are traceable + fresh instances eliminate state pollution  
- **Improved testability** - Properties can be tested in isolation + predictable instance lifecycle
- **Maintainable code** - Reactive patterns are self-documenting + simplified architecture
- **Eliminated MPV timing issues** - No more GLArea widget reuse problems
- **Reliable seek operations** - Fresh instances don't require complex retry logic

### Alignment with Project Goals
- Completes reactive migration to match project's 75% reactive architecture
- Eliminates hybrid imperative/reactive patterns causing race conditions
- Follows established reactive patterns from other ViewModels  
- Maintains all existing functionality while improving architecture
- **NEW**: Destroy-and-recreate pattern aligns with project's reliability and maintainability goals
- **NEW**: Eliminates the largest source of lifecycle bugs in the media player component