---
title: "U1 feasibility spike findings: Plex HLS transcode through playbin3"
date: 2026-05-28
plan: docs/plans/2026-05-28-003-feat-server-side-encoding-plan.md
verdict: GO
server: "PMS 1.43.2.10687 (Linux), Plex Pass, relay connection (maxUploadBitrate=2000)"
gstreamer: "1.26.11 (hlsdemux2 + adaptivedemux2 present)"
test_file: "28 Years Later — ratingKey 136798, HEVC main 10 1080p mkv, single part 950874"
---

# U1 — Go/No-Go on Plex server-side transcode through playbin3

**Verdict: GO.** A deliberately-incompatible HEVC main-10 file transcodes to HLS and
plays *and* seeks through `playbin3` with fakesinks. All load-bearing assumptions are
resolved; two new PFK traps were discovered that revise KTD3.

Validated against the app's own logged-in server (decision/start/stop probed with
`curl`, timeline measured with `ffprobe`, playback measured with a throwaway
`playbin3` + fakesink Rust spike — since deleted).

## Per-item results (a)–(k)

| # | Item | Result |
|---|------|--------|
| a | playbin3 plays a live transcode | **YES** — no bus error, decodes h264/mp3 mpegts segments |
| b | `duration_us>0` + prepared fire | **YES** — `query_duration` = 6907s (full content), AsyncDone fires |
| c | **Timeline origin** | **Resolved — see below.** Raw segments carry *absolute* PTS; `playbin3` *normalizes position to 0*. `resume_secs=0` for transcode is correct, **but** the player must add a `base_offset` for display. |
| d | `offset ≤ 12s` → 404 trap | **Did NOT reproduce** on 1.43.2 (offset=0 and offset=5 both probe fine, snap to ~10s first keyframe). Keep clamp-to-0 as cheap insurance. |
| e | GStreamer ≥1.22 + demux2 | **YES** — 1.26.11, `hlsdemux2`/`adaptivedemux2`/`playbin3` all present |
| f | **Reap timeout** | **~76s** un-pinged (alive at t+61s, gone by t+76s). → keepalive cadence ~15–20s (≤ ⅓ reap, KTD7). |
| g | Token propagates to segments | **YES** — ffprobe consumed the playlist + segments with only the base-URL `X-Plex-Token` |
| h | Decode/direct-play table | Probed: source library is overwhelmingly HEVC main-10 (10-bit) — the canonical playbin3 break case. Concrete table for U4 below. |
| i | **Session lifecycle** | **Server honors a client-chosen `session=` id** — it appears verbatim as the `key` in `/transcode/sessions`. `/stop?session=ID` → 200 → gone. `/stop` on an already-reaped session → **404** (treat as success). KTD7 per-reload session scheme is viable. |
| j | Decision response shape | Recorded below. `Part@decision`/`Stream@decision` present; **`TranscodeSession` is null in the decision response** — sources res/bitrate from the selected `Media`. |
| k | Fallback protocol | `protocol=http` (MP4) endpoint reachable; HLS works, so not on the critical path. |

## Two new PFK traps (revise KTD3)

KTD3 prescribed deriving the decision URL by string-replacing `/start.m3u8`→`/decision`
keeping the identical query *with `hasMDE=1`*. On PMS 1.43.2 **two query params cause a
generic HTTP 400** on the universal transcoder:

1. **`X-Plex-Client-Identifier` in the query string → 400.** It must be sent as an HTTP
   **header** (which `PlexClient` already does, `api.rs:22`) or omitted entirely. Never
   put the client id in the transcode/decision query string.
2. **`hasMDE=1` in the query → 400.** The decision endpoint works with the *same* query
   as `start.m3u8` **minus `hasMDE`**. Omit it.
