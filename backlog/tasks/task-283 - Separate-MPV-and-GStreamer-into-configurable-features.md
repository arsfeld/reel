---
id: task-283
title: Separate MPV and GStreamer into configurable features
status: Done
assignee:
  - '@claude'
created_date: '2025-09-27 19:52'
updated_date: '2025-09-27 20:17'
labels:
  - backend
  - player
  - platform
dependencies: []
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Refactor the player backends to use Rust feature flags, allowing each to be enabled/disabled at compile time. This will allow macOS builds to disable MPV (which has issues on that platform) while Linux can enable both.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 MPV backend is behind an 'mpv' feature flag in Cargo.toml
- [x] #2 GStreamer backend is behind a 'gstreamer' feature flag in Cargo.toml
- [x] #3 Player factory correctly handles available backends based on enabled features
- [x] #4 macOS configuration disables MPV feature by default
- [x] #5 Linux configuration enables both features by default
- [x] #6 Code compiles successfully with only MPV enabled
- [x] #7 Code compiles successfully with only GStreamer enabled
- [x] #8 Code compiles successfully with both backends enabled
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add feature flags to Cargo.toml for both mpv and gstreamer
2. Add conditional compilation to player/mod.rs exports
3. Refactor player/factory.rs to handle missing backends at compile time
4. Add compile-time checks in imports throughout the codebase
5. Test different feature combinations
6. Update nix packages configuration for platform-specific defaults
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Refactored player backends to use Rust feature flags for conditional compilation:

1. Added feature flags in Cargo.toml:
   - "mpv" feature for MPV backend with libmpv2 dependencies
   - "gstreamer" feature for GStreamer backend with gstreamer dependencies
   - Default features include both backends

2. Moved UpscalingMode to a common types module to minimize conditional compilation

3. Updated player factory to handle conditional compilation gracefully:
   - Backends only compiled when their feature is enabled
   - Fallback logic when primary backend fails
   - Clear error messages when features are disabled

4. Minimized #[cfg(feature)] annotations by keeping code abstract:
   - UpscalingMode always available (returns error for non-MPV backends)
   - UI code checks is_mpv_backend flag instead of compile-time checks
   - Controller methods available but return appropriate errors

5. Updated Nix packages configuration:
   - macOS builds use only GStreamer (MPV disabled due to platform issues)
   - Linux builds use both backends
   - Configuration in nix/packages.nix with buildFeatures

6. All three configurations tested and compile successfully:
   - MPV-only: cargo check --no-default-features --features mpv
   - GStreamer-only: cargo check --no-default-features --features gstreamer
   - Both (default): cargo check

This approach provides flexibility for different platforms while keeping the codebase clean and maintainable.
<!-- SECTION:NOTES:END -->
