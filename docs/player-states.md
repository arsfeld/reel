# Media Player Controls Visibility State Machine

## Overview

This document describes the state machine for managing the visibility of media player controls and cursor based on mouse movement and position. The system provides an intuitive user experience where controls appear when needed and disappear when not in use.

## Current Implementation Status

As of the refactoring completed in task #230, the player controls now use a proper 3-state machine implementation that replaces the previous complex boolean flag system.

## Core Principles

1. **Immediate Response**: Controls appear instantly when the mouse enters the window or moves
2. **Smart Hiding**: Controls hide after inactivity, but stay visible when being used
3. **Clean Viewing**: Both controls and cursor disappear together for distraction-free viewing
4. **Hover Persistence**: Controls remain visible while the mouse hovers over them

## State Definitions

### 1. HIDDEN
- **Description**: Controls and cursor are completely hidden
- **UI State**: No controls visible, no cursor visible
- **Entry Actions**: Hide controls UI, hide cursor
- **Implementation**: `ControlState::Hidden`

### 2. VISIBLE
- **Description**: Controls and cursor are visible with an inactivity timer running
- **UI State**: Controls visible, cursor visible
- **Entry Actions**: Show controls UI, show cursor, start/reset inactivity timer (3 seconds)
- **Implementation**: `ControlState::Visible { timer_id: Option<SourceId> }`

### 3. HOVERING
- **Description**: Controls and cursor are visible because mouse is over control elements
- **UI State**: Controls visible, cursor visible
- **Entry Actions**: Show controls UI, show cursor, cancel inactivity timer
- **Implementation**: `ControlState::Hovering`

## Events and Transitions

### State Transition Table

| Current State | Event | Next State | Actions |
|--------------|-------|------------|---------|
| HIDDEN | MouseEnterWindow | VISIBLE | Show controls, show cursor, start timer |
| HIDDEN | MouseMove | VISIBLE | Show controls, show cursor, start timer |
| VISIBLE | MouseLeaveWindow | HIDDEN | Hide controls, hide cursor immediately |
| VISIBLE | MouseMove | VISIBLE | Reset inactivity timer |
| VISIBLE | MouseEnterControls | HOVERING | Cancel timer |
| VISIBLE | InactivityTimeout | HIDDEN | Hide controls, hide cursor |
| HOVERING | MouseLeaveWindow | HIDDEN | Hide controls, hide cursor immediately |
| HOVERING | MouseLeaveControls | VISIBLE | Start inactivity timer |
| HOVERING | MouseMove | HOVERING | No action needed |

## Event Definitions

### MouseEnterWindow
- Triggered when the mouse cursor enters the player window boundaries
- Should be detected at the window/widget level

### MouseLeaveWindow
- Triggered when the mouse cursor exits the player window boundaries
- Should be detected at the window/widget level

### MouseMove
- Triggered when the mouse moves within the window
- Should have a small threshold to avoid micro-movements triggering visibility

### MouseEnterControls
- Triggered when the mouse enters the bounding box of any control element
- Includes: play/pause button, seek bar, volume slider, fullscreen button, etc.

### MouseLeaveControls
- Triggered when the mouse exits the bounding box of all control elements

### InactivityTimeout
- Triggered after 2-3 seconds of no mouse movement
- Only active in VISIBLE state
- Cancelled when entering HOVERING state

## Implementation Details

### Code Structure
```rust
// State enum definition
enum ControlState {
    Hidden,
    Visible { timer_id: Option<SourceId> },
    Hovering,
}

// Key fields in PlayerPage struct
control_state: ControlState,
last_mouse_position: Option<(f64, f64)>,
window_event_debounce: Option<SourceId>,
controls_overlay: Option<gtk::Box>,
inactivity_timeout_secs: u64,
mouse_move_threshold: f64,
window_event_debounce_ms: u64,
```

### State Transition Methods
- `transition_to_hidden()`: Transitions to Hidden state, handles timer cleanup
- `transition_to_visible()`: Transitions to Visible state, starts inactivity timer
- `transition_to_hovering()`: Transitions to Hovering state, cancels timer
- `controls_visible()`: Helper to check if controls should be shown
- `mouse_movement_exceeds_threshold()`: Checks if mouse movement is significant
- `is_mouse_over_controls()`: Detects if mouse is over control area

