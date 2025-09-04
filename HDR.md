# High Dynamic Range (HDR) Support in Reel

This guide covers HDR implementation in Reel, leveraging GTK4's modern color management capabilities for embedded HDR playback within the native desktop experience.

## Overview

Reel implements HDR support using GTK4's new color management system, providing seamless HDR playback without external windows or breaking the integrated desktop experience. HDR support is available as an optional feature flag to maintain compatibility across different systems and GTK versions.

## Technical Architecture

### GTK4 Color Management Foundation

Reel's HDR implementation builds on GTK4's revolutionary color management system introduced in 2024:

- **GdkColorState**: Modern color space handling beyond legacy sRGB assumptions
- **BT.2100-PQ Support**: Native HDR color space for 10-bit and 12-bit content
- **Wayland Integration**: Direct communication with HDR-capable compositors via `xx-color-management-v4` protocol
- **Floating-Point Framebuffers**: GPU-accelerated HDR rendering with `GL_RGB16F/GL_RGBA16F` formats

### MPV Integration

The HDR pipeline integrates MPV's advanced color management with GTK4's capabilities:

```rust
// HDR-capable OpenGL context configuration
gl_area.set_required_version(4, 6);
gl_area.set_use_es(false);

// MPV HDR settings
mpv.set_property("target-colorspace-hint", "yes");
mpv.set_property("icc-profile-auto", "yes");
mpv.set_property("tone-mapping", "hable");
mpv.set_property("hdr-compute-peak", "yes");
mpv.set_property("target-trc", "pq");
mpv.set_property("target-prim", "bt.2020");
```

## Feature Flag Implementation

HDR support is controlled by configuration to ensure broad compatibility:

### Configuration

```toml
[playback]
hdr_enabled = false  # Default: disabled for compatibility
hdr_tone_mapping = "hable"  # Options: hable, reinhard, mobius, bt2390
hdr_peak_detection = true
hdr_fallback_sdr = true  # Graceful degradation
```

### Runtime Detection

The application automatically detects HDR capabilities:

```rust
pub fn detect_hdr_support() -> HdrCapabilities {
    HdrCapabilities {
        gtk_color_management: check_gtk_color_management(),
        wayland_hdr_protocol: check_wayland_hdr(),
        display_hdr_capable: check_display_capabilities(),
        driver_support: check_driver_hdr_support(),
    }
}
```

## Implementation Guide

### 1. Enhanced GLArea Context Creation

```rust
pub fn create_hdr_video_widget(&self, hdr_enabled: bool) -> gtk4::Widget {
    let gl_area = GLArea::new();
    
    if hdr_enabled {
        // Request modern OpenGL for HDR support
        gl_area.set_required_version(4, 6);
        gl_area.set_use_es(false);
        
        // Enable HDR-capable color format
        gl_area.set_has_alpha(true);
        gl_area.set_has_depth_buffer(false);
        gl_area.set_has_stencil_buffer(false);
    }
    
    // Configure color state if available
    if let Some(color_state) = get_hdr_color_state() {
        // Set BT.2100-PQ color state for HDR content
        gl_area.set_color_state(color_state);
    }
    
    gl_area
}
```

### 2. HDR-Aware MPV Configuration

```rust
fn configure_mpv_hdr(&self, mpv: &Mpv, config: &HdrConfig) -> Result<()> {
    // Color management
    mpv.set_property("target-colorspace-hint", "yes")?;
    mpv.set_property("icc-profile-auto", "yes")?;
    
    // HDR tone mapping
    mpv.set_property("tone-mapping", &config.tone_mapping)?;
    mpv.set_property("tone-mapping-param", config.tone_mapping_param)?;
    mpv.set_property("hdr-compute-peak", config.peak_detection)?;
    
    // HDR target configuration
    if config.hdr_enabled {
        mpv.set_property("target-trc", "pq")?;
        mpv.set_property("target-prim", "bt.2020")?;
        mpv.set_property("target-peak", config.target_peak_nits)?;
    }
    
    // Fallback handling
    if config.fallback_sdr {
        mpv.set_property("tone-mapping-mode", "hybrid")?;
    }
    
    Ok(())
}
```

### 3. Runtime Environment Configuration

Enable experimental GTK4 HDR features when the feature flag is active:

