---
id: task-468.05
title: Review and simplify StreamManager complexity
status: To Do
assignee: []
created_date: '2025-11-22 21:17'
updated_date: '2025-11-22 21:56'
labels: []
dependencies: []
parent_task_id: task-468
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The StreamManager maintains parallel state tracking that might be redundant. Heavy use of try_lock() suggests potential deadlock concerns, and the default track workaround indicates timing issues with stream collection messages. The implementation should be reviewed for potential simplifications.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Lock acquisition patterns analyzed for deadlock potential
- [ ] #2 Redundant state tracking identified and removed where possible
- [ ] #3 try_lock usage minimized or justified
- [ ] #4 Stream collection timing issues investigated and documented
- [ ] #5 Code complexity reduced without losing functionality
<!-- AC:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Deep Analysis: Critical Issues Found

The initial analysis concluded the StreamManager is "sound and well-designed" with only minor documentation improvements needed. **This conclusion is fundamentally flawed.** The implementation has serious concurrency bugs, data integrity issues, and architectural problems.

---

## ❌ **Critical Issue #1: Silent Data Loss from try_lock() Failures**

**Location:** `stream_manager.rs:72-74`, `150-155`

**The Problem:**
```rust
if let Ok(mut guard) = self.stream_collection.try_lock() {
    *guard = Some(collection.clone());
}
// If try_lock() fails, collection is SILENTLY DROPPED
```

**Impact:**
- StreamCollection message arrives from GStreamer **ONCE per media load**
- If lock is held by UI thread, try_lock() fails
- Collection is **permanently lost** - no retry, no error
- UI shows stale/wrong track data
- Track switching breaks or uses invalid stream IDs
- User sees phantom tracks or crashes

**Why "next message will retry" is FALSE:**
GStreamer sends StreamCollection **once** when media loads. There is no automatic retry. This is **data corruption**, not a logging issue.

---

## ❌ **Critical Issue #2: try_lock() is a Band-Aid Hiding the Real Problem**

**The Real Problem:** Long critical sections in UI-facing code

**Location:** `stream_manager.rs:282-320` - `get_audio_tracks()`
```rust
let audio_streams = self.audio_streams.lock().unwrap();  // Lock acquired

for stream in audio_streams.iter() {  // Loop while holding lock
    let track_name = if let Some(ref lang) = stream.language {
        format!("Audio Track {} ({})", stream.index + 1, lang)  // String allocation!
    } else if let Some(ref codec) = stream.codec {
        format!("Audio Track {} [{}]", stream.index + 1, codec)  // More allocations!
    } else {
        format!("Audio Track {}", stream.index + 1)
    };
    tracks.push((stream.index, track_name));
}
// Lock held through ENTIRE loop + all string allocations!
```

**Why This Matters:**
- Bus handler runs on glib main loop thread
- UI calls `get_audio_tracks()` from async Relm4 components (same thread)
- Lock held during string formatting causes bus handler to fail acquiring lock
- try_lock() failures cascade

**Proper Fix:**
```rust
let streams = self.audio_streams.lock().unwrap().clone();  // Clone inside lock
// Lock automatically dropped here
// Do expensive work outside lock
for stream in streams.iter() {
    // Format strings without holding lock
}
```

**OR:** Use `RwLock` for read-heavy operations (multiple readers, single writer pattern).

---

## ❌ **Critical Issue #3: Default Track Workaround Creates UX Bugs**

**Location:** `stream_manager.rs:300-318`

**The Code:**
```rust
if tracks.is_empty() {
    if let Some(pb) = playbin {
        // Provide fake track if no collection received yet
        tracks.push((0, "Audio Track 1".to_string()));
    }
}
```

**Problems:**
1. UI shows phantom "Audio Track 1" before real data arrives
2. User selections based on fake data become invalid when real tracks load
3. Index 0 might map to different track than user expected
4. Creates confusing UI flash: "Audio Track 1" → "Audio Track 1 (English), Audio Track 2 (Spanish)"
5. Hides the actual problem: race condition in startup sequence

**Better Solutions:**
1. **Synchronization:** Block UI track selection until StreamCollection arrives
2. **Explicit loading state:** Return `None` and show "Loading tracks..." in UI
3. **Proper async/await:** Wait for StreamCollection with timeout using glib signals
4. **State machine:** Track initialization state properly (Uninitialized → Loading → Ready → Error)

**Root Cause:**
StreamCollection timing is non-deterministic, but GStreamer's AsyncDone typically follows it. The proper fix is to **wait for both** before allowing playback controls.

---

## ❌ **Issue #4: State Duplication Causes Desynchronization**

**Current State Storage:**
```rust
stream_collection: Arc<Mutex<Option<gst::StreamCollection>>>     // Source of truth
audio_streams: Arc<Mutex<Vec<StreamInfo>>>                       // DERIVED from collection
subtitle_streams: Arc<Mutex<Vec<StreamInfo>>>                    // DERIVED from collection
current_audio_stream: Arc<Mutex<Option<String>>>                 // "Current" tracking
current_subtitle_stream: Arc<Mutex<Option<String>>>              // "Current" tracking
```

