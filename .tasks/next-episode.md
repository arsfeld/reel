# Next Episode Feature - Reactive Implementation Plan

## Overview

This plan outlines the implementation of a fully reactive next episode feature for the player UI, including:
- **Mini player overlay** showing next episode preview
- **Episode screenshot/thumbnail** display
- **Countdown timer** with auto-play functionality
- **User controls** for play now, cancel, or disable auto-play

The implementation follows the reactive principles established in the codebase, building on the existing 55% reactive player architecture.

**Dependencies**: Requires completion of Phase 6 from player-ui.md (Auto-Play State Machine)
**Estimated Effort**: 16-20 hours

## Architecture Design

### Component Structure

```
PlayerPage
├── VideoWidget (existing)
├── PlayerControls (existing)
└── NextEpisodeOverlay (NEW)
    ├── MiniPlayer
    │   ├── ThumbnailPreview
    │   ├── EpisodeInfo
    │   └── ProgressIndicator
    ├── CountdownTimer
    │   ├── CircularProgress
    │   └── TimeRemaining
    └── ActionButtons
        ├── PlayNowButton
        ├── CancelButton
        └── DisableAutoPlayToggle
```

### Reactive Data Flow

```
PlayerViewModel (Properties)
├── next_episode: Property<Option<MediaItem>>
├── auto_play_state: Property<AutoPlayState>
├── next_episode_thumbnail: Property<Option<DynamicImage>>
├── auto_play_enabled: Property<bool>
└── countdown_progress: ComputedProperty<f64>

UI Bindings
├── Overlay visibility → auto_play_state
├── Thumbnail image → next_episode_thumbnail
├── Episode info text → next_episode
├── Countdown progress → countdown_progress
└── Button states → auto_play_enabled
```

## Implementation Phases

### Phase 1: Enhance ViewModel with Next Episode Properties
**Goal**: Extend PlayerViewModel with comprehensive next episode support
**Estimated Effort**: 3-4 hours
**Files**: `src/core/viewmodels/player_view_model.rs`

#### 1.1 Add Next Episode Properties

```rust
pub struct PlayerViewModel {
    // Existing properties...
    
    // Enhanced next episode properties
    next_episode: Property<Option<MediaItem>>,
    next_episode_thumbnail: Property<Option<DynamicImage>>,
    auto_play_enabled: Property<bool>,
    auto_play_countdown_duration: Property<u32>, // Configurable countdown (5-30 seconds)
    next_episode_load_state: Property<LoadState>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoadState {
    Idle,
    Loading,
    Ready,
    Error(String),
}
```

#### 1.2 Add Computed Properties for UI

```rust
impl PlayerViewModel {
    pub fn countdown_progress(&self) -> ComputedProperty<f64> {
        ComputedProperty::new(
            vec![
                Arc::new(self.auto_play_state.clone()) as Arc<dyn PropertyLike>,
                Arc::new(self.auto_play_countdown_duration.clone()) as Arc<dyn PropertyLike>,
            ],
            |values| {
                let state = values[0].downcast_ref::<AutoPlayState>().unwrap();
                let total = values[1].downcast_ref::<u32>().unwrap();
                
                match state {
                    AutoPlayState::Counting(remaining) => {
                        (*remaining as f64) / (*total as f64)
                    }
                    _ => 0.0,
                }
            },
        )
    }
    
    pub fn should_show_next_episode_overlay(&self) -> ComputedProperty<bool> {
        ComputedProperty::new(
            vec![
                Arc::new(self.auto_play_state.clone()) as Arc<dyn PropertyLike>,
                Arc::new(self.next_episode.clone()) as Arc<dyn PropertyLike>,
            ],
            |values| {
                let state = values[0].downcast_ref::<AutoPlayState>().unwrap();
                let next = values[1].downcast_ref::<Option<MediaItem>>().unwrap();
                
                next.is_some() && !matches!(state, AutoPlayState::Idle)
            },
        )
    }
    
    pub fn next_episode_info(&self) -> ComputedProperty<NextEpisodeInfo> {
        ComputedProperty::new(
            vec![Arc::new(self.next_episode.clone()) as Arc<dyn PropertyLike>],
            |values| {
                let episode = values[0].downcast_ref::<Option<MediaItem>>().unwrap();
                
                episode.as_ref().map(|ep| NextEpisodeInfo {
                    title: ep.title.clone(),
                    show_title: ep.grandparent_title.clone().unwrap_or_default(),
                    season_episode: format!("S{}E{}", 
                        ep.parent_index.unwrap_or(0), 
                        ep.index.unwrap_or(0)),
                    duration: format_duration(ep.duration.unwrap_or_default()),
                    summary: ep.summary.clone().unwrap_or_default(),
                }).unwrap_or_default()
            },
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NextEpisodeInfo {
    pub title: String,
    pub show_title: String,
    pub season_episode: String,
    pub duration: String,
    pub summary: String,
}
```

