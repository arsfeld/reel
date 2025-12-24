use gstreamer as gst;
use gstreamer::prelude::*;
#[cfg(target_os = "macos")]
use tracing::warn;
use tracing::{debug, error, info};

/// Creates an optimized video sink based on platform and available features.
///
/// This function tries multiple sink configurations in order of preference:
/// 1. macOS-specific sink (on macOS only)
/// 2. glsinkbin + gtk4paintablesink (best performance)
/// 3. gtk4paintablesink with conversion pipeline
/// 4. glimagesink fallback
/// 5. autovideosink fallback
pub fn create_optimized_video_sink(
    force_fallback: bool,
    use_gl_sink: bool,
) -> Option<gst::Element> {
    // On macOS, prefer native video sinks for better compatibility
    #[cfg(target_os = "macos")]
    {
        if !force_fallback {
            // Try macOS-specific sink configuration
            if let Some(sink) = create_macos_video_sink() {
                info!("Using macOS-optimized video sink");
                return Some(sink);
            }
        }
    }

    if !force_fallback && !use_gl_sink {
        // Try glsinkbin + gtk4paintablesink first (best performance)
        if let Some(sink) = create_glsinkbin_gtk4_sink() {
            info!("Using glsinkbin + gtk4paintablesink (optimal GL handling)");
            return Some(sink);
        }

        // Fallback to direct gtk4paintablesink
        if let Some(sink) = create_gtk4_sink_with_conversion() {
            info!("Using gtk4paintablesink with conversion pipeline");
            return Some(sink);
        }
    }

    // Try glimagesink or autovideosink fallback
    if use_gl_sink && let Some(sink) = create_gl_fallback_sink() {
        info!("Using glimagesink fallback");
        return Some(sink);
    }

    // Final fallback to autovideosink
    if let Some(sink) = create_auto_fallback_sink() {
        info!("Using autovideosink fallback");
        return Some(sink);
    }

    error!("Failed to create any video sink!");
    None
}

/// Creates a macOS-specific video sink with GTK integration.
///
/// Prefers gtk4paintablesink with proper conversion for macOS,
/// with fallbacks to glimagesink and osxvideosink.
#[cfg(target_os = "macos")]
pub fn create_macos_video_sink() -> Option<gst::Element> {
    info!("Creating macOS-specific video sink");

    // For GTK integration, we should prefer gtk4paintablesink even on macOS
    // but with macOS-specific pipeline setup

    // Try gtk4paintablesink with proper conversion for macOS
    if let Ok(gtk_sink) = gst::ElementFactory::make("gtk4paintablesink")
        .name("gtk4paintablesink")
        .build()
    {
        // Create a bin with conversion elements optimized for macOS
        let bin = gst::Bin::new();

        // Use videoconvert for format conversion
        let convert = gst::ElementFactory::make("videoconvert")
            .name("video_converter")
            .build()
            .ok()?;

        // Set properties for better macOS performance
        convert.set_property("n-threads", 0u32); // Auto-detect optimal threads

        // Add a videoscale element for macOS compatibility
        let scale = gst::ElementFactory::make("videoscale")
            .name("video_scaler")
            .build()
            .ok()?;

        // Add elements to bin
        bin.add(&convert).ok()?;
        bin.add(&scale).ok()?;
        bin.add(&gtk_sink).ok()?;

        // Link elements
        gst::Element::link_many([&convert, &scale, &gtk_sink]).ok()?;

        // Create ghost pad
        let sink_pad = convert.static_pad("sink")?;
        let ghost_pad = gst::GhostPad::with_target(&sink_pad).ok()?;
        ghost_pad.set_active(true).ok()?;
        bin.add_pad(&ghost_pad).ok()?;

        info!("Using gtk4paintablesink with videoconvert+videoscale for macOS");
        return Some(bin.upcast());
    }

    // Fallback to glimagesink (won't integrate with GTK Picture widget)
    if let Ok(glsink) = gst::ElementFactory::make("glimagesink")
        .name("glimagesink")
        .build()
    {
        // Set properties for better macOS compatibility
        glsink.set_property("force-aspect-ratio", true);
        info!("Using glimagesink fallback for macOS");
        return Some(glsink);
    }

    // Last resort: osxvideosink (native but opens separate window)
    if let Ok(osxsink) = gst::ElementFactory::make("osxvideosink")
        .name("osxvideosink")
        .build()
    {
        warn!("Using osxvideosink - video will appear in separate window");
        return Some(osxsink);
    }

    None
}

