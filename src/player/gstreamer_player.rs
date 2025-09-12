use anyhow::{Context, Result};
use gdk4 as gdk;
use gstreamer as gst;
use gstreamer::glib;
use gstreamer::prelude::*;
use gtk4::{self, prelude::*};
use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone)]
pub enum PlayerState {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopped,
    Error,
}

pub struct GStreamerPlayer {
    playbin: RefCell<Option<gst::Element>>,
    state: Arc<RwLock<PlayerState>>,
    video_widget: RefCell<Option<gtk4::Widget>>,
    video_sink: RefCell<Option<gst::Element>>,
    is_playbin3: RefCell<bool>,
}

impl GStreamerPlayer {
    pub fn new() -> Result<Self> {
        info!("GStreamerPlayer::new() - Initializing GStreamer player");

        // Initialize GStreamer if not already done
        match gst::init() {
            Ok(_) => info!("GStreamerPlayer::new() - GStreamer initialized successfully"),
            Err(e) => {
                error!(
                    "GStreamerPlayer::new() - Failed to initialize GStreamer: {}",
                    e
                );
                return Err(anyhow::anyhow!("Failed to initialize GStreamer: {}", e));
            }
        }

        // Check for required elements
        Self::check_gstreamer_plugins();

        Ok(Self {
            playbin: RefCell::new(None),
            state: Arc::new(RwLock::new(PlayerState::Idle)),
            video_widget: RefCell::new(None),
            video_sink: RefCell::new(None),
            is_playbin3: RefCell::new(false),
        })
    }

    fn check_gstreamer_plugins() {
        info!("Checking GStreamer plugin availability");

        let required_elements = vec![
            "playbin3",
            "playbin", // Fallback if playbin3 not available
            "autovideosink",
            "autoaudiosink",
            "gtk4paintablesink",
            "glimagesink",
            "videoconvertscale", // Combined element for better performance
            "videoconvert",      // Fallback
            "videoscale",        // Fallback
            "capsfilter",
            "glsinkbin", // For better GL handling
        ];

        for element in required_elements {
            if let Some(factory) = gst::ElementFactory::find(element) {
                info!("  ✓ {} available (rank: {})", element, factory.rank());
            } else {
                error!("  ✗ {} NOT available", element);
            }
        }

        // List available playback elements
        let registry = gst::Registry::get();
        let factories = registry.features_filtered(|_| true, false);
        let playback_factories: Vec<String> = factories
            .iter()
            .filter_map(|f| f.downcast_ref::<gst::ElementFactory>())
            .filter(|f| {
                let name = f.name();
                name.contains("play") || name.contains("sink") || name.contains("decode")
            })
            .map(|f| f.name().to_string())
            .collect();

        info!(
            "Available playback-related elements: {:?}",
            playback_factories
        );
    }

    pub fn create_video_widget(&self) -> gtk4::Widget {
        info!("GStreamerPlayer::create_video_widget() - Starting video widget creation");

        // Create a GTK Picture widget for video display
        debug!("GStreamerPlayer::create_video_widget() - Creating GTK Picture widget");
        let picture = gtk4::Picture::new();
        picture.set_can_shrink(true);
        picture.set_vexpand(true);
        picture.set_hexpand(true);
        debug!("GStreamerPlayer::create_video_widget() - Picture widget created");

        // Check if we should force fallback mode or use alternative sink
        let force_fallback = std::env::var("REEL_FORCE_FALLBACK_SINK").is_ok();
        let use_gl_sink = std::env::var("REEL_USE_GL_SINK").is_ok();

        // Try to create optimized video sink with glsinkbin wrapper
        let video_sink = self.create_optimized_video_sink(force_fallback, use_gl_sink);

        // If we have a gtk4paintablesink, extract and set its paintable
        if let Some(ref sink) = video_sink
            && let Some(gtk_sink) = self.extract_gtk4_sink(sink)
        {
            let paintable = gtk_sink.property::<gdk::Paintable>("paintable");
            picture.set_paintable(Some(&paintable));
            debug!("GStreamerPlayer::create_video_widget() - Paintable set on Picture widget");
        }

        // Store the video sink
        self.video_sink.replace(video_sink);

        // Store and return the widget
        let widget = picture.upcast::<gtk4::Widget>();
        self.video_widget.replace(Some(widget.clone()));

        info!("GStreamerPlayer::create_video_widget() - Video widget creation complete");
        widget
    }