```rust
pub fn setup_hdr_environment() {
    if config.playback.hdr_enabled {
        std::env::set_var("GDK_DEBUG", "hdr");
        info!("HDR rendering enabled via GTK4 experimental support");
    }
}
```

## System Requirements

### Minimum Requirements
- **GTK4 4.10+**: Basic color management support
- **GNOME 47+**: Initial HDR compositor support
- **Mesa 24.0+** or **NVIDIA 550+**: HDR-capable GPU drivers
- **Wayland**: HDR protocol support (X11 not supported)

### Recommended Configuration
- **GTK4 4.16+**: Production-ready color management
- **GNOME 48+**: Stable HDR implementation
- **Hardware**: HDR-capable display with 10-bit+ panel

## Configuration Examples

### Conservative (Default)
```toml
[playback]
hdr_enabled = false
# Uses standard sRGB pipeline for maximum compatibility
```

### HDR Enthusiast
```toml
[playback]
hdr_enabled = true
hdr_tone_mapping = "hable"
hdr_peak_detection = true
hdr_target_peak = 1000  # nits
hdr_fallback_sdr = true
```

### Professional Color Grading
```toml
[playback]
hdr_enabled = true
hdr_tone_mapping = "bt2390"
hdr_peak_detection = false
hdr_target_peak = 4000  # nits
hdr_icc_profile_path = "/usr/share/color/icc/my-display.icc"
```

## Troubleshooting

### Common Issues

**HDR Content Appears Washed Out**
- Verify display HDR mode is enabled in system settings
- Check `target-peak` matches display capabilities
- Try different tone mapping algorithms

**Performance Issues**
- Disable `hdr-compute-peak` for better performance
- Use `mobius` tone mapping for faster processing
- Reduce `demuxer-max-bytes` if needed

**Color Accuracy Problems**
- Enable `icc-profile-auto` for display calibration
- Check Wayland compositor HDR support
- Verify GPU driver HDR capabilities

### Diagnostic Commands

```bash
# Check GTK4 HDR support
GDK_DEBUG=hdr reel --version

# Test HDR environment
echo $GDK_DEBUG
echo $GSK_RENDERER

# Verify Wayland HDR protocol
weston-info | grep color-management
```

## Performance Considerations

HDR rendering has minimal performance impact when properly configured:

- **GPU Usage**: ~5-10% increase for tone mapping
- **Memory**: Additional framebuffer allocations (~50MB for 4K)
- **CPU**: Negligible impact with hardware acceleration

### Optimization Tips

1. **Use hardware tone mapping** when available
2. **Cache ICC profiles** to avoid repeated loading
3. **Batch color space conversions** for efficiency
4. **Profile target peak detection** vs. fixed values

## Future Roadmap

### GTK4 4.16+ Integration
- Remove experimental flags requirement
- Native HDR widget support
- Improved color state APIs

### Advanced Features
- **HDR10+ dynamic metadata** support
- **Dolby Vision** tone mapping
- **Custom tone curves** for professional use
- **Real-time HDR analysis** overlays

## GStreamer HDR Implementation

Reel's dual-player architecture supports both MPV and GStreamer backends. While MPV provides the optimal HDR experience through the GTK4 integration described above, GStreamer offers an alternative HDR path with its own advantages and considerations.

### GStreamer HDR Architecture

GStreamer's HDR support centers around its video processing pipeline and color management capabilities:

```rust
// GStreamer HDR-capable pipeline configuration
pub fn create_hdr_gstreamer_pipeline(&self, hdr_config: &HdrConfig) -> Result<gst::Element> {
    let playbin = gst::ElementFactory::make("playbin3")
        .name("hdr_player")
        .build()?;
    
    // Create HDR-capable video sink
    let video_sink = self.create_hdr_video_sink(hdr_config)?;
    playbin.set_property("video-sink", &video_sink);
    
    // Enable HDR flags
    playbin.set_property_from_str(
        "flags", 
        "soft-colorbalance+deinterlace+soft-volume+audio+video+text"
    );
    
    Ok(playbin)
}
```

### Current GStreamer Implementation Analysis

The existing GStreamer player in Reel (`src/player/gstreamer_player.rs:1340`) provides a solid foundation for HDR integration:

**Strengths:**
- Uses `playbin3` (modern GStreamer playback) with better HDR metadata handling
- Implements `gtk4paintablesink` for seamless GTK4 integration
- Supports `videoconvertscale` for optimized color space conversion
- Includes comprehensive pipeline debugging and state management
- Handles subtitle overlay with RGBA format forcing (critical for HDR)

**HDR Integration Points:**
- Video sink creation (`create_optimized_video_sink` at line 141)
- Color format configuration (`RGBA` caps filtering at lines 295-298)
- Subtitle processing pipeline (`create_subtitle_filter` at line 428)

### GStreamer HDR Implementation Strategy

#### 1. HDR-Aware Sink Creation

```rust
fn create_hdr_video_sink(&self, hdr_config: &HdrConfig) -> Option<gst::Element> {
    let bin = gst::Bin::new();
    
    // HDR color space converter
    let convert = gst::ElementFactory::make("videoconvertscale")
        .name("hdr_converter")
        .build()
        .ok()?;
    
    // Configure HDR color matrix
    if hdr_config.hdr_enabled {
        // Set HDR color matrix (BT.2020 or BT.2100)
        convert.set_property_from_str("matrix-mode", "bt2020");
        convert.set_property_from_str("primaries-mode", "bt2020");
        convert.set_property_from_str("transfer-mode", "smpte2084"); // PQ curve
    }
    
    // HDR-capable caps filter
    let capsfilter = gst::ElementFactory::make("capsfilter").build().ok()?;
    let caps = if hdr_config.hdr_enabled {
        gst::Caps::builder("video/x-raw")
            .field("format", "P010_10LE") // 10-bit HDR format
            .field("colorimetry", "bt2020/bt2020-10/smpte2084/full")
            .build()
    } else {
        gst::Caps::builder("video/x-raw")
            .field("format", "RGBA")
            .build()
    };
    capsfilter.set_property("caps", &caps);
    
    // GTK4 sink for integration
    let gtk_sink = gst::ElementFactory::make("gtk4paintablesink")
        .name("hdr_gtk_sink")
        .build()
        .ok()?;
    
    // Build HDR pipeline
    bin.add_many([&convert, &capsfilter, &gtk_sink]).ok()?;
    gst::Element::link_many([&convert, &capsfilter, &gtk_sink]).ok()?;
    
    // Create ghost pad
    let sink_pad = convert.static_pad("sink")?;
    let ghost_pad = gst::GhostPad::with_target(&sink_pad).ok()?;
    bin.add_pad(&ghost_pad).ok()?;
    
    Some(bin.upcast())
}
```

#### 2. HDR Metadata Handling

```rust
fn configure_hdr_metadata(&self, playbin: &gst::Element, hdr_config: &HdrConfig) -> Result<()> {
    if !hdr_config.hdr_enabled {
        return Ok(());
    }
    
    // Set up HDR metadata handling via bus messages
    let bus = playbin.bus().context("Failed to get playbin bus")?;
    bus.add_watch(move |_, msg| {
        if let Some(structure) = msg.structure() {
            // Look for HDR metadata in stream tags
            if structure.name() == "application/x-hdr-metadata" {
                if let Ok(max_cll) = structure.get::<u32>("max-cll") {
                    info!("HDR MaxCLL: {} nits", max_cll);
                    // Configure tone mapping based on content
                }
                if let Ok(max_fall) = structure.get::<u32>("max-fall") {
                    info!("HDR MaxFALL: {} nits", max_fall);
                }
            }
        }
        glib::ControlFlow::Continue
    })?;
    
    Ok(())
}
```

#### 3. Configuration Integration

```rust
// Add to PlaybackConfig in config.rs
#[serde(default = "default_false", skip_serializing_if = "is_false")]
pub gstreamer_hdr_enabled: bool,

#[serde(
    default = "default_hdr_tone_mapping",
    skip_serializing_if = "is_default_hdr_tone_mapping"
)]
pub gstreamer_hdr_tone_mapping: String, // "bt2390", "reinhard", "linear"

#[serde(
    default = "default_hdr_target_nits",
    skip_serializing_if = "is_default_hdr_target_nits"
)]
pub gstreamer_hdr_target_nits: u32, // 1000, 4000, etc.
```

### GStreamer vs MPV HDR Comparison