/// Creates a glsinkbin with gtk4paintablesink for optimal GL handling.
///
/// This configuration provides the best performance by using glsinkbin
/// to handle OpenGL operations with gtk4paintablesink for GTK integration.
pub fn create_glsinkbin_gtk4_sink() -> Option<gst::Element> {
    info!("Attempting to create glsinkbin + gtk4paintablesink pipeline");

    // Try to create glsinkbin with gtk4paintablesink
    let glsinkbin = gst::ElementFactory::make("glsinkbin")
        .name("glsinkbin")
        .build()
        .ok()?;

    let gtk_sink = gst::ElementFactory::make("gtk4paintablesink")
        .name("gtk4paintablesink")
        .build()
        .ok()?;

    // Ensure aspect ratio is preserved
    if gtk_sink.has_property("force-aspect-ratio") {
        gtk_sink.set_property("force-aspect-ratio", true);
    }

    // Set gtk4paintablesink as the sink for glsinkbin
    glsinkbin.set_property("sink", &gtk_sink);

    // Create conversion bin with optimized videoconvertscale
    let bin = gst::Bin::new();

    // Try to use videoconvertscale (combined element)
    let convert = if let Ok(convert) = gst::ElementFactory::make("videoconvertscale")
        .name("video_converter")
        .build()
    {
        debug!("Using optimized videoconvertscale element");
        convert
    } else {
        // Fallback to separate elements
        debug!("videoconvertscale not available, using separate videoconvert");
        gst::ElementFactory::make("videoconvert")
            .name("video_converter")
            .build()
            .ok()?
    };

    // Auto-detect optimal thread count (0 = automatic)
    convert.set_property("n-threads", 0u32);
    debug!("Set video converter to auto-detect optimal thread count");

    // Force RGBA for subtitle overlay compatibility
    let capsfilter = gst::ElementFactory::make("capsfilter")
        .name("capsfilter")
        .build()
        .ok()?;

    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "RGBA")
        .build();
    capsfilter.set_property("caps", &caps);

    // Build pipeline
    bin.add(&convert).ok()?;
    bin.add(&capsfilter).ok()?;
    bin.add(&glsinkbin).ok()?;

    gst::Element::link_many([&convert, &capsfilter, &glsinkbin]).ok()?;

    // Add ghost pad
    let sink_pad = convert.static_pad("sink")?;
    let ghost_pad = gst::GhostPad::with_target(&sink_pad).ok()?;
    bin.add_pad(&ghost_pad).ok()?;

    Some(bin.upcast())
}