#### 1.3 Add Next Episode Management Methods

```rust
impl PlayerViewModel {
    pub async fn load_next_episode(&self) -> Result<()> {
        self.next_episode_load_state.set(LoadState::Loading).await;
        
        // Get next episode from current media context
        if let Some(current) = self.current_media.get().await {
            match self.fetch_next_episode(&current).await {
                Ok(next) => {
                    self.next_episode.set(Some(next.clone())).await;
                    
                    // Pre-load thumbnail
                    if let Some(thumb_url) = &next.thumb {
                        self.load_episode_thumbnail(thumb_url).await?;
                    }
                    
                    self.next_episode_load_state.set(LoadState::Ready).await;
                }
                Err(e) => {
                    self.next_episode_load_state.set(
                        LoadState::Error(e.to_string())
                    ).await;
                }
            }
        }
        
        Ok(())
    }
    
    async fn load_episode_thumbnail(&self, url: &str) -> Result<()> {
        // Fetch and cache thumbnail
        let image = self.image_loader.load_image(url).await?;
        self.next_episode_thumbnail.set(Some(image)).await;
        Ok(())
    }
    
    pub async fn play_next_episode_now(&self) {
        // Cancel countdown
        self.auto_play_state.set(AutoPlayState::Idle).await;
        
        // Navigate to next episode
        if let Some(next) = self.next_episode.get().await {
            self.emit_navigation_event(NavigationEvent::PlayMedia(next)).await;
        }
    }
    
    pub async fn toggle_auto_play(&self) {
        let enabled = !self.auto_play_enabled.get().await;
        self.auto_play_enabled.set(enabled).await;
        
        // Save preference
        self.save_auto_play_preference(enabled).await;
        
        // Cancel current countdown if disabling
        if !enabled && matches!(
            self.auto_play_state.get().await, 
            AutoPlayState::Counting(_)
        ) {
            self.cancel_auto_play().await;
        }
    }
}
```

### Phase 2: Create Next Episode Overlay Widget
**Goal**: Build the GTK4 UI components for next episode display
**Estimated Effort**: 6-8 hours
**Files**: `src/platforms/gtk/ui/widgets/next_episode_overlay.rs` (NEW)

#### 2.1 Define Widget Structure

```rust
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

pub struct NextEpisodeOverlay {
    pub container: gtk::Overlay,
    mini_player: MiniPlayer,
    countdown_timer: CountdownTimer,
    action_buttons: ActionButtons,
    _binding_handles: Rc<RefCell<Vec<BindingHandle>>>,
}

struct MiniPlayer {
    container: gtk::Box,
    thumbnail: gtk::Picture,
    title_label: gtk::Label,
    show_label: gtk::Label,
    episode_label: gtk::Label,
    duration_label: gtk::Label,
    summary_label: gtk::Label,
    progress_bar: gtk::ProgressBar,
}

struct CountdownTimer {
    container: gtk::Box,
    circular_progress: gtk::DrawingArea,
    time_label: gtk::Label,
    message_label: gtk::Label,
}

struct ActionButtons {
    container: gtk::Box,
    play_now_button: gtk::Button,
    cancel_button: gtk::Button,
    auto_play_switch: gtk::Switch,
    auto_play_label: gtk::Label,
}
```

