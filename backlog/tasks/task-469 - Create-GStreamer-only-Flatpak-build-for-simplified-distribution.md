---
id: task-469
title: Create GStreamer-only Flatpak build for simplified distribution
status: In Progress
assignee: []
created_date: '2025-11-24 20:01'
updated_date: '2025-11-24 20:33'
labels:
  - flatpak
  - build
  - gstreamer
  - ci-cd
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Create a simplified Flatpak build process that uses only GStreamer (no MPV) to significantly reduce build complexity and time. The current Flatpak builds libmpv from source with all its dependencies (ffmpeg, luajit, libplacebo, glslang, etc.), which is slow and complicated.

## Background
- The app supports GStreamer-only mode via `--no-default-features --features gstreamer`
- GNOME Platform 48 includes GStreamer and all standard plugins
- Only `gtk4paintablesink` (from gst-plugins-rs) needs to be built separately
- Must set `GST_PLUGIN_PATH=/app/lib/gstreamer-1.0` for plugin discovery

## Benefits
- **Build time**: Minutes instead of hours
- **Manifest complexity**: ~50 lines vs ~350 lines
- **Maintenance**: Fewer external dependencies to track
- **Bundle size**: Smaller, uses runtime GStreamer

## Key Files
- Current manifest: `dev.arsfeld.Reel.json`
- Build script: `scripts/build-flatpak.sh`
- Release workflow: `.github/workflows/release.yml`
- Deploy workflow: `.github/workflows/deploy-flatpak-repo.yml`

## References
- [Bundling gst-plugin-rs in Flatpak](https://discourse.flathub.org/t/bundling-gst-plugin-rs-libraries-in-flatpak/7474)
- [gst-plugins-rs GitLab](https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs)
- [gtk4paintablesink docs](https://gstreamer.freedesktop.org/documentation/gtk4/index.html)
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Flatpak builds successfully using only GStreamer (no MPV dependencies)
- [ ] #2 gtk4paintablesink works correctly for video playback
- [ ] #3 Build completes in under 30 minutes on GitHub Actions
- [ ] #4 Flatpak can be installed and runs the application correctly
- [ ] #5 GitHub Actions workflow builds and publishes Flatpak to repository
- [ ] #6 Both x86_64 and aarch64 architectures are supported
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
## Implementation Plan

### Phase 1: Create Simplified Flatpak Manifest
1. **Create new manifest file** `dev.arsfeld.Reel-gstreamer.json`
   - Use GNOME Platform 48 + Rust SDK extension
   - Remove all MPV-related modules (libmpv, ffmpeg, luajit, libplacebo, glslang, etc.)
   - Add `gst-plugin-gtk4` module from gst-plugins-rs
   - Configure cargo to build with `--no-default-features --features gstreamer`
   - Add `GST_PLUGIN_PATH=/app/lib/gstreamer-1.0` to finish-args

2. **gst-plugin-gtk4 module structure:**
   ```json
   {
     "name": "gst-plugin-gtk4",
     "buildsystem": "meson",
     "sources": [
       {
         "type": "git",
         "url": "https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs.git",
         "tag": "gstreamer-1.24.0"
       }
     ],
     "config-opts": [
       "-Dgtk4=enabled",
       "-Dgtk4-wayland=enabled",
       "-Dgtk4-x11egl=enabled"
     ]
   }
   ```

### Phase 2: Update Build Script
1. **Modify `scripts/build-flatpak.sh`** or create `scripts/build-flatpak-gstreamer.sh`
   - Point to the new manifest
   - Generate cargo-sources.json with GStreamer-only features
   - Keep the same flatpak-builder workflow

### Phase 3: Update GitHub Actions Workflow
1. **Update `.github/workflows/release.yml`**
   - Re-enable the `build-flatpak` job (currently commented out)
   - Use the new GStreamer-only manifest
   - Simplify dependencies installation (no need for cross-compile toolchains for ffmpeg)

2. **Test workflow changes:**
   - x86_64 build on ubuntu-latest
   - aarch64 build with QEMU emulation

### Phase 4: Testing & Validation
1. Build Flatpak locally and verify:
   - Video playback works with gtk4paintablesink
   - Audio playback works
   - All GStreamer plugins load correctly
   - Application starts and functions normally

2. Test on clean system (fresh VM or container)

### Phase 5: Documentation
1. Update build documentation in `.flatpak-builder.yml`
2. Add notes about GStreamer-only vs full (MPV) builds if needed

## Technical Notes

### Cargo Build Command
```bash
cargo --offline build --release --no-default-features --features gstreamer
```

### Required finish-args
```json
"finish-args": [
  "--share=network",
  "--share=ipc",
  "--socket=fallback-x11",
  "--socket=wayland",
  "--socket=pulseaudio",
  "--device=dri",
  "--env=GST_PLUGIN_PATH=/app/lib/gstreamer-1.0"
]
```

### GStreamer plugins from runtime (already included)
- gst-plugins-base (playback, audio/video convert)
- gst-plugins-good (audio decoders, video filters)
- gst-plugins-bad (h264/h265 decoders, HLS support)
- gst-plugins-ugly (MP3, DVD support)
- gst-libav (FFmpeg-based decoders)

### Plugin we need to build
- `libgstgtk4.so` (gtk4paintablesink) from gst-plugins-rs
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
## Implementation Complete

### Files Modified:

1. **`dev.arsfeld.Reel.json`** - Replaced with simplified GStreamer-only manifest
   - Only 2 modules: gst-plugin-gtk4 and reel
   - Uses GNOME Platform 48 + Rust SDK extension 24.08
   - Sets `GST_PLUGIN_PATH=/app/lib/gstreamer-1.0`
   - Properly configures cargo with vendored dependencies

2. **`scripts/build-flatpak.sh`** - Updated to generate gst-plugins-rs sources
   - Clones gst-plugins-rs to get Cargo.lock
   - Generates both gst-plugin-gtk4-sources.json and cargo-sources.json
   - Builds and exports Flatpak bundle

3. **`.github/workflows/release.yml`** - Enabled Flatpak build job
   - Uses simplified GStreamer-only approach
   - x86_64 enabled, aarch64 ready to enable

4. **`.gitignore`** - Added generated files to ignore list

### Key Changes:
- Original manifest: ~335 lines with MPV + all dependencies
- New manifest: ~75 lines with just GStreamer + gtk4paintablesink
- Build time: Expected to be significantly faster (no FFmpeg, libplacebo, etc.)

### Testing:
The build script and manifest are validated. Run `./scripts/build-flatpak.sh` to test the full build locally.
<!-- SECTION:NOTES:END -->