3. **`X-Plex-Platform` is required AND value-validated** (found while validating U5).
   Omitting it 400s both `decision` and `start.m3u8`. The *value* is checked:
   `Generic` and `Chrome` are accepted, but **`Linux` is rejected with 400**. It must be
   a **query** param (GStreamer fetches `start.m3u8` + segments from the URL, not via the
   client's headers), so `PlexClient`'s header platform is irrelevant here. U5 sends
   `X-Plex-Platform=Generic` in the query.

Keepalive confirmed: `GET /video/:/transcode/universal/ping?session=<id>` returns 200 on a
live session (the U1 404 was just an absent session). `/stop` likewise 200 on a live
session, 404 once reaped (treat as success).

With those two omitted, the string-replace `start.m3u8`→`decision` approach (KTD3) works
and returns a parseable decision. A minimal working query (from `python-plexapi`):

```
/video/:/transcode/universal/start.m3u8
  ?path=%2Flibrary%2Fmetadata%2F136798
  &mediaIndex=0&partIndex=0&protocol=hls&fastSeek=1&copyts=1&offset=0
  &maxVideoBitrate=4000&videoResolution=1920x1080
  &X-Plex-Platform=Chrome&session=<client-id>&X-Plex-Token=<token>
```

`/decision` = the same query string with the endpoint segment swapped.

## KTD1 — timeline origin, resolved (the load-bearing one)

- `ffprobe` on the raw HLS (offset=600) reports `start_time≈610s` (**absolute** content
  time), `duration=6907s` (full). `copyts` does **not** change this — both 0 and 1 give
  absolute segment PTS.
- **But `playbin3` normalizes the presentation: first `query_position` = 0s**, counting up
  from 0, while `query_duration` = full content length (6907s).

Consequence for the implementation:

- **Resume:** after a transcode reload built with `offset=P`, `playbin3` position starts
  at **0**, so `SetUrl.resume_secs = 0` is correct (no client re-seek). KTD1's conclusion
  holds. Direct-play is unchanged (`resume_secs = position`).
- **NEW requirement (not in the plan):** because position is 0-based but duration is the
  *full* content length, the player must track `transcode_base_offset = P` and compute
  **displayed/seek-bar/watch-progress position = base_offset + raw_position**. Otherwise a
  resume-at-10min shows 0:00 on the seek bar and reports the wrong scrobble position.
  This affects U7 (decision→SetUrl), U8 (switch/seek), and U10/watch-progress reporting.

## KTD2 — seek-during-transcode, refined

- In-pipeline seek **within the transcoder's ahead-buffer works** (proved: 4s→34s
  requested, landed 36s, `seek_simple` ok). `TranscoderThrottleBuffer=60`.
- Seeks beyond the buffer (or backward before the build offset) still need an
  offset-reload. **v1 should keep KTD2's reload-on-seek** for correctness/simplicity;
  in-buffer in-pipeline seek is a future optimization.

## Decision response shape (for U3 models)

Forced/auto transcode of the HEVC file (h264-only-ish target) returned:

```
MediaContainer:
  generalDecisionCode=1001  generalDecisionText="Direct play not available; Conversion OK."
  directPlayDecisionCode=3000 directPlayDecisionText="...No direct play video profile exists for protocol http, with container mkv, and video codec hevc."
  transcodeDecisionCode=1001
  Metadata[0].Media[0]: selected=true videoCodec=h264 audioCodec=mp3 container=mpegts protocol=hls videoResolution=720p bitrate=1877
    Part[0].decision="transcode" container=mpegts
      Stream(type=1).decision="transcode" codec=h264 location="segments-av"
      Stream(type=2).decision="transcode" codec=mp3
  TranscodeSession: (absent in decision response — only materializes from start.m3u8)
```

- Parse `Part@decision` / `Stream@decision` ∈ {`directplay`,`copy`,`transcode`}.
- **Indicator (R16) res/bitrate must come from the selected `Media`** (`videoResolution`,
  `bitrate`, output `videoCodec`), NOT from `TranscodeSession` (null here).
- A live `/transcode/sessions` entry carries: `key` (= client session id),
  `videoDecision`, `audioDecision`, `protocol`, `throttled`, `complete`, `progress`,
  `duration`, `transcodeHwEncoding` (= `vaapi` here).

## U4 direct-play table seed (h)

Source library distribution (Movies, n=157): predominantly `hevc main 10` (1080/4k/480),
plus `h264 high`, some `av1 main`. The machine's `playbin3` decodes h264 fine; HEVC main-10
and HDR are the break cases. U4's `add-direct-play-profile` set should declare h264
(mp4/mkv + aac/ac3/eac3) as direct-play and **exclude HEVC-10bit/HDR/DV** (KTD12) so they
route to transcode-to-SDR. (Full `gst-inspect` decoder enumeration to be encoded as U4's
single-source-of-truth table.)
