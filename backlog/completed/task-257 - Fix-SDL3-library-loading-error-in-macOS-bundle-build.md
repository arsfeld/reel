---
id: task-257
title: Fix SDL3 library loading error in macOS bundle build
status: Done
assignee:
  - '@claude'
created_date: '2025-09-26 17:30'
updated_date: '2025-10-02 14:57'
labels:
  - macos
  - build
  - nix
dependencies: []
---

## Description

The build-macos-bundle command from flake.nix fails with 'Failed loading SDL3 library' error. This prevents successful creation of the macOS app bundle.

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Diagnose why SDL3 library fails to load during bundle build
- [x] #2 Fix SDL3 library path or dependency configuration in flake.nix
- [x] #3 Ensure build-macos-bundle command completes successfully
- [x] #4 Verify the generated macOS app bundle works correctly
<!-- AC:END -->


## Implementation Plan

1. Identify why MPV tries to load SDL3 at runtime
2. Modify launcher script to suppress SDL3 loading or provide alternative
3. Set MPV to use native macOS audio/video outputs instead of SDL
4. Test the bundled app to verify it works


## Implementation Notes

Fixed SDL3 loading error in macOS bundle by:

1. Identified that MPV was trying to load SDL for audio/video output
2. Modified launcher script to prevent SDL loading by setting MPV_ENABLE_SDL=no
3. Configured MPV to use native macOS drivers (libmpv for video, coreaudio for audio)
4. Removed SDL bundling code from build script since SDL is not needed
5. Removed SDL2 from macOS bundle tools in flake.nix

The app now uses native macOS drivers instead of SDL, which resolves the loading error.

Update: The initial fix was incorrect. The actual issue is that libavdevice (part of FFmpeg used by MPV) has a hard dependency on SDL2.

Corrected solution:
1. Restored SDL2 to macOS bundle tools in flake.nix
2. Added proper SDL2 bundling logic to copy and fix library paths
3. Configured SDL2 environment variables properly in launcher
4. SDL2 is bundled for libavdevice compatibility but MPV still uses native drivers for playback