/// Creates a gtk4paintablesink with video conversion pipeline.
///
/// This provides GTK integration with proper format conversion for compatibility.
pub fn create_gtk4_sink_with_conversion() -> Option<gst::Element> {
    let gtk_sink = gst::ElementFactory::make("gtk4paintablesink")
        .name("gtk4paintablesink")
        .build()
        .ok()?;

    // Ensure aspect ratio is preserved
    if gtk_sink.has_property("force-aspect-ratio") {
        gtk_sink.set_property("force-aspect-ratio", true);
    }

    let bin = gst::Bin::new();

    // Try videoconvertscale first, fallback to separate elements
    let convert = if let Ok(convert) = gst::ElementFactory::make("videoconvertscale")
        .name("video_converter")
        .build()
    {
        debug!("Using optimized videoconvertscale element");
        convert
    } else {
        debug!("videoconvertscale not available, using separate videoconvert");
        gst::ElementFactory::make("videoconvert")
            .name("video_converter")
            .build()
            .ok()?
    };

    // Auto-detect optimal thread count (0 = automatic)
    convert.set_property("n-threads", 0u32);

    // Force RGBA format
    let capsfilter = gst::ElementFactory::make("capsfilter")
        .name("capsfilter")
        .build()
        .ok()?;

    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "RGBA")
        .build();
    capsfilter.set_property("caps", &caps);

    // Build pipeline
    bin.add(&convert).ok()?;
    bin.add(&capsfilter).ok()?;
    bin.add(&gtk_sink).ok()?;

    gst::Element::link_many([&convert, &capsfilter, &gtk_sink]).ok()?;

    // Create ghost pad
    let sink_pad = convert.static_pad("sink")?;
    let ghost_pad = gst::GhostPad::with_target(&sink_pad).ok()?;
    bin.add_pad(&ghost_pad).ok()?;

    Some(bin.upcast())
}

/// Creates a glimagesink fallback with conversion pipeline.
pub fn create_gl_fallback_sink() -> Option<gst::Element> {
    let gl_sink = gst::ElementFactory::make("glimagesink")
        .name("glimagesink")
        .build()
        .ok()?;

    create_sink_with_conversion(gl_sink)
}

/// Creates an autovideosink fallback with conversion pipeline.
pub fn create_auto_fallback_sink() -> Option<gst::Element> {
    let auto_sink = gst::ElementFactory::make("autovideosink")
        .name("autovideosink")
        .build()
        .ok()?;

    create_sink_with_conversion(auto_sink)
}

/// Creates a bin with video conversion pipeline for the given sink element.
///
/// This helper function wraps any sink with proper video conversion,
/// format filtering (RGBA), and ghost pad setup.
pub fn create_sink_with_conversion(sink: gst::Element) -> Option<gst::Element> {
    let bin = gst::Bin::new();

    // Try videoconvertscale first
    let convert = if let Ok(convert) = gst::ElementFactory::make("videoconvertscale")
        .name("video_converter")
        .build()
    {
        convert
    } else {
        gst::ElementFactory::make("videoconvert")
            .name("video_converter")
            .build()
            .ok()?
    };

    // Auto-detect optimal thread count (0 = automatic)
    convert.set_property("n-threads", 0u32);

    // Force RGBA for subtitle compatibility
    let capsfilter = gst::ElementFactory::make("capsfilter")
        .name("capsfilter")
        .build()
        .ok()?;

    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "RGBA")
        .build();
    capsfilter.set_property("caps", &caps);

    bin.add(&convert).ok()?;
    bin.add(&capsfilter).ok()?;
    bin.add(&sink).ok()?;

    gst::Element::link_many([&convert, &capsfilter, &sink]).ok()?;

    let sink_pad = convert.static_pad("sink")?;
    let ghost_pad = gst::GhostPad::with_target(&sink_pad).ok()?;
    bin.add_pad(&ghost_pad).ok()?;

    Some(bin.upcast())
}

/// Extracts the gtk4paintablesink element from a bin or returns the element if it's already a gtk4paintablesink.
///
/// This function recursively searches through bins to find the gtk4paintablesink element.
pub fn extract_gtk4_sink(element: &gst::Element) -> Option<gst::Element> {
    // Check if this is a bin
    if let Some(bin) = element.dynamic_cast_ref::<gst::Bin>() {
        // Iterate through bin elements to find gtk4paintablesink
        let mut iter = bin.iterate_elements();
        while let Ok(Some(elem)) = iter.next() {
            if elem
                .factory()
                .is_some_and(|f| f.name() == "gtk4paintablesink")
            {
                return Some(elem);
            }
            // Recursively check if this element is also a bin
            if let Some(sink) = extract_gtk4_sink(&elem) {
                return Some(sink);
            }
        }
    } else if element
        .factory()
        .is_some_and(|f| f.name() == "gtk4paintablesink")
    {
        return Some(element.clone());
    }
    None
}