#### 2.2 Implement Widget Creation

```rust
impl NextEpisodeOverlay {
    pub fn new() -> Self {
        let container = gtk::Overlay::new();
        container.add_css_class("next-episode-overlay");
        
        // Create main overlay box
        let overlay_box = gtk::Box::new(gtk::Orientation::Horizontal, 20);
        overlay_box.set_halign(gtk::Align::End);
        overlay_box.set_valign(gtk::Align::End);
        overlay_box.set_margin_end(20);
        overlay_box.set_margin_bottom(100); // Above player controls
        overlay_box.add_css_class("next-episode-box");
        
        // Create mini player
        let mini_player = Self::create_mini_player();
        overlay_box.append(&mini_player.container);
        
        // Create countdown timer
        let countdown_timer = Self::create_countdown_timer();
        overlay_box.append(&countdown_timer.container);
        
        // Create action buttons
        let action_buttons = Self::create_action_buttons();
        overlay_box.append(&action_buttons.container);
        
        container.add_overlay(&overlay_box);
        
        Self {
            container,
            mini_player,
            countdown_timer,
            action_buttons,
            _binding_handles: Rc::new(RefCell::new(Vec::new())),
        }
    }
    
    fn create_mini_player() -> MiniPlayer {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 8);
        container.set_size_request(320, -1);
        container.add_css_class("mini-player");
        
        // Thumbnail with 16:9 aspect ratio
        let thumbnail = gtk::Picture::new();
        thumbnail.set_size_request(320, 180);
        thumbnail.add_css_class("episode-thumbnail");
        thumbnail.set_content_fit(gtk::ContentFit::Cover);
        
        // Episode info
        let info_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        info_box.set_margin_top(8);
        info_box.set_margin_bottom(8);
        info_box.set_margin_start(12);
        info_box.set_margin_end(12);
        
        let title_label = gtk::Label::new(None);
        title_label.set_xalign(0.0);
        title_label.add_css_class("episode-title");
        title_label.set_ellipsize(pango::EllipsizeMode::End);
        title_label.set_lines(1);
        
        let show_label = gtk::Label::new(None);
        show_label.set_xalign(0.0);
        show_label.add_css_class("episode-show");
        
        let metadata_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        
        let episode_label = gtk::Label::new(None);
        episode_label.add_css_class("episode-number");
        
        let duration_label = gtk::Label::new(None);
        duration_label.add_css_class("episode-duration");
        
        metadata_box.append(&episode_label);
        metadata_box.append(&gtk::Label::new(Some("•")));
        metadata_box.append(&duration_label);
        
        let summary_label = gtk::Label::new(None);
        summary_label.set_xalign(0.0);
        summary_label.add_css_class("episode-summary");
        summary_label.set_ellipsize(pango::EllipsizeMode::End);
        summary_label.set_lines(2);
        summary_label.set_wrap(true);
        
        let progress_bar = gtk::ProgressBar::new();
        progress_bar.add_css_class("episode-progress");
        progress_bar.set_visible(false); // Only show if episode has progress
        
        info_box.append(&title_label);
        info_box.append(&show_label);
        info_box.append(&metadata_box);
        info_box.append(&summary_label);
        info_box.append(&progress_bar);
        
        container.append(&thumbnail);
        container.append(&info_box);
        
        MiniPlayer {
            container,
            thumbnail,
            title_label,
            show_label,
            episode_label,
            duration_label,
            summary_label,
            progress_bar,
        }
    }
    
    fn create_countdown_timer() -> CountdownTimer {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 8);
        container.set_size_request(120, -1);
        container.add_css_class("countdown-timer");
        container.set_valign(gtk::Align::Center);
        
        // Circular progress indicator
        let circular_progress = gtk::DrawingArea::new();
        circular_progress.set_size_request(80, 80);
        circular_progress.add_css_class("circular-progress");
        
        let time_label = gtk::Label::new(None);
        time_label.add_css_class("countdown-time");
        
        let message_label = gtk::Label::new(Some("Playing next"));
        message_label.add_css_class("countdown-message");
        
        container.append(&circular_progress);
        container.append(&time_label);
        container.append(&message_label);
        
        CountdownTimer {
            container,
            circular_progress,
            time_label,
            message_label,
        }
    }
    
    fn create_action_buttons() -> ActionButtons {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 12);
        container.add_css_class("action-buttons");
        container.set_valign(gtk::Align::Center);
        
        // Primary actions
        let button_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        
        let play_now_button = gtk::Button::with_label("Play Now");
        play_now_button.add_css_class("suggested-action");
        
        let cancel_button = gtk::Button::with_label("Cancel");
        cancel_button.add_css_class("destructive-action");
        
        button_box.append(&play_now_button);
        button_box.append(&cancel_button);
        
        // Auto-play toggle
        let auto_play_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        auto_play_box.set_margin_top(8);
        
        let auto_play_label = gtk::Label::new(Some("Auto-play"));
        auto_play_label.add_css_class("dim-label");
        
        let auto_play_switch = gtk::Switch::new();
        
        auto_play_box.append(&auto_play_label);
        auto_play_box.append(&auto_play_switch);
        
        container.append(&button_box);
        container.append(&auto_play_box);
        
        ActionButtons {
            container,
            play_now_button,
            cancel_button,
            auto_play_switch,
            auto_play_label,
        }
    }
}
```

