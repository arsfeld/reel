# Backend Selection Regression Checklist

This checklist verifies that switching the playback backend between MPV and GStreamer works as expected on October 21, 2025.

## Linux (default build with MPV + GStreamer)
- [ ] Launch Reel and open any video (should use the default MPV backend).
- [ ] Open Preferences → Player and switch *Default Player Backend* to `GStreamer`.
- [ ] Confirm the player shows a toast announcing the backend switch and the stream restarts using GStreamer (check logs or the diagnostic overlay).
- [ ] Switch back to `MPV` and validate that playback restarts with MPV (toast appears, MPV-only features such as upscaling are available).
- [ ] Close and relaunch Reel; verify the selected backend persists and loads immediately on first playback.

## macOS (build with `--no-default-features --features gstreamer`)
- [ ] Launch Reel and start a video; confirm the preferences subtitle explains that GStreamer is required on macOS.
- [ ] Attempt to change the backend (only `GStreamer` is available) and confirm no unexpected options appear.
- [ ] Restart the app to ensure the persisted value remains `GStreamer`.

Log the runs in `docs/test-runs.md` if applicable.