    fn create_optimized_video_sink(
        &self,
        force_fallback: bool,
        use_gl_sink: bool,
    ) -> Option<gst::Element> {
        // On macOS, prefer native video sinks for better compatibility
        #[cfg(target_os = "macos")]
        {
            if !force_fallback {
                // Try macOS-specific sink configuration
                if let Some(sink) = self.create_macos_video_sink() {
                    info!("Using macOS-optimized video sink");
                    return Some(sink);
                }
            }
        }

        if !force_fallback && !use_gl_sink {
            // Try glsinkbin + gtk4paintablesink first (best performance)
            if let Some(sink) = self.create_glsinkbin_gtk4_sink() {
                info!("Using glsinkbin + gtk4paintablesink (optimal GL handling)");
                return Some(sink);
            }

            // Fallback to direct gtk4paintablesink
            if let Some(sink) = self.create_gtk4_sink_with_conversion() {
                info!("Using gtk4paintablesink with conversion pipeline");
                return Some(sink);
            }
        }

        // Try glimagesink or autovideosink fallback
        if use_gl_sink && let Some(sink) = self.create_gl_fallback_sink() {
            info!("Using glimagesink fallback");
            return Some(sink);
        }

        // Final fallback to autovideosink
        if let Some(sink) = self.create_auto_fallback_sink() {
            info!("Using autovideosink fallback");
            return Some(sink);
        }

        error!("Failed to create any video sink!");
        None
    }