### Phase 3: Implement Reactive Bindings
**Goal**: Connect the overlay to ViewModel with reactive bindings
**Estimated Effort**: 4-5 hours
**Files**: `src/platforms/gtk/ui/widgets/next_episode_overlay.rs`, `src/platforms/gtk/ui/reactive/bindings.rs`

#### 3.1 Add Binding Methods

```rust
impl NextEpisodeOverlay {
    pub fn bind_to_view_model(&self, view_model: Arc<PlayerViewModel>) {
        let mut handles = self._binding_handles.borrow_mut();
        
        // Overlay visibility
        handles.push(bind_visibility_to_property(
            &self.container,
            view_model.should_show_next_episode_overlay(),
            |should_show| *should_show,
        ));
        
        // Mini player bindings
        self.bind_mini_player(&mut handles, &view_model);
        
        // Countdown timer bindings
        self.bind_countdown_timer(&mut handles, &view_model);
        
        // Action button bindings
        self.bind_action_buttons(&mut handles, &view_model);
    }
    
    fn bind_mini_player(
        &self,
        handles: &mut Vec<BindingHandle>,
        view_model: &Arc<PlayerViewModel>,
    ) {
        // Thumbnail image
        handles.push(bind_image_to_property(
            &self.mini_player.thumbnail,
            view_model.next_episode_thumbnail().clone(),
            |image| image.clone(),
        ));
        
        // Episode info
        let info = view_model.next_episode_info();
        
        handles.push(bind_text_to_computed_property(
            &self.mini_player.title_label,
            info.clone(),
            |info| info.title.clone(),
        ));
        
        handles.push(bind_text_to_computed_property(
            &self.mini_player.show_label,
            info.clone(),
            |info| info.show_title.clone(),
        ));
        
        handles.push(bind_text_to_computed_property(
            &self.mini_player.episode_label,
            info.clone(),
            |info| info.season_episode.clone(),
        ));
        
        handles.push(bind_text_to_computed_property(
            &self.mini_player.duration_label,
            info.clone(),
            |info| info.duration.clone(),
        ));
        
        handles.push(bind_text_to_computed_property(
            &self.mini_player.summary_label,
            info.clone(),
            |info| info.summary.clone(),
        ));
        
        // Progress bar (if episode was partially watched)
        handles.push(bind_value_to_property(
            &self.mini_player.progress_bar,
            view_model.next_episode_progress().clone(),
            |progress| progress.unwrap_or(0.0),
        ));
    }
    
    fn bind_countdown_timer(
        &self,
        handles: &mut Vec<BindingHandle>,
        view_model: &Arc<PlayerViewModel>,
    ) {
        // Time remaining text
        handles.push(bind_text_to_property(
            &self.countdown_timer.time_label,
            view_model.auto_play_state().clone(),
            |state| match state {
                AutoPlayState::Counting(seconds) => format!("{}s", seconds),
                _ => String::new(),
            },
        ));
        
        // Circular progress drawing
        let progress = view_model.countdown_progress();
        let drawing_area = self.countdown_timer.circular_progress.clone();
        
        drawing_area.set_draw_func(move |_, cr, width, height| {
            let progress_value = progress.get_blocking();
            Self::draw_circular_progress(cr, width, height, progress_value);
        });
        
        // Trigger redraw on progress change
        handles.push(progress.subscribe(move |_| {
            drawing_area.queue_draw();
        }));
    }
    
    fn bind_action_buttons(
        &self,
        handles: &mut Vec<BindingHandle>,
        view_model: &Arc<PlayerViewModel>,
    ) {
        // Play now button
        let vm = view_model.clone();
        self.action_buttons.play_now_button.connect_clicked(move |_| {
            let vm = vm.clone();
            glib::spawn_future_local(async move {
                vm.play_next_episode_now().await;
            });
        });
        
        // Cancel button
        let vm = view_model.clone();
        self.action_buttons.cancel_button.connect_clicked(move |_| {
            let vm = vm.clone();
            glib::spawn_future_local(async move {
                vm.cancel_auto_play().await;
            });
        });
        
        // Auto-play switch
        handles.push(bind_active_to_property(
            &self.action_buttons.auto_play_switch,
            view_model.auto_play_enabled().clone(),
            |enabled| *enabled,
        ));
        
        let vm = view_model.clone();
        self.action_buttons.auto_play_switch.connect_state_set(move |_, _| {
            let vm = vm.clone();
            glib::spawn_future_local(async move {
                vm.toggle_auto_play().await;
            });
            glib::Propagation::Proceed
        });
    }
    
    fn draw_circular_progress(
        cr: &cairo::Context, 
        width: i32, 
        height: i32, 
        progress: f64,
    ) {
        let center_x = width as f64 / 2.0;
        let center_y = height as f64 / 2.0;
        let radius = (width.min(height) as f64 / 2.0) - 4.0;
        
        // Background circle
        cr.set_source_rgba(0.5, 0.5, 0.5, 0.3);
        cr.arc(center_x, center_y, radius, 0.0, 2.0 * std::f64::consts::PI);
        cr.set_line_width(4.0);
        cr.stroke().unwrap();
        
        // Progress arc
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        let start_angle = -std::f64::consts::PI / 2.0;
        let end_angle = start_angle + (2.0 * std::f64::consts::PI * progress);
        cr.arc(center_x, center_y, radius, start_angle, end_angle);
        cr.stroke().unwrap();
    }
}
```

