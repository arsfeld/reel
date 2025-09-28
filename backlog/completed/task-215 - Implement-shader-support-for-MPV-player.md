---
id: task-215
title: Implement shader support for MPV player
status: Done
assignee:
  - '@claude'
created_date: '2025-09-22 15:23'
updated_date: '2025-09-22 17:49'
labels:
  - player
  - mpv
  - enhancement
dependencies: []
priority: medium
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Add support for embedding and applying custom shaders in the MPV player backend. Shaders should be compiled into the application binary to ensure they're always available, rather than loaded from external files. This will enable video enhancement features like upscaling, color correction, and other visual effects.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 MPV player correctly applies loaded shaders to video playback
- [x] #2 Shader loading errors are handled gracefully with fallback to default rendering
- [x] #3 Multiple shaders can be chained/combined if supported
- [x] #4 Shader configuration can be toggled on/off in player settings
- [x] #5 Shaders are embedded into the application binary at compile time
<!-- AC:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Review current MPV player shader implementation
2. Create a build-time shader embedding system using include_str! macro
3. Modify apply_upscaling_settings to use embedded shaders
4. Create temporary files for MPV to load shaders from memory
5. Test shader application with existing upscaling modes
6. Add error handling for shader application failures
7. Ensure all existing upscaling modes work with embedded shaders
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented comprehensive shader embedding system for MPV player:

1. Created new shaders module (src/player/shaders.rs) that embeds all shader files at compile time using include_str! macro
2. Modified apply_upscaling_settings() to use embedded shaders instead of loading from disk
3. Implemented temporary file creation for MPV (which requires file paths) using system temp directory
4. Added proper error handling with fallback to built-in scalers when shader preparation fails
5. Added support for custom shader configurations via apply_custom_shaders() method
6. Exposed get_available_shaders() to list all embedded shaders
7. All existing upscaling modes (None, HighQuality, FSR, Anime) now use embedded shaders

Key improvements:
- Shaders are guaranteed to be available as they are compiled into the binary
- No runtime file system dependencies for shader loading
- Graceful fallback to built-in MPV scalers if shader application fails
- Support for chaining multiple shaders (e.g., Anime4K uses both Clamp and Upscale)
- Custom mode allows dynamic shader configuration at runtime
<!-- SECTION:NOTES:END -->
