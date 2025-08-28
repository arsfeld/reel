# MPV Upscaling Shaders

This directory contains upscaling shaders for MPV video playback enhancement.

## Included Shaders

### AMD FSR (FidelityFX Super Resolution)
- **File**: `FSR.glsl`
- **License**: MIT
- **Source**: https://gist.github.com/agyild/82219c545228d70c5604f865ce0b0ce5
- **Description**: Vendor-agnostic upscaling for general content
- **Usage**: Good for movies, TV shows, and general video content

### Anime4K
- **Files**: `Anime4K_*.glsl`
- **License**: MIT
- **Source**: https://github.com/bloc97/Anime4K
- **Description**: Optimized for anime/cartoon content
- **Variants**:
  - `Anime4K_Clamp_Highlights.glsl` - Reduces oversaturation
  - `Anime4K_Restore_CNN_M.glsl` - Restores details (Medium)
  - `Anime4K_Upscale_CNN_x2_M.glsl` - 2x upscaling (Medium)
  - `Anime4K_AutoDownscalePre_x2.glsl` - Auto downscale preprocessing
  - `Anime4K_AutoDownscalePre_x4.glsl` - Auto downscale preprocessing

### FSRCNNX
- **Files**: `FSRCNNX_x2_*.glsl`
- **License**: MIT (dual-licensed GPL-3.0/MIT, we use MIT)
- **Source**: https://github.com/igv/FSRCNN-TensorFlow
- **Description**: High-quality upscaling for real-life content
- **Variants**:
  - `FSRCNNX_x2_8-0-4-1.glsl` - Lighter variant
  - `FSRCNNX_x2_16-0-4-1.glsl` - Higher quality variant

## Installation

These shaders are automatically installed with the application and available to the MPV player.

## Usage in Application

The shaders are used through the upscaling button in the video player controls:
- **None**: No upscaling (fastest)
- **High Quality**: Built-in MPV scalers
- **FSR**: AMD FidelityFX Super Resolution
- **Anime**: Anime4K optimized for animated content

## Performance Notes

- Shader performance depends on GPU capabilities
- FSR provides good balance of quality and performance
- Anime4K is optimized for real-time playback on mid-range GPUs
- FSRCNNX requires more GPU power but provides excellent quality

## Adding New Shaders

To add new shaders:
1. Ensure the shader has a compatible license (MIT, BSD, Apache 2.0, or GPL-compatible)
2. Place the `.glsl` file in this directory
3. Update this README with license and source information
4. Update the player code to include the new shader option

## License Compliance

All included shaders are used under their respective open-source licenses:
- AMD FSR: MIT License
- Anime4K: MIT License  
- FSRCNNX: MIT License (from dual-license option)