### Phase 4: Integrate with Player Page
**Goal**: Add the overlay to the player page and handle lifecycle
**Estimated Effort**: 2-3 hours
**Files**: `src/platforms/gtk/ui/pages/player.rs`

#### 4.1 Add Overlay to Player Page

```rust
pub struct PlayerPage {
    // Existing fields...
    next_episode_overlay: NextEpisodeOverlay,
}

impl PlayerPage {
    pub fn new() -> Self {
        // Existing setup...
        
        // Create next episode overlay
        let next_episode_overlay = NextEpisodeOverlay::new();
        
        // Add overlay to main container
        main_overlay.add_overlay(&next_episode_overlay.container);
        
        Self {
            // Existing fields...
            next_episode_overlay,
        }
    }
    
    pub async fn load_media(&self, media: MediaItem) {
        // Existing load logic...
        
        // Load next episode info
        let vm = self.view_model.clone();
        tokio::spawn(async move {
            if let Err(e) = vm.load_next_episode().await {
                log::warn!("Failed to load next episode: {}", e);
            }
        });
        
        // Bind overlay to view model
        self.next_episode_overlay.bind_to_view_model(self.view_model.clone());
    }
}
```

#### 4.2 Handle Playback Completion

```rust
impl PlayerViewModel {
    pub async fn handle_playback_near_end(&self) {
        // Check if we're within last 30 seconds
        let position = self.position.get().await;
        let duration = self.duration.get().await;
        
        if duration > Duration::ZERO {
            let remaining = duration - position;
            
            if remaining <= Duration::from_secs(30) && 
               self.auto_play_enabled.get().await &&
               self.next_episode.get().await.is_some() {
                // Start showing overlay with countdown
                let countdown_duration = self.auto_play_countdown_duration.get().await;
                self.start_auto_play_countdown(countdown_duration).await;
            }
        }
    }
    
    pub async fn handle_playback_completed(&self) {
        self.playback_state.set(PlaybackState::Stopped).await;
        
        // If auto-play is disabled but there's a next episode, show overlay without countdown
        if !self.auto_play_enabled.get().await && 
           self.next_episode.get().await.is_some() {
            self.auto_play_state.set(AutoPlayState::Disabled).await;
        }
    }
}
```