### Timer Management
```
INACTIVITY_TIMEOUT = 3 seconds (configurable)

- Start timer: When entering VISIBLE state
- Reset timer: On any significant MouseMove in VISIBLE state
- Cancel timer: When entering HOVERING state or HIDDEN state
- Timer action: Transition to HIDDEN state (sets from_timer flag to prevent double-remove)
```

### Hover Detection
The control area detection includes:
- Actual widget bounds checking when controls_overlay is available
- Fallback to bottom 20% heuristic for backward compatibility
- Configurable padding around controls for better usability

### Cursor Management
- Cursor visibility is synchronized with control state
- Uses GTK cursor APIs (`gtk::gdk::Cursor`)
- Hidden cursor uses "none" cursor type
- Visible cursor uses "default" cursor type

### Edge Cases Handled

1. **Timer Double-Remove Prevention**: The `from_timer` flag prevents attempting to remove an already-fired timer
2. **Rapid Enter/Exit**: Window events can be debounced (currently immediate for responsiveness)
3. **Fullscreen Transitions**: State machine continues working in fullscreen mode
4. **Keyboard Navigation**: Any keyboard input triggers MouseMove event to show controls
5. **Mouse Movement Threshold**: 5px threshold prevents micro-movements from triggering state changes

## Configuration Constants

```rust
const DEFAULT_INACTIVITY_TIMEOUT_SECS: u64 = 3;
const DEFAULT_MOUSE_MOVE_THRESHOLD: f64 = 5.0; // pixels
const DEFAULT_WINDOW_EVENT_DEBOUNCE_MS: u64 = 50; // milliseconds
const CONTROL_FADE_ANIMATION_MS: u64 = 200; // milliseconds for fade transition
```

## State Diagram

```
                 ┌──────────────┐
                 │              │
                 │    HIDDEN    │◄───────────────────┐
                 │              │                    │
                 └──────┬───────┘                    │
                        │                            │
              MouseEnterWindow                       │
                  MouseMove                    MouseLeaveWindow
                        │                            │
                        ▼                            │
                 ┌──────────────┐                    │
                 │              │                    │
    ┌───────────►│   VISIBLE    │────────────────────┘
    │            │              │
    │            └──────┬───────┘
    │                   │    ▲
    │         MouseEnterControls │
    │                   │    │ MouseLeaveControls
    │                   ▼    │
    │            ┌──────────────┐
    │            │              │
    └────────────│   HOVERING   │
MouseLeaveWindow │              │
                 └──────────────┘
```

## CSS Animations

The state machine integrates with CSS animations for smooth transitions:

```css
/* Fade animations defined in src/styles/player.css */
.player-controls.fade-in {
    animation: fadeInControls 200ms ease-out;
}

.player-controls.fade-out {
    animation: fadeOutControls 150ms ease-in;
}

@keyframes fadeInControls {
    from { opacity: 0; transform: translateY(12px); }
    to { opacity: 1; transform: translateY(0); }
}

@keyframes fadeOutControls {
    from { opacity: 1; transform: translateY(0); }
    to { opacity: 0; transform: translateY(8px); }
}
```

The CSS classes are applied based on the `controls_visible()` method return value.

## Migration from Boolean Flag System

The previous implementation used multiple boolean flags that led to complex interactions:
- `show_controls`: Whether controls were visible
- `controls_fade_out`: Whether in fade animation
- `mouse_over_controls`: Whether mouse was over controls
- `controls_timer`: Timer for auto-hide

These have been replaced with the single `ControlState` enum, eliminating race conditions and simplifying the logic.

## Benefits of This Design

1. **Predictable**: Users can easily understand when controls will appear/disappear
2. **Responsive**: Immediate feedback to user actions
3. **Non-intrusive**: Controls get out of the way for immersive viewing
4. **Accessible**: Controls stay visible when being actively used
5. **Efficient**: Minimal state transitions and clear logic
6. **Maintainable**: Single state variable instead of multiple flags
7. **Bug-Free**: Proper timer management prevents crashes from double-removal

## Testing Scenarios

1. Move mouse into window → Controls appear
2. Stop moving for 3 seconds → Controls disappear
3. Move mouse while controls hidden → Controls reappear
4. Hover over seek bar → Controls stay visible
5. Move mouse away from controls → Timer starts, controls hide after timeout
6. Exit window while hovering controls → Controls hide immediately
7. Re-enter window → Controls appear immediately
8. Rapidly enter/exit window → No flickering