# Player Modernization Plan (VM‑Only, No Feature Flag)

This document captures the finalized plan to modernize the Player page by progressively moving functionality from `src/ui/pages/player.rs` into `PlayerViewModel`, without adding new services and without feature flags. It preserves current behavior while aligning with the app’s reactive, SeaORM-backed architecture and full ID policy.

## Objectives
- Centralize player state and data flow in `PlayerViewModel`.
- Keep `PlayerPage` primarily a UI layer binding to the ViewModel.
- Preserve all current features and UX behaviors during incremental migration.
- Respect full cache‑key IDs end‑to‑end (no truncation).

## Scope and Constraints
- No new service layer: all logic moves into `PlayerViewModel` (VM-only approach).
- No feature flag: migrations land as incremental, safe refactors that keep parity at each step.
- Existing player backends (MPV, GStreamer) remain unchanged.

## Architecture Overview
- `PlayerViewModel` becomes the single source of truth for:
  - Current media item, stream info, markers (intro/credits), next episode
  - Playback state, position/duration, errors, loading, auto‑play state
  - Progress persistence and event emission
- `PlayerPage` focuses on:
  - Rendering video widget via `Player` (MPV/GStreamer)
  - OSD controls, overlays, keyboard/gestures
  - Subscribing to VM properties and reflecting changes in the UI
- `MainWindow::show_player` remains the entry point but delegates OSD/back ownership to PlayerPage and ensures header visibility is correctly toggled/restored.

## Incremental Phases

### Phase 1: Strengthen PlayerViewModel
- Inject dependencies: `Arc<AppState>` (or minimally `SourceCoordinator`) alongside existing `Arc<DataService>` and `EventBus`.
- Add properties:
  - `current_media: Option<MediaItem>`
  - `is_loading: bool`
  - `error: Option<String>`
  - `stream_info: Option<StreamInfo>`
  - `markers: (Option<ChapterMarker>, Option<ChapterMarker>)`
  - `next_episode: Option<Episode>`
  - `auto_play_state: enum { Idle, Counting(u32), Disabled }`
  - Reuse existing: `playback_state, position, duration, volume, is_muted, playback_rate, is_fullscreen, playlist, playlist_index`
- Add methods:
  - `set_media_item(MediaItem)`
  - `load_stream_and_metadata()` (fetch stream URL and markers via backend; set `is_loading`/`error`/`stream_info`/`markers`)
  - `find_next_episode()` (for episodes)
  - `save_progress_throttled(media_id, position, duration)` (DB updates and watched threshold; debounce)
  - Reuse existing playback event emission via EventBus
- Event handling: continue handling `MediaUpdated/MediaDeleted` with full cache‑key IDs.

### Phase 2: Delegate Data Loading to VM
- Update `PlayerPage::load_media` to:
  - Call `vm.set_media_item(media_item.clone())`
  - Await `vm.load_stream_and_metadata()` while showing loading overlay
  - On `stream_info` change: create video widget, `Player.load_media(url)`, resume position, start playback
  - On `error` change: show error overlay with friendly messaging; keep Go Back
- Keep markers UI logic in page but source marker data from `vm.markers`.

### Phase 3: Move Progress Persistence to VM
- Keep a sampling timer in PlayerPage for `position`/`duration` reads from `Player`.
- Replace direct DB/backend writes with `vm.save_progress_throttled(id, position, duration)`.
- On pause/stop, flush once more via VM.

### Phase 4: Auto‑Play and Next Episode via VM
- VM resolves `next_episode` when credits marker is approached or on “Skip credits”.
- VM exposes `next_episode` and `auto_play_state`.
- PlayerPage displays PiP overlay and countdown based on VM props.
- “Play Now” uses `vm.next_episode` to trigger navigation and playback; “Cancel” sets `auto_play_state = Disabled`.

### Phase 5: Tracks and Quality (Light VM Hooks)
- Keep menus/actions in PlayerPage for now to reduce risk.
- VM hooks:
  - `on_tracks_discovered(audio_tracks, subtitle_tracks)` (store/persist user pref)
  - `on_quality_options_discovered(options)` (optional)
  - `select_audio_track(index)`, `select_subtitle_track(index)`, `select_quality(index)` update VM prefs; PlayerPage executes on `Player`, re‑seeks, resumes; VM emits event and persists preference.