### Phase 5: Add CSS Styling
**Goal**: Style the overlay for a polished appearance
**Estimated Effort**: 1-2 hours
**Files**: `src/platforms/gtk/resources/style.css`

```css
/* Next Episode Overlay */
.next-episode-overlay {
    background: transparent;
}

.next-episode-box {
    background: alpha(@window_bg_color, 0.95);
    border-radius: 12px;
    padding: 16px;
    box-shadow: 0 4px 16px alpha(black, 0.4);
    transition: all 200ms ease-in-out;
}

.next-episode-box:hover {
    box-shadow: 0 6px 20px alpha(black, 0.5);
}

/* Mini Player */
.mini-player {
    background: @card_bg_color;
    border-radius: 8px;
    overflow: hidden;
}

.episode-thumbnail {
    background: @view_bg_color;
}

.episode-title {
    font-weight: bold;
    font-size: 14pt;
}

.episode-show {
    color: @warning_color;
    font-size: 10pt;
    font-weight: 600;
}

.episode-number,
.episode-duration {
    color: alpha(@window_fg_color, 0.7);
    font-size: 9pt;
}

.episode-summary {
    color: alpha(@window_fg_color, 0.8);
    font-size: 10pt;
    margin-top: 4px;
}

.episode-progress {
    margin-top: 8px;
    min-height: 3px;
}

/* Countdown Timer */
.countdown-timer {
    padding: 12px;
}

.circular-progress {
    margin: 0 auto;
}

.countdown-time {
    font-size: 24pt;
    font-weight: bold;
    margin-top: 8px;
}

.countdown-message {
    color: alpha(@window_fg_color, 0.7);
    font-size: 10pt;
}

/* Action Buttons */
.action-buttons {
    min-width: 140px;
}

.action-buttons button {
    min-height: 36px;
    font-weight: 600;
}

/* Animations */
@keyframes slideIn {
    from {
        transform: translateX(400px);
        opacity: 0;
    }
    to {
        transform: translateX(0);
        opacity: 1;
    }
}

.next-episode-box {
    animation: slideIn 300ms ease-out;
}
```

## Testing Strategy

### Unit Tests
1. **ViewModel Properties**: Test next episode loading, countdown logic, state transitions
2. **Computed Properties**: Verify countdown progress, visibility conditions
3. **Auto-play Logic**: Test enable/disable, countdown cancellation

### Integration Tests
1. **Reactive Bindings**: Verify UI updates when properties change
2. **Event Handling**: Test button clicks, switch toggles
3. **Lifecycle**: Test overlay appearance/disappearance timing

