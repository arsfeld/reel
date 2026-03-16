---
title: "feat: macOS Video Playback Rendering"
type: feat
status: completed
date: 2026-03-16
origin: docs/brainstorms/2026-03-15-macos-swiftui-rewrite-brainstorm.md
---

# macOS Video Playback Rendering

## Overview

Wire up actual video frame rendering in the macOS SwiftUI frontend. The Zig core already has a complete libmpv OpenGL render pipeline (proven working on GTK4/Linux). The macOS app has the player controls UI, PlayerModel state polling, and keyboard shortcuts — but `PlayerNSView` is a plain black `NSView` that never renders video frames. The C ABI does not export the three render functions needed by the frontend.

This plan closes that gap: export render functions from the C ABI, build an `NSOpenGLView` that drives mpv rendering via CVDisplayLink, integrate it into SwiftUI, and add watch progress save/resume.

## Problem Statement / Motivation

The macOS player is non-functional for video. Audio plays (mpv processes the file and outputs sound), but the screen stays black. This makes the entire macOS app unusable as a media center. The GTK4 frontend has working video rendering using the same Zig core, proving the render pipeline is correct — it just isn't exposed through the C ABI for macOS to consume.

(see brainstorm: `docs/brainstorms/2026-03-15-macos-swiftui-rewrite-brainstorm.md` — "Bridge only the mpv player" decision)

## Proposed Solution

1. Export three render C ABI functions from `lib.zig` / `reel.h`
2. Replace `PlayerNSView` (plain NSView) with an `NSOpenGLView` subclass that initializes the mpv render context and renders frames
3. Use CVDisplayLink (or CADisplayLink) for vsync-driven frame presentation
4. Share the existing `PlayerModel.player` pointer with the render view (single mpv instance for both control and rendering)
5. Add watch progress persistence and resume-from-position

## Technical Considerations

### Architecture: Single Player Instance

The render view must use the **same** `ReelPlayer*` that `PlayerModel` already owns. The GTK frontend has a single player that is both controlled and rendered — macOS must follow the same pattern.

```
PlayerModel (owns ReelPlayer*)
    │
    ├── Controls: play/pause/seek/volume via C ABI
    ├── State polling: position/duration/state every 250ms
    └── Passes player pointer to VideoPlayerView
            │
            └── NSOpenGLView calls reel_player_init_render / reel_player_render
```

### Initialization Ordering (Critical)

mpv requires the render context to exist **before** file loading to capture the first frame. The sequence must be:

1. `PlayerModel.createPlayer()` — creates mpv instance
2. `NSOpenGLView.prepareOpenGL()` — OpenGL context ready
3. `reel_player_init_render(player, getProcAddress)` — render context created
4. `reel_player_load_file(player, path)` — file loads, first frame renders

`PlayerModel.play()` must coordinate with the view: create the player, wait for the render context to be initialized, then load the file.

### OpenGL on macOS (Pragmatic Choice)

OpenGL is deprecated on macOS but still functional on macOS 15 (current target). The Zig core's render pipeline is OpenGL-based, and the GTK frontend uses it successfully. Using OpenGL now gives us:

- Proven render pipeline (identical to GTK)
- No Zig core changes beyond C ABI exports
- Working playback immediately