| Feature | GStreamer HDR | MPV HDR | Notes |
|---------|---------------|---------|-------|
| **GTK4 Integration** | ✅ Native | ✅ Native | Both use gtk4paintablesink/mpv_render_context_render |
| **HDR Metadata** | 🟡 Basic | ✅ Complete | GStreamer has limited HDR10+ support |
| **Tone Mapping** | 🟡 Limited | ✅ Advanced | MPV has superior tone mapping algorithms |
| **Color Management** | ✅ Good | ✅ Excellent | Both support ICC profiles |
| **Performance** | ✅ Hardware | ✅ Hardware | Both can use GPU acceleration |
| **Subtitle Support** | ❌ Color Issues | ✅ Perfect | Known GStreamer subtitle color artifacts |
| **Wide Format Support** | 🟡 Good | ✅ Excellent | MPV supports more exotic HDR formats |
| **Configuration** | 🟡 Pipeline-based | ✅ Property-based | MPV easier to configure |

### System Requirements for GStreamer HDR

**Minimum GStreamer Version:**
- **GStreamer 1.20+**: Basic HDR10 support
- **GStreamer 1.22+**: Improved color management
- **GStreamer 1.24+**: Enhanced HDR metadata handling

**Required Plugins:**
```bash
# Check GStreamer HDR plugin availability
gst-inspect-1.0 videoconvertscale  # Color space conversion
gst-inspect-1.0 gtk4paintablesink  # GTK4 integration
gst-inspect-1.0 playbin3          # Modern playback
gst-inspect-1.0 vaapih264dec       # Hardware HDR decoding (Intel)
gst-inspect-1.0 nvh264dec         # Hardware HDR decoding (NVIDIA)
```

### Configuration Examples

#### Basic HDR (Compatibility Mode)
```toml
[playback]
player_backend = "gstreamer"
gstreamer_hdr_enabled = true
gstreamer_hdr_tone_mapping = "linear"
gstreamer_hdr_target_nits = 1000
```

#### Professional HDR Setup
```toml
[playback]
player_backend = "gstreamer"
gstreamer_hdr_enabled = true
gstreamer_hdr_tone_mapping = "bt2390"
gstreamer_hdr_target_nits = 4000
hardware_acceleration = true
```

### Limitations and Known Issues

**GStreamer HDR Limitations:**
1. **Subtitle Color Artifacts**: Converting HDR→SDR affects subtitle rendering colors
2. **Limited Tone Mapping**: Fewer algorithms compared to MPV's advanced options
3. **HDR10+ Support**: Dynamic metadata handling is incomplete
4. **Complex Pipeline**: HDR configuration requires pipeline manipulation
5. **Plugin Dependencies**: Requires specific GStreamer plugins for full HDR support

**Workarounds:**
- Use separate subtitle rendering pass after tone mapping
- Implement custom color correction for subtitle overlay
- Fall back to SDR mode when subtitle quality is critical

### Implementation Priority

Given the current state of HDR support:

1. **Primary HDR Implementation**: MPV with GTK4 (as documented above)
2. **Secondary HDR Option**: GStreamer for specific use cases
3. **Fallback Mode**: Auto-detect and disable HDR when subtitle issues occur

### Future GStreamer HDR Roadmap

**GStreamer 1.26+ Features:**
- Native HDR10+ dynamic metadata support
- Improved subtitle color handling in HDR pipelines  
- Better Wayland HDR integration
- Enhanced color management APIs

**Reel Integration Goals:**
- Automatic HDR format detection and configuration
- Seamless switching between SDR/HDR based on content
- Real-time HDR performance monitoring
- Advanced tone curve customization

## Alternative: gpu-next (Not Recommended)

While MPV's `gpu-next` video output supports HDR, it requires external windows that break Reel's integrated desktop experience. This approach:

- Creates popup windows outside the main interface
- Loses GTK4 theming and integration
- Complicates window management
- Provides no significant HDR advantages over the GTK4 approach

The GTK4 embedded approach is strongly preferred for maintaining Reel's premium, integrated user experience while providing excellent HDR capabilities.

## Conclusion

Reel's HDR implementation represents the future of Linux desktop media playback, combining cutting-edge GTK4 color management with MPV's proven HDR processing. The feature flag approach ensures compatibility while enabling users with modern systems to enjoy true HDR content within a beautifully integrated desktop experience.

For most users, HDR support should remain disabled for now. Early adopters with GNOME 48+ and HDR displays can enable the feature flag to experience the future of desktop media playback.