    #[cfg(target_os = "macos")]
    fn create_macos_video_sink(&self) -> Option<gst::Element> {
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

            // Add elements to bin
            bin.add(&convert).ok()?;
            bin.add(&gtk_sink).ok()?;

            // Link elements
            convert.link(&gtk_sink).ok()?;

            // Create ghost pad
            let sink_pad = convert.static_pad("sink")?;
            let ghost_pad = gst::GhostPad::with_target(&sink_pad).ok()?;
            ghost_pad.set_active(true).ok()?;
            bin.add_pad(&ghost_pad).ok()?;

            info!("Using gtk4paintablesink with videoconvert for macOS");
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

    fn create_glsinkbin_gtk4_sink(&self) -> Option<gst::Element> {
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

    fn create_gtk4_sink_with_conversion(&self) -> Option<gst::Element> {
        let gtk_sink = gst::ElementFactory::make("gtk4paintablesink")
            .name("gtk4paintablesink")
            .build()
            .ok()?;

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

    fn create_gl_fallback_sink(&self) -> Option<gst::Element> {
        let gl_sink = gst::ElementFactory::make("glimagesink")
            .name("glimagesink")
            .build()
            .ok()?;

        self.create_sink_with_conversion(gl_sink)
    }

    fn create_auto_fallback_sink(&self) -> Option<gst::Element> {
        let auto_sink = gst::ElementFactory::make("autovideosink")
            .name("autovideosink")
            .build()
            .ok()?;

        self.create_sink_with_conversion(auto_sink)
    }

    fn create_sink_with_conversion(&self, sink: gst::Element) -> Option<gst::Element> {
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

    fn print_gst_launch_pipeline(&self, playbin: &gst::Element, _url: &str) {
        // Check if GST_DEBUG_DUMP_DOT_DIR is set
        let dot_dir = std::env::var("GST_DEBUG_DUMP_DOT_DIR").unwrap_or_else(|_| {
            // If not set, set it to /tmp
            unsafe {
                std::env::set_var("GST_DEBUG_DUMP_DOT_DIR", "/tmp");
            }
            "/tmp".to_string()
        });

        info!("GST_DEBUG_DUMP_DOT_DIR is set to: {}", dot_dir);

        // Dump the pipeline graph
        if let Some(bin) = playbin.dynamic_cast_ref::<gst::Bin>() {
            bin.debug_to_dot_file(gst::DebugGraphDetails::ALL, "reel-playbin-READY");
            info!("Dumped pipeline to {}/reel-playbin-READY.dot", dot_dir);

            // Also try to get the actual pipeline description
            info!("════════════════════════════════════════════════════════════════");
            info!("Playbin element details:");
            info!("  Name: {}", playbin.name());
            if let Some(factory) = playbin.factory() {
                info!("  Factory: {}", factory.name());
            }

            // List all properties for debugging
            if playbin.has_property("uri") {
                let uri: String = playbin.property("uri");
                info!("  URI: {}", uri);
            }

            // Try to iterate through bin children
            let mut count = 0;
            let mut iter = bin.iterate_elements();
            info!("Elements in playbin:");
            while let Ok(Some(elem)) = iter.next() {
                count += 1;
                if let Some(factory) = elem.factory() {
                    info!("  - {} ({})", elem.name(), factory.name());
                } else {
                    info!("  - {}", elem.name());
                }
            }
            info!("Total elements in bin: {}", count);
            info!("════════════════════════════════════════════════════════════════");
        } else {
            error!("Playbin is not a Bin! Type: {:?}", playbin.type_());
        }
    }

    fn extract_gtk4_sink(&self, element: &gst::Element) -> Option<gst::Element> {
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
                if let Some(sink) = self.extract_gtk4_sink(&elem) {
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

    pub async fn load_media(&self, url: &str, _video_sink: Option<&gst::Element>) -> Result<()> {
        info!("GStreamerPlayer::load_media() - Loading media: {}", url);
        debug!("GStreamerPlayer::load_media() - Full URL: {}", url);

        // Update state
        {
            let mut state = self.state.write().await;
            *state = PlayerState::Loading;
            debug!("GStreamerPlayer::load_media() - State set to Loading");
        }

        // Clear existing playbin if any
        if let Some(old_playbin) = self.playbin.borrow().as_ref() {
            debug!("GStreamerPlayer::load_media() - Clearing existing playbin");
            old_playbin
                .set_state(gst::State::Null)
                .context("Failed to set old playbin to null state")?;
        }

        // Try to create playbin3 first (better subtitle support)
        let (playbin, is_playbin3) = if gst::ElementFactory::find("playbin3").is_some() {
            info!("GStreamerPlayer::load_media() - Creating playbin3 element");
            let pb = gst::ElementFactory::make("playbin3")
                .name("player")
                .property("uri", url)
                .build()
                .context("Failed to create playbin3 element")?;

            // Enable all features including text overlay
            pb.set_property_from_str(
                "flags",
                "soft-colorbalance+deinterlace+soft-volume+audio+video+text",
            );

            // playbin3 has QoS always enabled, no property needed

            // Configure subtitle properties
            if pb.has_property("subtitle-encoding") {
                pb.set_property_from_str("subtitle-encoding", "UTF-8");
                info!("Set subtitle encoding to UTF-8");
            }

            if pb.has_property("subtitle-font-desc") {
                pb.set_property_from_str("subtitle-font-desc", "Sans, 18");
                info!("Set subtitle font to Sans, 18");
            }

            info!("GStreamerPlayer::load_media() - Using modern playbin3");
            (pb, true)
        } else if gst::ElementFactory::find("playbin").is_some() {
            info!(
                "GStreamerPlayer::load_media() - Falling back to playbin (playbin3 not available)"
            );
            let pb = gst::ElementFactory::make("playbin")
                .name("player")
                .property("uri", url)
                .build()
                .context("Failed to create playbin element")?;

            // Enable all features including text overlay
            pb.set_property_from_str(
                "flags",
                "soft-colorbalance+deinterlace+soft-volume+audio+video+text",
            );

            // Enable QoS for better performance
            if pb.has_property("enable-qos") {
                pb.set_property("enable-qos", true);
                info!("Enabled QoS for better performance");
            }

            // Configure subtitle rendering properties
            if pb.has_property("subtitle-encoding") {
                pb.set_property_from_str("subtitle-encoding", "UTF-8");
                info!("Set subtitle encoding to UTF-8");
            }

            if pb.has_property("subtitle-font-desc") {
                pb.set_property_from_str("subtitle-font-desc", "Sans, 18");
                info!("Set subtitle font to Sans, 18");
            }

            (pb, false)
        } else {
            error!("GStreamerPlayer::load_media() - No playbin/playbin3 element available!");
            return Err(anyhow::anyhow!(
                "No playbin/playbin3 element available - GStreamer plugins may not be properly installed"
            ));
        };

        // Store whether we're using playbin3
        self.is_playbin3.replace(is_playbin3);

        // Use our stored video sink if available
        if let Some(sink) = self.video_sink.borrow().as_ref() {
            debug!("GStreamerPlayer::load_media() - Setting video sink on playbin");
            playbin.set_property("video-sink", sink);
            info!("GStreamerPlayer::load_media() - Video sink configured");
        } else {
            // Create a fallback video sink
            info!("GStreamerPlayer::load_media() - No pre-configured sink, creating fallback");

            if let Some(fallback_sink) = self.create_auto_fallback_sink() {
                playbin.set_property("video-sink", &fallback_sink);
                info!("GStreamerPlayer::load_media() - Fallback video sink configured");
            } else {
                error!("GStreamerPlayer::load_media() - Failed to create any fallback video sink!");
            }
        }

        // Store the playbin
        self.playbin.replace(Some(playbin.clone()));
        debug!("GStreamerPlayer::load_media() - Playbin stored");

        // Print the pipeline in gst-launch format
        self.print_gst_launch_pipeline(&playbin, url);

        // Debug: Log the complete pipeline structure
        if let Some(sink) = playbin.property::<Option<gst::Element>>("video-sink") {
            info!("video-sink is attached: {:?}", sink.name());
            // Try to inspect the sink structure
            if let Some(bin) = sink.dynamic_cast_ref::<gst::Bin>() {
                info!("video-sink is a bin, contains:");
                let mut iter = bin.iterate_elements();
                while let Ok(Some(elem)) = iter.next() {
                    info!(
                        "  - {}: {}",
                        elem.name(),
                        elem.factory()
                            .map_or("unknown".to_string(), |f| f.name().to_string())
                    );
                }
            }
        }

        // Set up message handling
        let bus = playbin.bus().context("Failed to get playbin bus")?;
        debug!("GStreamerPlayer::load_media() - Got playbin bus");

        let state_clone = self.state.clone();
        let _ = bus
            .add_watch(move |_, msg| {
                let state = state_clone.clone();
                let msg = msg.clone();
                glib::spawn_future_local(async move {
                    Self::handle_bus_message(&msg, state).await;
                });
                glib::ControlFlow::Continue
            })
            .context("Failed to add bus watch")?;

        // Set to ready state first
        debug!("GStreamerPlayer::load_media() - Setting playbin to Ready state");
        playbin
            .set_state(gst::State::Ready)
            .context("Failed to set playbin to ready state")?;
        info!("GStreamerPlayer::load_media() - Playbin set to Ready state");

        info!("GStreamerPlayer::load_media() - Media loading complete");
        Ok(())
    }

    pub async fn play(&self) -> Result<()> {
        info!("GStreamerPlayer::play() - Starting playback");

        if let Some(playbin) = self.playbin.borrow().as_ref() {
            debug!("GStreamerPlayer::play() - Setting playbin to Playing state");
            match playbin.set_state(gst::State::Playing) {
                Ok(gst::StateChangeSuccess::Success) => {
                    info!("GStreamerPlayer::play() - Successfully set playbin to playing state");

                    // On macOS, ensure the state change is complete
                    #[cfg(target_os = "macos")]
                    {
                        // Wait for state change to complete with a timeout
                        let (state_change, current, _) =
                            playbin.state(gst::ClockTime::from_seconds(2));
                        match state_change {
                            Ok(gst::StateChangeSuccess::Success) => {
                                info!(
                                    "GStreamerPlayer::play() - State change confirmed, now in {:?}",
                                    current
                                );
                            }
                            _ => {
                                warn!(
                                    "GStreamerPlayer::play() - State change not complete, current: {:?}",
                                    current
                                );
                            }
                        }
                    }

                    // Ensure GST_DEBUG_DUMP_DOT_DIR is set
                    if std::env::var("GST_DEBUG_DUMP_DOT_DIR").is_err() {
                        unsafe {
                            std::env::set_var("GST_DEBUG_DUMP_DOT_DIR", "/tmp");
                        }
                    }

                    // Dump the actual running pipeline after it starts playing
                    if let Some(bin) = playbin.dynamic_cast_ref::<gst::Bin>() {
                        bin.debug_to_dot_file(gst::DebugGraphDetails::ALL, "reel-playbin-PLAYING");
                        info!("Dumped PLAYING state pipeline to /tmp/reel-playbin-PLAYING.dot");

                        // List what's actually in the pipeline now
                        let mut iter = bin.iterate_elements();
                        info!("Elements in PLAYING pipeline:");
                        while let Ok(Some(elem)) = iter.next() {
                            if let Some(factory) = elem.factory() {
                                info!("  - {} ({})", elem.name(), factory.name());
                            }
                        }
                    }
                }
                Ok(gst::StateChangeSuccess::Async) => {
                    info!("GStreamerPlayer::play() - Playbin state change is async, waiting...");

                    // On macOS, wait for the async state change to complete
                    #[cfg(target_os = "macos")]
                    {
                        let (state_change, current, _) =
                            playbin.state(gst::ClockTime::from_seconds(3));
                        match state_change {
                            Ok(gst::StateChangeSuccess::Success) => {
                                info!(
                                    "GStreamerPlayer::play() - Async state change completed, now in {:?}",
                                    current
                                );
                            }
                            _ => {
                                warn!(
                                    "GStreamerPlayer::play() - Async state change still pending after 3s, current: {:?}",
                                    current
                                );
                            }
                        }
                    }

                    // Wait a bit for the pipeline to negotiate, then dump it
                    glib::timeout_add_once(std::time::Duration::from_secs(2), {
                        let playbin = playbin.clone();
                        move || {
                            // Ensure GST_DEBUG_DUMP_DOT_DIR is set
                            if std::env::var("GST_DEBUG_DUMP_DOT_DIR").is_err() {
                                unsafe {
                                    std::env::set_var("GST_DEBUG_DUMP_DOT_DIR", "/tmp");
                                }
                            }

                            if let Some(bin) = playbin.dynamic_cast_ref::<gst::Bin>() {
                                bin.debug_to_dot_file(
                                    gst::DebugGraphDetails::ALL,
                                    "reel-playbin-PLAYING-async",
                                );
                                info!(
                                    "Dumped PLAYING state pipeline (async) to /tmp/reel-playbin-PLAYING-async.dot"
                                );

                                // List what's actually in the pipeline now
                                let mut iter = bin.iterate_elements();
                                info!("Elements in PLAYING pipeline (after async):");
                                while let Ok(Some(elem)) = iter.next() {
                                    if let Some(factory) = elem.factory() {
                                        info!("  - {} ({})", elem.name(), factory.name());
                                    }
                                }
                            }
                        }
                    });
                }
                Ok(gst::StateChangeSuccess::NoPreroll) => {
                    info!("GStreamerPlayer::play() - Playbin state change: no preroll");
                }
                Err(gst::StateChangeError) => {
                    // Get more details about the error
                    let state = playbin.state(gst::ClockTime::from_seconds(1));
                    error!("GStreamerPlayer::play() - Failed to set playbin to playing state");
                    error!("GStreamerPlayer::play() - Current state: {:?}", state);

                    // Get the bus to check for error messages
                    if let Some(bus) = playbin.bus() {
                        while let Some(msg) = bus.pop() {
                            use gst::MessageView;
                            if let MessageView::Error(err) = msg.view() {
                                error!(
                                    "GStreamerPlayer::play() - Bus error: {} ({:?})",
                                    err.error(),
                                    err.debug()
                                );
                            }
                        }
                    }

                    return Err(anyhow::anyhow!("Failed to set playbin to playing state"));
                }
            }

            let mut state = self.state.write().await;
            *state = PlayerState::Playing;
            info!("GStreamerPlayer::play() - Player state set to Playing");
        } else {
            error!("GStreamerPlayer::play() - No playbin available!");
            return Err(anyhow::anyhow!("No playbin available"));
        }
        info!("GStreamerPlayer::play() - Playback started");
        Ok(())
    }

    pub async fn pause(&self) -> Result<()> {
        debug!("Pausing playback");

        if let Some(playbin) = self.playbin.borrow().as_ref() {
            playbin
                .set_state(gst::State::Paused)
                .context("Failed to set playbin to paused state")?;

            let mut state = self.state.write().await;
            *state = PlayerState::Paused;
        }
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        debug!("Stopping playback");

        if let Some(playbin) = self.playbin.borrow().as_ref() {
            playbin
                .set_state(gst::State::Null)
                .context("Failed to set playbin to null state")?;

            let mut state = self.state.write().await;
            *state = PlayerState::Stopped;
        }
        Ok(())
    }

    pub async fn seek(&self, position: Duration) -> Result<()> {
        debug!("Seeking to {:?}", position);

        let position_ns = position.as_nanos() as i64;

        if let Some(playbin) = self.playbin.borrow().as_ref() {
            playbin
                .seek_simple(
                    gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                    gst::ClockTime::from_nseconds(position_ns as u64),
                )
                .context("Failed to seek")?;
        }
        Ok(())
    }

    pub async fn get_position(&self) -> Option<Duration> {
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            playbin
                .query_position::<gst::ClockTime>()
                .map(|pos| Duration::from_nanos(pos.nseconds()))
        } else {
            None
        }
    }

    pub async fn get_duration(&self) -> Option<Duration> {
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            playbin
                .query_duration::<gst::ClockTime>()
                .map(|dur| Duration::from_nanos(dur.nseconds()))
        } else {
            None
        }
    }

    pub async fn set_volume(&self, volume: f64) -> Result<()> {
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            playbin.set_property("volume", volume);
        }
        Ok(())
    }

    pub async fn get_video_dimensions(&self) -> Option<(i32, i32)> {
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            // Get video sink's pad
            if let Some(video_sink) = playbin.property::<Option<gst::Element>>("video-sink")
                && let Some(sink_pad) = video_sink.static_pad("sink")
                && let Some(caps) = sink_pad.current_caps()
                && let Some(structure) = caps.structure(0)
            {
                let width = structure.get::<i32>("width").ok();
                let height = structure.get::<i32>("height").ok();
                if let (Some(w), Some(h)) = (width, height) {
                    return Some((w, h));
                }
            }

            // Alternative: try to get from stream info
            // Handle both playbin and playbin3 property names
            let n_video = if playbin.has_property("n-video-streams") {
                // playbin3 property
                playbin.property::<i32>("n-video-streams")
            } else if playbin.has_property("n-video") {
                // playbin property
                playbin.property::<i32>("n-video")
            } else {
                0
            };

            if n_video > 0 {
                // Try to get video pad - signal names differ between playbin versions
                let pad = if playbin.has_property("n-video-streams") {
                    // playbin3 uses get-video-stream-pad
                    playbin.emit_by_name::<Option<gst::Pad>>("get-video-stream-pad", &[&0i32])
                } else {
                    // playbin uses get-video-pad
                    playbin.emit_by_name::<Option<gst::Pad>>("get-video-pad", &[&0i32])
                };

                if let Some(pad) = pad
                    && let Some(caps) = pad.current_caps()
                    && let Some(structure) = caps.structure(0)
                {
                    let width = structure.get::<i32>("width").ok();
                    let height = structure.get::<i32>("height").ok();
                    if let (Some(w), Some(h)) = (width, height) {
                        return Some((w, h));
                    }
                }
            }
            None
        } else {
            None
        }
    }

    async fn handle_bus_message(msg: &gst::Message, state: Arc<RwLock<PlayerState>>) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(_) => {
                info!("GStreamerPlayer - Bus message: End of stream");
                let mut state = state.write().await;
                *state = PlayerState::Stopped;
            }
            MessageView::Error(err) => {
                error!(
                    "GStreamerPlayer - Bus error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                let mut state = state.write().await;
                *state = PlayerState::Error;
            }
            MessageView::StateChanged(state_changed) => {
                if state_changed
                    .src()
                    .map(|s| s == state_changed.src().unwrap())
                    .unwrap_or(false)
                {
                    debug!(
                        "GStreamerPlayer - State changed from {:?} to {:?}",
                        state_changed.old(),
                        state_changed.current()
                    );
                }
            }
            MessageView::Buffering(buffering) => {
                let percent = buffering.percent();
                debug!("GStreamerPlayer - Buffering: {}%", percent);
            }
            _ => {}
        }
    }

    pub async fn get_state(&self) -> PlayerState {
        self.state.read().await.clone()
    }

    pub async fn get_audio_tracks(&self) -> Vec<(i32, String)> {
        let mut tracks = Vec::new();

        if let Some(playbin) = self.playbin.borrow().as_ref() {
            // Check playbin state - tracks might not be available until PLAYING
            // On macOS, we need to wait a bit for state to settle
            let timeout = if cfg!(target_os = "macos") {
                gst::ClockTime::from_mseconds(100)
            } else {
                gst::ClockTime::ZERO
            };
            let (_, current, _) = playbin.state(timeout);
            debug!("Getting audio tracks, playbin state: {:?}", current);

            // Check if this is playbin3 (doesn't have n-audio property)
            let is_playbin3 = *self.is_playbin3.borrow();

            let n_audio = if is_playbin3 {
                // playbin3 doesn't expose track count directly
                // We need to wait for PAUSED/PLAYING state for stream collection
                if current < gst::State::Paused {
                    debug!("playbin3 not in PAUSED/PLAYING state yet, can't get tracks");
                    return tracks;
                }

                // For now, try a reasonable maximum
                // TODO: Use GstStreamCollection API when available
                let mut count = 0;
                for i in 0..10 {
                    // Try to select the stream to see if it exists
                    if playbin
                        .emit_by_name::<Option<gst::TagList>>("get-audio-tags", &[&i])
                        .is_some()
                    {
                        count = i + 1;
                    } else {
                        break;
                    }
                }
                debug!("playbin3: Found {} audio tracks by probing", count);
                count
            } else if playbin.has_property("n-audio") {
                // Regular playbin
                let count = playbin.property::<i32>("n-audio");
                debug!("Got n-audio: {}", count);
                count
            } else {
                warn!("No audio track count property found!");
                0
            };
            info!("Found {} audio tracks", n_audio);

            for i in 0..n_audio {
                // Get audio stream tags - try different signal names for compatibility
                let tags = if playbin.has_property("n-audio-streams") {
                    // playbin3 uses get-audio-stream-tags
                    playbin.emit_by_name::<Option<gst::TagList>>("get-audio-stream-tags", &[&i])
                } else {
                    // playbin uses get-audio-tags
                    playbin.emit_by_name::<Option<gst::TagList>>("get-audio-tags", &[&i])
                };

                if let Some(tags) = tags {
                    let mut title = format!("Audio Track {}", i + 1);

                    // Try to get language code
                    if let Some(lang) = tags.get::<gst::tags::LanguageCode>() {
                        let lang_str = lang.get();
                        title = format!("Audio {} ({})", i + 1, lang_str);
                    }

                    // Try to get title
                    if let Some(tag_title) = tags.get::<gst::tags::Title>() {
                        let title_str = tag_title.get();
                        title = title_str.to_string();
                    }

                    tracks.push((i, title));
                } else {
                    tracks.push((i, format!("Audio Track {}", i + 1)));
                }
            }
        }

        tracks
    }

    pub async fn get_subtitle_tracks(&self) -> Vec<(i32, String)> {
        let mut tracks = Vec::new();

        if let Some(playbin) = self.playbin.borrow().as_ref() {
            // Check playbin state - tracks might not be available until PLAYING
            // On macOS, we need to wait a bit for state to settle
            let timeout = if cfg!(target_os = "macos") {
                gst::ClockTime::from_mseconds(100)
            } else {
                gst::ClockTime::ZERO
            };
            let (_, current, _) = playbin.state(timeout);
            debug!("Getting subtitle tracks, playbin state: {:?}", current);

            // Check if this is playbin3
            let is_playbin3 = *self.is_playbin3.borrow();

            let n_text = if is_playbin3 {
                // playbin3 doesn't expose track count directly
                // We need to wait for PAUSED/PLAYING state for stream collection
                if current < gst::State::Paused {
                    debug!("playbin3 not in PAUSED/PLAYING state yet, can't get tracks");
                    // Still add None option
                    tracks.push((-1, "None".to_string()));
                    return tracks;
                }

                // For now, try a reasonable maximum
                // TODO: Use GstStreamCollection API when available
                let mut count = 0;
                for i in 0..10 {
                    // Try to get tags to see if track exists
                    if playbin
                        .emit_by_name::<Option<gst::TagList>>("get-text-tags", &[&i])
                        .is_some()
                    {
                        count = i + 1;
                    } else {
                        break;
                    }
                }
                debug!("playbin3: Found {} subtitle tracks by probing", count);
                count
            } else if playbin.has_property("n-text") {
                // Regular playbin
                let count = playbin.property::<i32>("n-text");
                debug!("Got n-text: {}", count);
                count
            } else {
                warn!("No subtitle track count property found!");
                0
            };
            info!("Found {} subtitle tracks", n_text);

            // Add "None" option
            tracks.push((-1, "None".to_string()));

            for i in 0..n_text {
                // Get subtitle stream tags - try different signal names for compatibility
                let tags = if playbin.has_property("n-text-streams") {
                    // playbin3 uses get-text-stream-tags
                    playbin.emit_by_name::<Option<gst::TagList>>("get-text-stream-tags", &[&i])
                } else {
                    // playbin uses get-text-tags
                    playbin.emit_by_name::<Option<gst::TagList>>("get-text-tags", &[&i])
                };

                if let Some(tags) = tags {
                    let mut title = format!("Subtitle {}", i + 1);

                    // Try to get language code
                    if let Some(lang) = tags.get::<gst::tags::LanguageCode>() {
                        let lang_str = lang.get();
                        title = format!("Subtitle {} ({})", i + 1, lang_str);
                    }

                    // Try to get title
                    if let Some(tag_title) = tags.get::<gst::tags::Title>() {
                        let title_str = tag_title.get();
                        title = title_str.to_string();
                    }

                    tracks.push((i, title));
                } else {
                    tracks.push((i, format!("Subtitle {}", i + 1)));
                }
            }
        }

        tracks
    }

    #[allow(dead_code)]
    pub async fn set_audio_track(&self, track_index: i32) -> Result<()> {
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            let is_playbin3 = *self.is_playbin3.borrow();

            if is_playbin3 {
                // playbin3 uses signals for track selection
                // emit select-stream signal
                // For now, we'll use the compatibility approach
                info!("Setting audio track on playbin3 to index {}", track_index);
                // TODO: Implement proper stream selection for playbin3
            } else if playbin.has_property("current-audio") {
                // playbin property
                playbin.set_property("current-audio", track_index);
                info!("Set current-audio to {}", track_index);
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn set_subtitle_track(&self, track_index: i32) -> Result<()> {
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            info!("Setting subtitle track to index: {}", track_index);

            if track_index < 0 {
                // Disable subtitles
                playbin.set_property_from_str(
                    "flags",
                    "soft-colorbalance+deinterlace+soft-volume+audio+video",
                );
                info!("Disabled subtitles - flags set to audio+video only");
            } else {
                // Enable subtitles and set track
                playbin.set_property_from_str(
                    "flags",
                    "soft-colorbalance+deinterlace+soft-volume+audio+video+text",
                );
                info!("Enabled subtitles - flags set to audio+video+text");

                let is_playbin3 = *self.is_playbin3.borrow();

                if is_playbin3 {
                    // playbin3 uses different approach for subtitle selection
                    info!(
                        "Setting subtitle track on playbin3 to index {}",
                        track_index
                    );
                    // TODO: Implement proper stream selection for playbin3
                } else if playbin.has_property("current-text") {
                    // playbin property
                    playbin.set_property("current-text", track_index);
                    info!("Set current-text to {} (playbin)", track_index);
                } else {
                    error!("No subtitle track property available!");
                }
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn get_current_audio_track(&self) -> i32 {
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            if playbin.has_property("current-audio-stream") {
                // playbin3 property
                playbin.property::<i32>("current-audio-stream")
            } else if playbin.has_property("current-audio") {
                // playbin property
                playbin.property::<i32>("current-audio")
            } else {
                -1
            }
        } else {
            -1
        }
    }

    #[allow(dead_code)]
    pub async fn get_current_subtitle_track(&self) -> i32 {
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            // Check if we have any subtitle tracks available
            let n_text = if playbin.has_property("n-text-streams") {
                playbin.property::<i32>("n-text-streams")
            } else if playbin.has_property("n-text") {
                playbin.property::<i32>("n-text")
            } else {
                0
            };
            if n_text <= 0 {
                return -1; // No subtitle tracks available
            }

            // Get the current subtitle track
            // If subtitles are disabled, this will return -1
            if playbin.has_property("current-text-stream") {
                // playbin3 property
                playbin.property::<i32>("current-text-stream")
            } else if playbin.has_property("current-text") {
                // playbin property
                playbin.property::<i32>("current-text")
            } else {
                -1
            }
        } else {
            -1
        }
    }
}