**The Problem:**
1. `audio_streams`/`subtitle_streams` are **parsed derivatives** of `stream_collection`
2. 5 separate locks must be updated atomically but aren't
3. When try_lock() fails on lines 150-155, parsed streams don't update
4. State becomes **internally inconsistent**: collection updated, parsed streams stale
5. "Current" tracking can desync from GStreamer's actual active streams

**Evidence of Desync Risk:**
- `process_stream_collection_sync()`: Updates collection at line 72-74, then audio/subtitle at 150-155
- If second try_lock() fails, collection is stored but streams aren't parsed
- Subsequent `get_audio_tracks()` returns stale data from old collection

**Better Architectures:**

**Option A: Single Lock with Atomic Updates**
```rust
struct StreamManager {
    state: Arc<RwLock<StreamState>>,
}

struct StreamState {
    collection: Option<gst::StreamCollection>,
    audio_streams: Vec<StreamInfo>,      // Updated atomically with collection
    subtitle_streams: Vec<StreamInfo>,   // Updated atomically with collection
    current_audio: Option<String>,
    current_subtitle: Option<String>,
}
```

**Option B: Derive on Read (No Caching)**
```rust
struct StreamManager {
    collection: Arc<Mutex<Option<gst::StreamCollection>>>,
    current_audio: Arc<Mutex<Option<String>>>,
    current_subtitle: Arc<Mutex<Option<String>>>,
}

fn get_audio_tracks(&self) -> Vec<(i32, String)> {
    let collection = self.collection.lock().unwrap();
    // Parse on-demand - always fresh, no desync possible
    parse_audio_streams(&collection)
}
```

**Option C: Query GStreamer Directly (No State)**
```rust
fn get_audio_tracks(&self, playbin: &gst::Element) -> Vec<(i32, String)> {
    let n_audio = playbin.property::<i32>("n-audio");
    (0..n_audio).map(|i| {
        playbin.emit_by_name("get-audio-tags", &[&i])
    }).collect()
}
// No caching, no desync, always accurate
```

---

## ❌ **Issue #5: Architecture Needs Fundamental Redesign**

**Current Issues:**
1. **5 separate locks** that must maintain invariants but can desync
2. **Optimistic concurrency** (try_lock) that silently fails and loses data
3. **Shared mutable state** between glib main loop thread and async Rust tasks
4. **No message ordering guarantees** between bus handler updates and UI reads
5. **Lock contention** between bus handler and UI operations on same thread

**Why "Current Complexity is NECESSARY" is Wrong:**

The complexity exists because of poor architectural choices, not inherent problem complexity. GStreamer stream management is well-defined and doesn't require 5 separate locks.

**Better Architecture: Message Passing (Actor Pattern)**
```rust
enum StreamCommand {
    UpdateCollection(gst::StreamCollection),
    SetAudioTrack(i32),
    SetSubtitleTrack(i32),
    QueryTracks(oneshot::Sender<TrackInfo>),
}

struct StreamManager {
    command_tx: mpsc::Sender<StreamCommand>,
    state_rx: watch::Receiver<StreamState>,  // UI reads current state
}

// Separate actor task processes commands sequentially
// - No locks needed (single-threaded actor)
// - No races (sequential processing)
// - Clear ordering (command queue)
// - No deadlocks (no lock acquisition)
```

**Benefits:**
- Bus handler sends UpdateCollection command (never blocks)
- UI sends QueryTracks command (async response)
- Actor processes sequentially on dedicated thread
- State updates are atomic and ordered
- No try_lock(), no silent failures

---

## 📊 **Acceptance Criteria Re-evaluation**

| Criterion | Original Conclusion | Reality |
|-----------|---------------------|---------|
| #1 Lock deadlock analysis | ✅ "Low risk, try_lock prevents it" | ❌ try_lock causes data loss instead |
| #2 Redundant state | ✅ "No redundancy" | ❌ Derived state causes desync bugs |
| #3 try_lock justification | ✅ "Required for bus handler" | ❌ Band-aid hiding long critical sections |
| #4 Timing issues | ✅ "Keep workaround" | ❌ Fake data masks race conditions |
| #5 Reduce complexity | ✅ "Appropriate, no changes" | ❌ Architecture fundamentally flawed |

---

## 🎯 **Required Changes**

### Immediate (Fix Data Loss):
1. Replace all `try_lock()` with proper `lock()` calls
2. Use clone-before-release pattern in all getters
3. Minimize critical section duration (move string formatting outside locks)

### Short-term (Improve Performance):
4. Replace `Mutex` with `RwLock` for read-heavy collections
5. Remove default track workaround, return explicit loading state
6. Add proper initialization state machine

### Medium-term (Fix Architecture):
7. Consolidate to single `RwLock<StreamState>` for atomic updates
8. **OR** implement message-passing actor pattern
9. **OR** query GStreamer directly without caching

### Long-term (Investigate):
10. Test querying playbin3 properties/tags directly vs caching
11. Benchmark lock contention vs direct queries
12. Consider using GStreamer's native stream selection API more directly

---

## 🚨 **Conclusion**

The StreamManager implementation has **critical bugs** that cause:
- Silent data loss (try_lock failures)
- State desynchronization (parallel locks)
- UX confusion (phantom tracks)
- Potential crashes (invalid stream IDs)

This is not "sound and well-designed." It requires **architectural changes**, not just logging improvements.

**The task must be reopened and properly addressed.**
<!-- SECTION:NOTES:END -->