Metal migration (via MoltenVK or mpv's `--gpu-api=vulkan`) is a future optimization, not a blocker.

### Thread Safety

Three threads touch the player:

| Thread | Operation | Guard |
|--------|-----------|-------|
| Main thread | `pollState()`, `play()`, `stop()` | Timer on main RunLoop |
| CVDisplayLink thread | `reel_player_render()` | Only reads render_ctx, calls mpv_render_context_render (thread-safe per mpv docs) |
| mpv internal thread | Update callback | Must NOT call any mpv API — only signals main thread to queue redraw |

mpv's render API is designed for this: `mpv_render_context_render` is safe to call from any single thread (not concurrently), and the update callback explicitly documents that it runs on an internal thread. The CVDisplayLink callback fires on a dedicated thread, and we serialize render calls through it.

### `getProcAddress` on macOS

The GTK frontend uses `eglGetProcAddress`. macOS doesn't have EGL, but `epoxy` is already linked. Use `epoxy_get_proc_address` which works cross-platform and doesn't require a context parameter. This keeps `get_proc_address_ctx = null` in the Zig core unchanged.

### Retina Display Handling

All modern Macs are Retina. The `NSOpenGLView` must use `convertToBacking(bounds)` to get physical pixel dimensions for the FBO, not logical points. The legacy `VideoView.swift` already does this correctly — carry the pattern forward.

## Implementation Phases

### Phase 1: C ABI Render Exports

**Goal**: Export the three render functions from the Zig core so macOS Swift can call them.

**Tasks:**

1. Add three `export fn` wrappers in `src/lib.zig`:

```zig
// src/lib.zig — new exports

export fn reel_player_init_render(
    pw: ?*PlayerWrapper,
    get_proc_address: *const fn (?*anyopaque, [*c]const u8) callconv(.c) ?*anyopaque,
) c_int {
    const p = pw orelse return -1;
    p.p.initRender(get_proc_address) catch return -4; // REEL_ERR_RENDER
    return 0;
}

export fn reel_player_set_render_update_callback(
    pw: ?*PlayerWrapper,
    callback: ?*const fn (?*anyopaque) callconv(.c) void,
    ctx: ?*anyopaque,
) void {
    const p = pw orelse return;
    if (callback) |cb| {
        p.p.setRenderUpdateCallback(cb, ctx);
    }
}

export fn reel_player_render(
    pw: ?*PlayerWrapper,
    fbo: c_int,
    width: c_int,
    height: c_int,
) void {
    const p = pw orelse return;
    p.p.render(fbo, width, height);
}
```

2. Add C prototypes in `include/reel.h`:

```c
/* ── Player Rendering ───────────────────────────────────── */

typedef void* (*ReelGetProcAddressFn)(void* ctx, const char* name);
typedef void (*ReelRenderUpdateFn)(void* ctx);

/** Initialize OpenGL render context. Call after GL context is current. */
ReelError reel_player_init_render(ReelPlayer* player,
                                   ReelGetProcAddressFn get_proc_address);

/** Set callback invoked when mpv has a new frame. Fires on mpv's thread. */
void reel_player_set_render_update_callback(ReelPlayer* player,
                                             ReelRenderUpdateFn callback,
                                             void* ctx);

/** Render current frame into the given OpenGL FBO. */
void reel_player_render(ReelPlayer* player,
                         int fbo, int width, int height);
```

**Files:**
- `src/lib.zig` — add 3 export functions (~30 lines)
- `include/reel.h` — add typedefs + 3 prototypes (~15 lines)

**Success criteria:**
- [x] `zig build` succeeds
- [x] `swift build` links without undefined symbols for the new functions
- [ ] GTK frontend still works (no regressions)

---

### Phase 2: OpenGL Rendering View

**Goal**: Replace the black `PlayerNSView` with an `NSOpenGLView` that renders mpv frames.

**Tasks:**

1. **Replace `PlayerNSView` in `PlayerView.swift`** with a new `PlayerOpenGLView` (NSOpenGLView subclass):

```swift
// PlayerView.swift — PlayerOpenGLView

class PlayerOpenGLView: NSOpenGLView {
    var player: OpaquePointer?  // ReelPlayer* — set by VideoPlayerView
    private var displayLink: CVDisplayLink?
    private var renderReady = false

    override init(frame: NSRect) {
        let attrs: [NSOpenGLPixelFormatAttribute] = [
            UInt32(NSOpenGLPFAOpenGLProfile), UInt32(NSOpenGLProfileVersion3_2Core),
            UInt32(NSOpenGLPFAColorSize), 24,
            UInt32(NSOpenGLPFAAlphaSize), 8,
            UInt32(NSOpenGLPFADepthSize), 24,
            UInt32(NSOpenGLPFADoubleBuffer),
            UInt32(NSOpenGLPFAAccelerated),
            0
        ]
        let pixelFormat = NSOpenGLPixelFormat(attributes: attrs)!
        super.init(frame: frame, pixelFormat: pixelFormat)!
        wantsBestResolutionOpenGLSurface = true
    }

    override func prepareOpenGL() {
        super.prepareOpenGL()
        openGLContext?.makeCurrentContext()
        initializeRenderContext()
        setupDisplayLink()
    }

    private func initializeRenderContext() {
        guard let player = player else { return }
        let result = reel_player_init_render(player, { _, name in
            // Use epoxy for GL proc address resolution
            guard let name = name else { return nil }
            return unsafeBitCast(epoxy_get_proc_address(name), to: UnsafeMutableRawPointer?.self)
        })
        renderReady = (result == 0)
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let player = player, renderReady,
              let ctx = openGLContext else { return }
        ctx.makeCurrentContext()

        let bounds = convertToBacking(self.bounds)
        var fbo: GLint = 0
        glGetIntegerv(GLenum(GL_FRAMEBUFFER_BINDING), &fbo)

        reel_player_render(player, fbo, Int32(bounds.width), Int32(bounds.height))
        ctx.flushBuffer()
    }

    // ... CVDisplayLink setup, cleanup, keyboard handling
}
```

2. **Set up CVDisplayLink** for vsync-driven rendering (carry pattern from legacy `VideoView.swift`). Use `Unmanaged.passRetained(self)` and release in `deinit` to prevent use-after-free.

3. **Wire mpv update callback** to queue redraws:

```swift
private func setupUpdateCallback() {
    guard let player = player else { return }
    let viewPtr = Unmanaged.passUnretained(self).toOpaque()
    reel_player_set_render_update_callback(player, { ctx in
        guard let ctx = ctx else { return }
        let view = Unmanaged<PlayerOpenGLView>.fromOpaque(ctx).takeUnretainedValue()
        DispatchQueue.main.async { view.needsDisplay = true }
    }, viewPtr)
}
```

4. **Move keyboard handling** from current `PlayerNSView` into `PlayerOpenGLView`. Fix the mute key handler (currently a no-op). Add missing subtitle (`S`) and audio (`A`) cycle shortcuts.

**Files:**
- `macos/Reel/Sources/Views/PlayerView.swift` — replace PlayerNSView with PlayerOpenGLView

**Success criteria:**
- [x] Video frames render in the SwiftUI player view
- [x] CVDisplayLink drives smooth vsync rendering
- [x] Retina displays render at full resolution (backing pixels, not points)
- [x] All keyboard shortcuts work: Space, arrows, F, M, S, A, Escape
- [x] No crash on view deallocation (CVDisplayLink properly stopped)

---

### Phase 3: SwiftUI Integration & Player Lifecycle

**Goal**: Wire `PlayerOpenGLView` into SwiftUI via `NSViewRepresentable` and coordinate initialization ordering with `PlayerModel`.

**Tasks:**

1. **Update `VideoPlayerView` (NSViewRepresentable)** to pass the player pointer and coordinate lifecycle:

```swift
struct VideoPlayerView: NSViewRepresentable {
    let playerModel: PlayerModel

    func makeNSView(context: Context) -> PlayerOpenGLView {
        let view = PlayerOpenGLView(frame: .zero)
        view.player = playerModel.player  // Share the same ReelPlayer*
        view.playerModel = playerModel
        return view
    }

    func updateNSView(_ nsView: PlayerOpenGLView, context: Context) {
        // Update player pointer if it changed (e.g., after recreate)
        nsView.player = playerModel.player
    }
}
```

2. **Restructure `PlayerModel.play()` initialization ordering**:

```swift
func play(filePath: String) {
    createPlayer()
    currentFilePath = filePath
    isActive = true  // This triggers PlayerScreen to appear with VideoPlayerView
    // File loading happens AFTER the view calls prepareOpenGL + initRender
    // via a callback or notification from the OpenGL view
}

func onRenderContextReady() {
    // Called by PlayerOpenGLView after successful initRender
    guard let p = player, let path = currentFilePath else { return }
    let err = reel_player_load_file(p, path)
    guard err.rawValue == 0 else { return }
    isPlaying = true
    startPolling()
}
```

3. **Handle view lifecycle in SwiftUI**: `prepareOpenGL` fires when the `NSOpenGLView` is added to a window. SwiftUI's timing is non-deterministic, so `PlayerModel` must not load the file until the render context is confirmed ready.

4. **Handle player teardown**: When user presses Escape or playback ends:
   - Stop CVDisplayLink
   - Call `reel_player_stop()`
   - Set `isActive = false` (SwiftUI removes the player overlay)
   - Render view's `deinit` cleans up the display link

**Files:**
- `macos/Reel/Sources/Views/PlayerView.swift` — update VideoPlayerView
- `macos/Reel/Sources/Models/PlayerModel.swift` — restructure play() lifecycle
- `macos/Reel/Sources/ContentView.swift` — verify Escape handling

**Success criteria:**
- [x] Clicking Play from DetailView shows video in full window
- [x] First frame renders (no black flash before video appears)
- [x] Escape returns to previous view cleanly (no dangling render context)
- [x] Playing a new file while one is active transitions smoothly
- [x] No use-after-free crashes during SwiftUI view transitions

---

### Phase 4: Playback Polish

**Goal**: Watch progress, error handling, end-of-file behavior, cursor hiding.

**Tasks:**

1. **Watch progress save** — Add to `PlayerModel.pollState()`:
   - Track time since last save with a counter
   - Save every 10 seconds via `reel_library_update_watch_progress()`
   - Save immediately on pause and stop
   - Mark item as "watched" when position exceeds 90% of duration

```swift
private var lastSaveTime: Date = .distantPast

private func maybeSaveProgress() {
    guard isPlaying, Date().timeIntervalSince(lastSaveTime) >= 10 else { return }
    saveProgress()
}

private func saveProgress() {
    guard let p = player, let db = appState?.library else { return }
    let posMs = Int64(position * 1000)
    let durMs = Int64(duration * 1000)
    let watched: Int32 = (duration > 0 && position / duration > 0.9) ? 1 : 0
    reel_library_update_watch_progress(db, currentMediaItemId, posMs, durMs, watched)
    lastSaveTime = Date()
}
```

2. **Resume from saved position** — In `play()`, after file loads:
   - Query `reel_library_get_watch_progress()` for the media item
   - If position exists and item not marked watched, `seekAbsolute()` to saved position

3. **End-of-file handling** — In `pollState()`:
   - When `state == STOPPED` and `isActive`, auto-dismiss the player
   - Mark item as watched
   - Set `isActive = false` to return to the library view

4. **Escape key behavior** — Match Infuse convention:
   - If in macOS fullscreen: first Escape exits fullscreen only
   - Second Escape (or Escape in windowed mode): exits the player

5. **Mouse cursor hiding** — During fullscreen playback when controls are hidden:
   - `NSCursor.hide()` when controls fade out
   - `NSCursor.unhide()` on mouse movement or Escape

6. **Loading indicator** — Show a `ProgressView` spinner while `isActive && !isPlaying` (file is loading but first frame hasn't arrived)

7. **Playback error handling** — Surface errors from `reel_player_load_file()`:
   - Show an alert with the file path and error code
   - Dismiss the player overlay and return to library

**Files:**
- `macos/Reel/Sources/Models/PlayerModel.swift` — watch progress, resume, end-of-file
- `macos/Reel/Sources/Views/PlayerView.swift` — loading indicator, cursor hiding, Escape behavior
- `macos/Reel/Sources/ContentView.swift` — Escape key coordination

**Success criteria:**
- [x] Watch progress saves every 10s and on pause/stop
- [x] Resuming a partially watched item seeks to saved position
- [x] End-of-file dismisses player and marks item watched
- [x] Escape exits fullscreen first, then exits player on second press
- [x] Mouse cursor hides during fullscreen playback
- [x] Loading spinner shown while file is buffering
- [x] Playback errors shown to user with actionable message

## System-Wide Impact

- **Interaction graph**: Play click -> `PlayerModel.play()` -> sets `isActive` -> SwiftUI shows `PlayerScreen` -> `VideoPlayerView` creates `PlayerOpenGLView` -> `prepareOpenGL` fires -> `reel_player_init_render` creates mpv render context -> `onRenderContextReady()` -> `reel_player_load_file` -> mpv decodes -> update callback fires on mpv thread -> `DispatchQueue.main.async` queues redraw -> CVDisplayLink tick -> `reel_player_render(fbo, w, h)` -> frame displayed
- **Error propagation**: `reel_player_init_render` returns error code -> `renderReady = false` -> draw() skips rendering -> black screen (graceful degradation). `reel_player_load_file` returns error -> `PlayerModel` surfaces error alert -> player dismissed
- **State lifecycle risks**: SwiftUI can destroy/recreate views during transitions. CVDisplayLink must be stopped in `viewWillMove(toWindow: nil)` to prevent rendering into a detached view. `PlayerModel` lives in the environment (not the view) so playback state survives view rebuilds.
- **API surface parity**: After this, macOS matches GTK4's playback capability. Both frontends use the same Zig core player via C ABI, with platform-specific rendering surfaces (GtkGLArea vs NSOpenGLView).

## Acceptance Criteria

- [x] Video renders correctly in the macOS SwiftUI player (not just audio)
- [x] Playback is smooth at display refresh rate (CVDisplayLink vsync)
- [x] Retina displays render at native resolution
- [x] All keyboard shortcuts functional: Space, Left/Right, Up/Down, F, M, S, A, Escape
- [x] Watch progress persists across app sessions
- [x] Resume from saved position works
- [x] End-of-file returns to library view
- [x] Playback errors shown to user
- [x] No crashes on player open/close cycles
- [x] Plex streaming URLs play correctly (mpv handles HTTP natively)

## Dependencies & Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| NSOpenGLView lifecycle issues inside SwiftUI | Medium | High | Prototype early. Use `viewWillMove(toWindow:)` to detect attachment. Fallback: host in a child NSWindow overlay |
| `epoxy_get_proc_address` not available from Swift | Low | Medium | Fallback: `dlsym(RTLD_DEFAULT, name)` which works for system OpenGL |
| OpenGL removed in future macOS | Low (deprecated != removed) | High | This is a pragmatic first step. Metal/MoltenVK migration planned separately |
| CVDisplayLink use-after-free during SwiftUI view transitions | Medium | High | Use `Unmanaged.passRetained` + explicit release in `deinit`. Stop link in `viewWillMove(toWindow: nil)` |
| Initialization race: file loads before render context ready | High | High | Explicit two-phase init: `isActive` shows view, view's `prepareOpenGL` calls back to `onRenderContextReady`, which then loads the file |

## Out of Scope

- Metal/MoltenVK migration (future plan)
- Picture-in-Picture
- Auto-play next episode (future enhancement)
- Subtitle selection UI (cycle shortcut only for now)
- Audio track selection UI (cycle shortcut only for now)
- AirPlay integration

## Sources & References

### Origin

- **Brainstorm document:** [docs/brainstorms/2026-03-15-macos-swiftui-rewrite-brainstorm.md](docs/brainstorms/2026-03-15-macos-swiftui-rewrite-brainstorm.md) — Key decisions: NSViewRepresentable bridge for mpv, full window takeover, Liquid Glass controls overlay
- **Brainstorm document:** [docs/brainstorms/2026-03-14-reel-media-center-brainstorm.md](docs/brainstorms/2026-03-14-reel-media-center-brainstorm.md) — Architecture: Zig core + native frontends, libmpv as media backend, frontend-owned render surface
- **Parent plan Phase 5:** [docs/plans/2026-03-15-001-feat-macos-swiftui-cinematic-rewrite-plan.md](docs/plans/2026-03-15-001-feat-macos-swiftui-cinematic-rewrite-plan.md) — Video Player Integration phase

### Internal References

- Zig mpv player: `src/core/player.zig` — `initRender()`, `render()`, `setRenderUpdateCallback()`
- C ABI exports: `src/lib.zig:44-192` — PlayerWrapper, existing export functions
- C ABI header: `include/reel.h:36-62` — current player prototypes (missing render)
- GTK rendering (reference): `src/apprt/gtk/video_area.zig` — working OpenGL render integration
- macOS player model: `macos/Reel/Sources/Models/PlayerModel.swift` — current state polling
- macOS player view: `macos/Reel/Sources/Views/PlayerView.swift` — PlayerNSView (black), PlayerScreen (controls)
- Legacy OpenGL view: `macos/Reel/Sources/VideoView.swift` — NSOpenGLView patterns to carry forward
- Package config: `macos/Package.swift` — links mpv, epoxy, sqlite3

### External References

- [mpv render API docs](https://mpv.io/manual/master/#embedding-into-other-programs-(libmpv))
- [Apple NSOpenGLView](https://developer.apple.com/documentation/appkit/nsopenglview)
- [Apple CVDisplayLink](https://developer.apple.com/documentation/corevideo/cvdisplaylink)
- [libepoxy](https://github.com/anholt/libepoxy)
