---
id: task-454
title: Complete flatpak manifest with yuki-iptv libmpv module configuration
status: In Progress
assignee: []
created_date: '2025-10-23 02:29'
updated_date: '2025-10-23 02:31'
labels:
  - flatpak
  - build
  - mpv
  - dependencies
dependencies: []
priority: high
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The flatpak build is failing because libmpv cannot be found during linking. The current manifest has a partial libmpv configuration that needs to be replaced with the complete proven configuration from the yuki-iptv flatpak.

**Current state:**
- Removed org.freedesktop.Platform.ffmpeg-full extension (line 7)
- Already have: libplacebo (with glslang), libass, uchardet, partial libmpv with luajit
- File: dev.arsfeld.Reel.json

**What needs to be added:**
The libmpv module needs these additional sibling modules (from https://github.com/flathub/io.github.yuki_iptv.yuki-iptv/blob/master/libmpv.yml):

1. **libXpresent** - X11 presentation library
2. **libv4l2** - Video4Linux2 library  
3. **nv-codec-headers** - NVIDIA codec headers
4. **x264** - H.264 encoder (uses git source)
5. **ffmpeg** - Complete FFmpeg with all codecs
   - Nested module: **libjxl** (JPEG XL support)
     - Nested module: **highway** (SIMD library)

These should be inserted between the current libmpv module and the reel module.

**Reference:**
The complete YAML structure was fetched and is in the conversation. Convert the YAML modules structure to JSON and insert into dev.arsfeld.Reel.json.

**Testing:**
After updating manifest:
```bash
rm -rf build-dir .flatpak-builder repo
./scripts/build-flatpak.sh
```

The build should complete successfully with libmpv.so installed to /app/lib/
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Flatpak build completes without linker errors
- [ ] #2 libmpv library is found and linked successfully
- [ ] #3 Flatpak bundle is created: reel-*.flatpak
- [ ] #4 All modules from yuki-iptv libmpv.yml are included in manifest
<!-- AC:END -->