### Manual Testing Scenarios
1. **Episode Transition**: Verify overlay appears 30 seconds before episode end
2. **Countdown Behavior**: Test countdown accuracy and cancellation
3. **User Interactions**: Test all buttons and auto-play toggle
4. **Edge Cases**: 
   - No next episode available
   - Network failure loading thumbnail
   - Rapid play/pause during countdown
   - Window resize/fullscreen transitions

## Performance Considerations

### Optimizations
1. **Thumbnail Pre-loading**: Load next episode thumbnail when 50% through current episode
2. **Lazy Rendering**: Only create overlay widgets when first needed
3. **Efficient Redraws**: Limit circular progress updates to 10 FPS
4. **Memory Management**: Clear thumbnail cache when changing shows

### Monitoring
- Track overlay render time
- Monitor memory usage with thumbnails
- Measure countdown timer accuracy
- Profile reactive binding overhead

## Accessibility Features

1. **Keyboard Navigation**:
   - `Space`: Play next episode now
   - `Escape`: Cancel countdown
   - `Tab`: Navigate between buttons

2. **Screen Reader Support**:
   - Announce countdown time remaining
   - Describe next episode details
   - Indicate auto-play state

3. **Visual Indicators**:
   - High contrast mode support
   - Clear focus indicators
   - Sufficient text size

## Configuration Options

User-configurable settings:
```rust
pub struct AutoPlaySettings {
    pub enabled: bool,
    pub countdown_duration: u32, // 5-30 seconds
    pub show_thumbnails: bool,
    pub show_at_seconds_remaining: u32, // When to show overlay
}
```

## Success Criteria

### Functional Requirements
- ✅ Next episode overlay appears at appropriate time
- ✅ Countdown timer accurate to 1 second
- ✅ All user controls functional
- ✅ Smooth animations and transitions
- ✅ Thumbnail loads without blocking UI

### Technical Requirements
- ✅ 100% reactive implementation
- ✅ Zero polling timers
- ✅ All state managed by ViewModel
- ✅ Proper memory cleanup
- ✅ No UI thread blocking

### Performance Requirements
- ✅ Overlay renders in <100ms
- ✅ Smooth 60 FPS animations
- ✅ Memory usage <50MB for thumbnails
- ✅ CPU usage <5% during countdown

## Dependencies on Other Work

1. **Required Before Starting**:
   - Phase 6 from player-ui.md (Auto-Play State Machine)
   - Basic next episode data in MediaItem model

2. **Can Be Done in Parallel**:
   - Phase 5 from player-ui.md (Skip Buttons)
   - Phase 7 from player-ui.md (Cleanup)

3. **Follow-up Work**:
   - Persist auto-play preferences
   - Add episode preview on hover
   - Implement playlist/queue support

## Risk Mitigation

1. **Thumbnail Loading Failures**: Graceful fallback to placeholder image
2. **Timing Issues**: Use monotonic clock for countdown
3. **Memory Leaks**: Proper cleanup of image cache and bindings
4. **Performance Impact**: Lazy loading and progressive enhancement

## Timeline Estimate

- **Phase 1 (ViewModel Enhancement)**: 3-4 hours
- **Phase 2 (Widget Creation)**: 6-8 hours
- **Phase 3 (Reactive Bindings)**: 4-5 hours
- **Phase 4 (Integration)**: 2-3 hours
- **Phase 5 (Styling)**: 1-2 hours
- **Testing & Polish**: 2-3 hours

**Total: 18-25 hours**

## Conclusion

This implementation plan provides a comprehensive, fully reactive next episode feature that:
- Follows established reactive patterns in the codebase
- Provides excellent user experience with smooth animations
- Maintains high performance through efficient data flow
- Supports accessibility and configuration needs
- Integrates seamlessly with the existing player architecture

The phased approach allows for incremental development and testing, reducing risk and ensuring quality at each step.