### Phase 6: Cleanup and Lifecycle
- Add `ViewModel::dispose()` to cancel background tasks, countdowns, and subscriptions.
- `PlayerPage::stop()` calls `vm.dispose()` and cancels all timers/controllers; releases inhibit cookie.
- Ensure `MainWindow` restores header/toolbar style after leaving the player route.

### Phase 7: Polish and Verification
- Confirm full cache‑key ID usage throughout.
- Centralize user‑friendly error mapping for stream fetch in VM.
- Logging: concise, include `backend_id` and `media_id` context; limit noise during playback.
- Persist volume, playback rate, and track preferences via VM into Config.

## Detailed VM API Additions

In `src/ui/viewmodels/player_view_model.rs`:

- New properties (with `Property<T>`):
  - `current_media: Option<MediaItem>`
  - `is_loading: bool`
  - `error: Option<String>`
  - `stream_info: Option<StreamInfo>`
  - `markers: (Option<ChapterMarker>, Option<ChapterMarker>)`
  - `next_episode: Option<Episode>`
  - `auto_play_state: AutoPlayState { Idle, Counting(u32), Disabled }`
- New public methods:
  - `fn set_media_item(&self, media: MediaItem)`
  - `async fn load_stream_and_metadata(&self) -> Result<()>`
  - `async fn find_next_episode(&self) -> Result<Option<Episode>>`
  - `async fn save_progress_throttled(&self, id: &str, position: Duration, duration: Duration)`
  - `async fn select_audio_track(&self, index: i32)`
  - `async fn select_subtitle_track(&self, index: i32)`
  - `async fn select_quality(&self, index: usize)`
  - `fn dispose(&self)`
- Internal behavior:
  - Resolve backend from `current_media.backend_id()` using `AppState.source_coordinator`.
  - Fetch stream URL and markers; set properties and errors accordingly.
  - Debounce progress persistence (e.g., 2–5s) and mark watched at ~90%.
  - Emit playback events via EventBus (`PlaybackStarted/Paused/Stopped/Completed/PositionUpdated`).

## PlayerPage Changes (Bindings and Delegation)

In `src/ui/pages/player.rs`:
- `load_media()`
  - Use VM for setting media, loading stream info + markers
  - Subscribe to `is_loading`, `error`, `stream_info`, `markers`, `playback_state`, `position`, `duration`
  - Once `stream_info` is ready, create the video widget, `Player.load_media(url)`, resume, start playback, show controls
- Position/progress timers
  - Sample `Player.get_position()/get_duration()` as today; call `vm.save_progress_throttled(...)`
- Markers
  - Use `vm.markers` for showing/hiding skip buttons based on current position; keep timer logic local for simplicity
- Auto‑play
  - Display VM’s `next_episode` and `auto_play_state` in overlay; buttons call into page functions that consult VM state and navigate/play next
- Lifecycle
  - On stop/exit, call `vm.dispose()`; cancel timers/controllers and release inhibit cookie

## MainWindow Adjustments
- Keep `show_player` creating a new `PlayerPage` and hiding the header; avoid adding duplicate OSD/back buttons if PlayerPage already manages them.
- Ensure header/toolbar style is restored on exit (via PlayerPage cleanup or navigation hook).

## Risks and Mitigations
- UI flicker: Drive overlays via `is_loading`; only add video widget once `stream_info` is available.
- Event storms: Leverage improved `PropertySubscriber`; VM debounces saves and property changes.
- Resource leaks: `dispose()` + `PlayerPage::stop()` remove timers, controllers, and inhibit guards.
- Tracks/quality gaps: Keep current PlayerPage logic until VM hooks are proven; then iterate.

## QA Plan (Manual)
- Movie playback: start, pause, seek, resume, exit; verify DB/backend progress and watched state.
- Episode playback: intro/credits skip, auto‑play PiP, cancel auto‑play; next episode resolution.
- Track/quality: menus populate; switches re‑seek and resume.
- Failure modes: 401/404/timeout → friendly messages; Go Back works.
- Fullscreen/inhibit: toggle via key and double‑click; release on stop.
- Navigation: back from player restores header; repeated open/close stable.

## Acceptance Criteria
- Playback works identically (MPV/GStreamer), including resume and watched state.
- Skip intro/credits and auto‑play function using VM-provided metadata.
- Errors are user‑friendly; no duplicate OSD/back; header restored after exit.
- No leaked timers/controllers/inhibit across multiple sessions.
- Full IDs are used consistently.

---

# Player Modernization Checklist

Use this to track progress. Grouped by phase; each item should compile and run without regressions.

## Phase 1 – ViewModel Foundations
- [x] Inject `Arc<AppState>` (or `SourceCoordinator`) into `PlayerViewModel` constructor
- [x] Add properties: `current_media`, `is_loading`, `error`, `stream_info`, `markers`, `next_episode`, `auto_play_state`
- [x] Implement `set_media_item(MediaItem)`
- [x] Implement `load_stream_and_metadata()` (backend resolution, stream URL fetch, markers fetch, error mapping)
- [x] Implement `find_next_episode()` (episodes only)
- [x] Implement `save_progress_throttled(id, position, duration)` with watched threshold
- [x] Ensure `MediaUpdated/Deleted` handling uses full IDs

## Phase 2 – Page Delegation
- [x] Switch `PlayerPage::load_media()` to call VM for stream (markers remain in page temporarily)
- [x] Subscribe to `is_loading` and `error` and update overlays/UI
- [ ] Optionally subscribe to `stream_info`, `markers` (not required yet)
- [ ] On `stream_info`, create video widget, call `Player.load_media(url)`, resume position, start playback (still handled inline)
- [x] Keep markers UI, but source data from `vm.markers`

## Phase 3 – Progress Persistence
- [x] Replace direct DB/backend updates with `vm.save_progress_throttled(...)`
- [x] Flush on pause/stop
- [ ] Emit `Playback*` events from VM for position/stop/complete

## Phase 4 – Auto‑Play Next Episode
- [ ] VM computes `next_episode` and exposes `auto_play_state`
- [ ] PlayerPage renders PiP overlay and countdown from VM
- [ ] Wire “Play Now” to navigate and load `vm.next_episode`; “Cancel” disables auto‑play

## Phase 5 – Tracks and Quality Hooks
- [ ] Add VM hooks: `on_tracks_discovered`, `on_quality_options_discovered`
- [ ] Implement VM methods: `select_audio_track`, `select_subtitle_track`, `select_quality`
- [x] Keep actual player operations in PlayerPage (seek back + resume) while persisting prefs in VM (quality menu now refreshes from VM stream_info)

## Phase 6 – Cleanup and Lifecycle
- [ ] Add `ViewModel::dispose()` and cancel tasks/subscriptions
- [ ] Ensure `PlayerPage::stop()` calls `vm.dispose()`
- [ ] Cancel `glib::SourceId` timers (position sync, chapter monitor, countdown)
- [ ] Remove event controllers (hover, key, gestures) and release inhibit cookie on exit

## Phase 7 – Polish and Verification
- [ ] Full ID policy verified in VM and page
- [ ] Centralized error mapping for stream fetch in VM
- [ ] Concise logging with `backend_id` and `media_id` context
- [ ] Persist volume, playback rate, and track preferences via VM -> Config
- [ ] Manual QA scenarios pass (movies/episodes/quality/tracks/errors/fullscreen/navigation)

## Definition of Done
- [ ] All acceptance criteria met and validated
- [ ] No duplicate OSD/back controls; header state restored correctly
- [ ] No resource leaks after repeated player sessions
- [ ] Code and docs updated; comments localized to complex logic only

## File Touchpoints
- `src/ui/viewmodels/player_view_model.rs` (major additions)
- `src/ui/pages/player.rs` (delegate to VM, subscriptions, cleanup)
- `src/ui/main_window.rs` (header/OSD ownership sanity, restore on exit)

---

Refer to this document when implementing each phase to ensure minimal risk, consistent behavior, and alignment with the reactive architecture.
