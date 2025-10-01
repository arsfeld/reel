use crate::player::ZoomMode;
use anyhow::{Context, Result};
use gdk4 as gdk;
use gstreamer as gst;
use gstreamer::glib;
use gstreamer::prelude::*;
use gtk4::{self, prelude::*};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace, warn};

#[derive(Debug, Clone)]
pub enum PlayerState {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopped,
    Error,
}

#[derive(Debug, Clone)]
struct StreamInfo {
    stream_id: String,
    stream_type: gst::StreamType,
    tags: Option<gst::TagList>,
    caps: Option<gst::Caps>,
    index: i32,
    language: Option<String>,
    codec: Option<String>,
}

pub struct GStreamerPlayer {
    playbin: Arc<Mutex<Option<gst::Element>>>,
    state: Arc<RwLock<PlayerState>>,
    video_sink: Arc<Mutex<Option<gst::Element>>>,
    zoom_mode: Arc<Mutex<ZoomMode>>,
    video_widget: Arc<Mutex<Option<gtk4::Widget>>>,
    stream_collection: Arc<Mutex<Option<gst::StreamCollection>>>,
    audio_streams: Arc<Mutex<Vec<StreamInfo>>>,
    subtitle_streams: Arc<Mutex<Vec<StreamInfo>>>,
    current_audio_stream: Arc<Mutex<Option<String>>>,
    current_subtitle_stream: Arc<Mutex<Option<String>>>,
}

impl GStreamerPlayer {
    pub fn new() -> Result<Self> {
        debug!("Initializing GStreamer player");

        // Initialize GStreamer if not already done
        match gst::init() {
            Ok(_) => debug!("GStreamer initialized successfully"),
            Err(e) => {
                error!(
                    "GStreamerPlayer::new() - Failed to initialize GStreamer: {}",
                    e
                );
                return Err(anyhow::anyhow!("Failed to initialize GStreamer: {}", e));
            }
        }

        // On macOS, prioritize curlhttpsrc over souphttpsrc for HTTPS support
        #[cfg(target_os = "macos")]
        {
            Self::configure_macos_http_source_priority();
        }

        // Check for required elements
        Self::check_gstreamer_plugins();

        Ok(Self {
            playbin: Arc::new(Mutex::new(None)),
            state: Arc::new(RwLock::new(PlayerState::Idle)),
            video_sink: Arc::new(Mutex::new(None)),
            zoom_mode: Arc::new(Mutex::new(ZoomMode::default())),
            video_widget: Arc::new(Mutex::new(None)),
            stream_collection: Arc::new(Mutex::new(None)),
            audio_streams: Arc::new(Mutex::new(Vec::new())),
            subtitle_streams: Arc::new(Mutex::new(Vec::new())),
            current_audio_stream: Arc::new(Mutex::new(None)),
            current_subtitle_stream: Arc::new(Mutex::new(None)),
        })
    }

    #[cfg(target_os = "macos")]
    fn configure_macos_http_source_priority() {
        // Get the GStreamer registry
        let registry = gst::Registry::get();

        // Prioritize curlhttpsrc over souphttpsrc for better TLS support on macOS
        if let Some(curl_feature) =
            registry.find_feature("curlhttpsrc", gst::ElementFactory::static_type())
        {
            curl_feature.set_rank(gst::Rank::PRIMARY + 100);
            info!("Set curlhttpsrc to higher rank for macOS HTTPS support");
        } else {
            warn!("curlhttpsrc not found - HTTPS streaming may have issues on macOS");
        }
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
                debug!("{} available (rank: {})", element, factory.rank());
            } else {
                error!("{} NOT available", element);
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
        debug!("Starting video widget creation");

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
        *self.video_sink.lock().unwrap() = video_sink;

        // Store and return the widget
        let widget = picture.upcast::<gtk4::Widget>();
        *self.video_widget.lock().unwrap() = Some(widget.clone());
        debug!("Video widget creation complete");
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

    fn create_gtk4_sink_with_conversion(&self) -> Option<gst::Element> {
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
        info!("Loading media: {}", url);
        debug!("GStreamerPlayer::load_media() - Full URL: {}", url);

        // Update state
        {
            let mut state = self.state.write().await;
            *state = PlayerState::Loading;
            debug!("GStreamerPlayer::load_media() - State set to Loading");
        }

        // Clear existing playbin if any
        if let Some(old_playbin) = self.playbin.lock().unwrap().as_ref() {
            debug!("GStreamerPlayer::load_media() - Clearing existing playbin");
            old_playbin
                .set_state(gst::State::Null)
                .context("Failed to set old playbin to null state")?;
        }

        // Clear stream collections from previous media
        {
            *self.stream_collection.lock().unwrap() = None;
            self.audio_streams.lock().unwrap().clear();
            self.subtitle_streams.lock().unwrap().clear();
            *self.current_audio_stream.lock().unwrap() = None;
            *self.current_subtitle_stream.lock().unwrap() = None;
            debug!("GStreamerPlayer::load_media() - Cleared previous stream collections");
        }

        // Create playbin3 element - NEVER fallback
        debug!("Creating playbin3 element");
        let playbin = gst::ElementFactory::make("playbin3")
            .name("player")
            .property("uri", url)
            .build()
            .context("Failed to create playbin3 element - GStreamer plugins may not be properly installed")?;

        debug!("Successfully created playbin3");

        // Verify we're using playbin3
        if let Some(factory) = playbin.factory() {
            info!(
                "Using element: {} (factory: {})",
                playbin.name(),
                factory.name()
            );
            assert_eq!(
                factory.name(),
                "playbin3",
                "Must use playbin3, no fallbacks!"
            );
        }

        // Enable all features including text overlay
        playbin.set_property_from_str(
            "flags",
            "soft-colorbalance+deinterlace+soft-volume+audio+video+text",
        );

        // playbin3-specific configuration
        info!("Configuring playbin3-specific settings...");
        // playbin3 has QoS always enabled, no property needed

        // Check available properties and signals for debugging
        let properties = playbin.list_properties();
        debug!("Available playbin3 properties related to streams:");
        for prop in properties {
            let name = prop.name();
            if name.contains("stream")
                || name.contains("collection")
                || name.contains("track")
                || name == "flags"
            {
                debug!("  - {}: {:?}", name, prop.type_());
            }
        }

        // Also check what signals are available
        debug!("Checking available playbin3 signals...");
        // Note: GStreamer doesn't have a direct way to list signals via Rust bindings
        // but we know playbin3 should emit stream-collection on the bus

        // Configure subtitle properties if available
        if playbin.has_property("subtitle-encoding") {
            playbin.set_property_from_str("subtitle-encoding", "UTF-8");
            info!("Set subtitle encoding to UTF-8");
        }

        if playbin.has_property("subtitle-font-desc") {
            playbin.set_property_from_str("subtitle-font-desc", "Sans, 18");
            info!("Set subtitle font to Sans, 18");
        }

        debug!("Using playbin3");

        // Use our stored video sink if available
        if let Some(sink) = self.video_sink.lock().unwrap().as_ref() {
            debug!("GStreamerPlayer::load_media() - Setting video sink on playbin");
            playbin.set_property("video-sink", sink);
            debug!("Video sink configured");
        } else {
            // On macOS, sometimes it's better to let playbin choose its own sink
            #[cfg(target_os = "macos")]
            {
                info!(
                    "GStreamerPlayer::load_media() - No pre-configured sink on macOS, letting playbin auto-select"
                );
                // Don't set any video-sink, let playbin use autovideosink internally
            }

            #[cfg(not(target_os = "macos"))]
            {
                // Create a fallback video sink on other platforms
                debug!("No pre-configured sink, creating fallback");

                if let Some(fallback_sink) = self.create_auto_fallback_sink() {
                    playbin.set_property("video-sink", &fallback_sink);
                    debug!("Fallback video sink configured");
                } else {
                    error!(
                        "GStreamerPlayer::load_media() - Failed to create any fallback video sink!"
                    );
                }
            }
        }

        // Store the playbin
        *self.playbin.lock().unwrap() = Some(playbin.clone());
        debug!("GStreamerPlayer::load_media() - Playbin stored");

        // Print the pipeline in gst-launch format
        self.print_gst_launch_pipeline(&playbin, url);

        // Debug: Log the complete pipeline structure
        if let Some(sink) = playbin.property::<Option<gst::Element>>("video-sink") {
            debug!("video-sink is attached: {:?}", sink.name());
            // Try to inspect the sink structure
            if let Some(bin) = sink.dynamic_cast_ref::<gst::Bin>() {
                debug!("video-sink is a bin, contains:");
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

        // Set up message handling with enhanced logging
        let bus = playbin.bus().context("Failed to get playbin bus")?;
        debug!("GStreamerPlayer::load_media() - Got playbin bus");

        let state_clone = self.state.clone();
        let stream_collection_clone = self.stream_collection.clone();
        let audio_streams_clone = self.audio_streams.clone();
        let subtitle_streams_clone = self.subtitle_streams.clone();
        let current_audio_clone = self.current_audio_stream.clone();
        let current_subtitle_clone = self.current_subtitle_stream.clone();
        let playbin_clone = self.playbin.clone();

        info!("Setting up bus sync handler for immediate message processing...");

        bus.set_sync_handler(move |_, msg| {
            // Log immediately when any message arrives
            let msg_type = msg.type_();
            if !matches!(msg_type, gst::MessageType::Qos | gst::MessageType::Progress) {
                let src_name = msg
                    .src()
                    .map(|s| s.name().to_string())
                    .unwrap_or_else(|| "unknown".to_string());

                // Highlight specific message types we're interested in
                if matches!(msg_type, gst::MessageType::StreamCollection) {
                    info!(
                        "Stream collection message (sync): {:?} from {}",
                        msg_type, src_name
                    );
                } else if matches!(msg_type, gst::MessageType::StreamsSelected) {
                    info!(
                        "Streams selected message (sync): {:?} from {}",
                        msg_type, src_name
                    );
                } else if matches!(msg_type, gst::MessageType::StreamStart) {
                    info!(
                        "Stream start message (sync): {:?} from {}",
                        msg_type, src_name
                    );
                } else if matches!(
                    msg_type,
                    gst::MessageType::StateChanged
                        | gst::MessageType::Tag
                        | gst::MessageType::StreamStatus
                ) {
                    // Skip StateChanged, Tag, and StreamStatus messages to reduce noise
                } else {
                    trace!("Bus message (sync): {:?} from {}", msg_type, src_name);
                }
            }

            let state = state_clone.clone();
            let stream_collection = stream_collection_clone.clone();
            let audio_streams = audio_streams_clone.clone();
            let subtitle_streams = subtitle_streams_clone.clone();
            let current_audio = current_audio_clone.clone();
            let current_subtitle = current_subtitle_clone.clone();
            let playbin = playbin_clone.clone();
            let msg = msg.clone();

            // Handle message synchronously to avoid context issues
            Self::handle_bus_message_sync(
                &msg,
                &state,
                &stream_collection,
                &audio_streams,
                &subtitle_streams,
                &current_audio,
                &current_subtitle,
                &playbin,
            );

            gst::BusSyncReply::Pass
        });

        info!("Bus sync handler set up successfully");

        // Preroll the pipeline to PAUSED state to get video dimensions early
        // This allows the video widget to resize before playback starts
        debug!("GStreamerPlayer::load_media() - Prerolling pipeline to get dimensions");
        match playbin.set_state(gst::State::Paused) {
            Ok(gst::StateChangeSuccess::Success) => {
                debug!("Pipeline prerolled successfully");

                // Check bus for any pending messages
                info!("Checking for pending bus messages after preroll...");
                let mut message_count = 0;
                while let Some(msg) = bus.pop() {
                    message_count += 1;
                    let msg_type = msg.type_();
                    let src_name = msg
                        .src()
                        .map(|s| s.name().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    info!(
                        "  Found pending message #{}: {:?} from {}",
                        message_count, msg_type, src_name
                    );

                    // Check specifically for stream collection
                    if matches!(msg_type, gst::MessageType::StreamCollection) {
                        debug!("Found StreamCollection message in pending queue");
                    }
                }
                info!("Total pending messages found: {}", message_count);
            }
            Ok(gst::StateChangeSuccess::Async) => {
                debug!("Pipeline prerolling asynchronously");
                // The AsyncDone message will signal when preroll is complete
            }
            Ok(gst::StateChangeSuccess::NoPreroll) => {
                debug!("Live pipeline, no preroll needed");
            }
            Err(e) => {
                warn!(
                    "GStreamerPlayer::load_media() - Failed to start preroll: {:?}",
                    e
                );
            }
        }

        debug!("GStreamerPlayer::load_media() - Playbin configured, ready for playback");

        // Configure playbin for better network streaming
        {
            // Set connection-speed for better buffering decisions
            if playbin.has_property("connection-speed") {
                playbin.set_property("connection-speed", 10000u64); // 10 Mbps
                debug!("Set playbin connection-speed for better buffering");
            }

            // Increase buffer size for network streams
            if playbin.has_property("buffer-size") {
                playbin.set_property("buffer-size", 10 * 1024 * 1024i32); // 10MB
                debug!("Set playbin buffer-size to 10MB");
            }

            // Set buffer duration for smoother playback
            if playbin.has_property("buffer-duration") {
                playbin.set_property(
                    "buffer-duration",
                    gst::ClockTime::from_seconds(10).nseconds() as i64,
                );
                debug!("Set playbin buffer-duration to 10 seconds");
            }
        }

        debug!("Media loading complete");
        Ok(())
    }

    pub async fn play(&self) -> Result<()> {
        info!("Starting playback");

        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            // On macOS, ensure we transition through states properly
            #[cfg(target_os = "macos")]
            {
                // Get current state
                let (_, current, _) = playbin.state(gst::ClockTime::ZERO);
                info!(
                    "GStreamerPlayer::play() - Current state before play: {:?}",
                    current
                );

                // If we're in Null state, first go to Ready, then Paused, then Playing
                if current == gst::State::Null {
                    debug!("Transitioning from Null -> Ready");
                    match playbin.set_state(gst::State::Ready) {
                        Ok(_) => {
                            // Wait for state change to complete
                            let (state_result, new_state, _) =
                                playbin.state(gst::ClockTime::from_seconds(1));
                            match state_result {
                                Ok(_) => debug!("Transitioned to Ready state: {:?}", new_state),
                                Err(_) => warn!("Failed to transition to Ready state"),
                            }
                        }
                        Err(e) => {
                            error!("Failed to set Ready state: {:?}", e);
                            return Err(anyhow::anyhow!("Failed to set Ready state"));
                        }
                    }

                    // Now go to Paused to allow preroll
                    info!(
                        "GStreamerPlayer::play() - Transitioning from Ready -> Paused for preroll"
                    );
                    match playbin.set_state(gst::State::Paused) {
                        Ok(gst::StateChangeSuccess::Success) => {
                            info!("Successfully transitioned to Paused");
                        }
                        Ok(gst::StateChangeSuccess::Async) => {
                            info!("Async transition to Paused, waiting for preroll...");
                            // Wait for preroll to complete
                            let (state_result, new_state, _) =
                                playbin.state(gst::ClockTime::from_seconds(5));
                            match state_result {
                                Ok(_) => debug!("Preroll complete, now in {:?}", new_state),
                                Err(_) => {
                                    warn!("Preroll timeout, checking for errors...");
                                    // Check bus for errors
                                    if let Some(bus) = playbin.bus() {
                                        while let Some(msg) = bus.pop() {
                                            use gst::MessageView;
                                            if let MessageView::Error(err) = msg.view() {
                                                error!(
                                                    "Bus error during preroll: {} ({:?})",
                                                    err.error(),
                                                    err.debug()
                                                );
                                                return Err(anyhow::anyhow!(
                                                    "Preroll failed: {}",
                                                    err.error()
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        _ => {
                            warn!("Failed to transition to Paused for preroll");
                        }
                    }
                }
            }

            debug!("Setting playbin to Playing state");
            match playbin.set_state(gst::State::Playing) {
                Ok(gst::StateChangeSuccess::Success) => {
                    debug!("Successfully set playbin to playing state");

                    // Update internal state to Playing
                    let mut state = self.state.write().await;
                    *state = PlayerState::Playing;

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
                        debug!("Dumped PLAYING state pipeline to /tmp/reel-playbin-PLAYING.dot");

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
                    debug!("Playbin state change is async, waiting...");

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
                                // Update state if we successfully reached Playing
                                if current == gst::State::Playing {
                                    let mut state = self.state.write().await;
                                    *state = PlayerState::Playing;
                                }
                            }
                            _ => {
                                warn!(
                                    "GStreamerPlayer::play() - Async state change still pending after 3s, current: {:?}",
                                    current
                                );
                            }
                        }
                    }

                    // For non-macOS, the state will be updated by the bus message handler

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
                    debug!("Playbin state change: no preroll");
                    // Live sources don't need preroll, update state immediately
                    let mut state = self.state.write().await;
                    *state = PlayerState::Playing;
                }
                Err(gst::StateChangeError) => {
                    // Get more details about the error
                    let (state_result, current, pending) =
                        playbin.state(gst::ClockTime::from_seconds(1));
                    error!("GStreamerPlayer::play() - Failed to set playbin to playing state");
                    error!(
                        "GStreamerPlayer::play() - State result: {:?}, Current: {:?}, Pending: {:?}",
                        state_result, current, pending
                    );

                    // Try to recover by resetting the pipeline
                    warn!("GStreamerPlayer::play() - Attempting pipeline recovery...");

                    // First, try to go back to Null state
                    if let Err(e) = playbin.set_state(gst::State::Null) {
                        error!(
                            "GStreamerPlayer::play() - Failed to reset to Null state: {:?}",
                            e
                        );
                    } else {
                        // Wait for null state to be reached
                        let _ = playbin.state(gst::ClockTime::from_seconds(1));

                        // Try once more with a simpler approach
                        info!(
                            "GStreamerPlayer::play() - Retrying playback with direct Playing state"
                        );
                        match playbin.set_state(gst::State::Playing) {
                            Ok(_) => {
                                info!(
                                    "GStreamerPlayer::play() - Recovery successful, playback started"
                                );
                                let mut state = self.state.write().await;
                                *state = PlayerState::Playing;
                                return Ok(());
                            }
                            Err(_) => {
                                error!("GStreamerPlayer::play() - Recovery failed");
                            }
                        }
                    }

                    // Get the bus to check for error messages
                    let mut error_details = Vec::new();
                    if let Some(bus) = playbin.bus() {
                        while let Some(msg) = bus.pop() {
                            use gst::MessageView;
                            match msg.view() {
                                MessageView::Error(err) => {
                                    let error_msg = format!(
                                        "{} (from: {:?})",
                                        err.error(),
                                        err.src().map(|s| s.path_string())
                                    );
                                    error!(
                                        "GStreamerPlayer::play() - Bus error: {} ({:?})",
                                        err.error(),
                                        err.debug()
                                    );
                                    error_details.push(error_msg);
                                }
                                MessageView::Warning(warn) => {
                                    warn!(
                                        "GStreamerPlayer::play() - Bus warning: {} ({:?})",
                                        warn.error(),
                                        warn.debug()
                                    );
                                }
                                _ => {}
                            }
                        }
                    }

                    // Update state to Error
                    let mut state = self.state.write().await;
                    *state = PlayerState::Error;

                    let error_msg = if !error_details.is_empty() {
                        format!("Failed to play media: {}", error_details.join("; "))
                    } else {
                        "Failed to set playbin to playing state (no specific error available)"
                            .to_string()
                    };

                    return Err(anyhow::anyhow!(error_msg));
                }
            }
            debug!("Player state set to Playing");
        } else {
            error!("GStreamerPlayer::play() - No playbin available!");
            return Err(anyhow::anyhow!("No playbin available"));
        }
        debug!("Playback started");
        Ok(())
    }

    pub async fn pause(&self) -> Result<()> {
        debug!("Pausing playback");

        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
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

        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
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

        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            // Get current pipeline state
            let (state_result, current_state, pending_state) =
                playbin.state(gst::ClockTime::from_mseconds(100));

            debug!(
                "GStreamerPlayer::seek() - Current state: {:?}, Pending: {:?}, Result: {:?}",
                current_state, pending_state, state_result
            );

            // Ensure pipeline is at least in PAUSED state for seeking to work
            if current_state < gst::State::Paused {
                info!(
                    "GStreamerPlayer::seek() - Pipeline in {:?} state, need to reach PAUSED for seeking",
                    current_state
                );

                // Try to set to PAUSED state
                match playbin.set_state(gst::State::Paused) {
                    Ok(gst::StateChangeSuccess::Success) => {
                        debug!("Successfully reached PAUSED state");
                    }
                    Ok(gst::StateChangeSuccess::Async) => {
                        debug!("Waiting for PAUSED state...");
                        // Wait for state change to complete (max 2 seconds)
                        let (wait_result, new_state, _) =
                            playbin.state(gst::ClockTime::from_seconds(2));
                        match wait_result {
                            Ok(_) => {
                                debug!("Reached {:?} state", new_state);
                                if new_state < gst::State::Paused {
                                    warn!(
                                        "GStreamerPlayer::seek() - Failed to reach PAUSED state for seeking"
                                    );
                                    // Continue anyway, seeking might still work
                                }
                            }
                            Err(_) => {
                                warn!("GStreamerPlayer::seek() - Timeout waiting for PAUSED state");
                                // Continue anyway, seeking might still work
                            }
                        }
                    }
                    Ok(gst::StateChangeSuccess::NoPreroll) => {
                        debug!("Live source, no preroll needed");
                    }
                    Err(_) => {
                        error!("GStreamerPlayer::seek() - Failed to set PAUSED state for seeking");
                        // Continue anyway, seeking might still work
                    }
                }
            }

            // Perform the seek operation
            // Use FLUSH to clear buffers and get immediate response
            // Use SNAP_BEFORE for HTTP streams - safer for matroska over HTTP
            let seek_flags = gst::SeekFlags::FLUSH | gst::SeekFlags::SNAP_BEFORE;
            let seek_position = gst::ClockTime::from_nseconds(position_ns as u64);

            debug!(
                "GStreamerPlayer::seek() - Attempting seek to {} with flags {:?}",
                seek_position.display(),
                seek_flags
            );

            // Try seek_simple first (simpler API)
            let seek_result = playbin.seek_simple(seek_flags, seek_position);

            if seek_result.is_err() {
                // If seek_simple fails, try the full seek API with more control
                warn!("GStreamerPlayer::seek() - seek_simple failed, trying full seek API");

                let seek_result = playbin.seek(
                    1.0, // rate (normal speed)
                    seek_flags,
                    gst::SeekType::Set,
                    seek_position,
                    gst::SeekType::None,
                    gst::ClockTime::NONE,
                );

                if seek_result.is_err() {
                    error!("GStreamerPlayer::seek() - Both seek methods failed");

                    // Check if media is seekable
                    let mut query = gst::query::Seeking::new(gst::Format::Time);
                    if playbin.query(&mut query) {
                        let (seekable, start, end) = query.result();
                        error!(
                            "GStreamerPlayer::seek() - Media seekable: {}, range: {:?} - {:?}",
                            seekable, start, end
                        );

                        if !seekable {
                            return Err(anyhow::anyhow!("Media is not seekable"));
                        }
                    }

                    // Check for errors on the bus
                    if let Some(bus) = playbin.bus() {
                        while let Some(msg) = bus.pop() {
                            use gst::MessageView;
                            if let MessageView::Error(err) = msg.view() {
                                error!(
                                    "GStreamerPlayer::seek() - Bus error: {} ({:?})",
                                    err.error(),
                                    err.debug()
                                );
                            }
                        }
                    }

                    return Err(anyhow::anyhow!("Failed to seek to position {:?}", position));
                } else {
                    debug!("Seek succeeded using full API");
                }
            } else {
                debug!("Seek succeeded using seek_simple");
            }

            // If we were playing before, resume playback
            let state_before = self.state.read().await.clone();
            if matches!(state_before, PlayerState::Playing) {
                debug!("GStreamerPlayer::seek() - Resuming playback after seek");
                match playbin.set_state(gst::State::Playing) {
                    Ok(_) => {
                        debug!("GStreamerPlayer::seek() - Resumed playing state");
                    }
                    Err(e) => {
                        warn!(
                            "GStreamerPlayer::seek() - Failed to resume playing after seek: {:?}",
                            e
                        );
                    }
                }
            }
        } else {
            return Err(anyhow::anyhow!("No playbin available for seeking"));
        }

        Ok(())
    }

    pub async fn get_position(&self) -> Option<Duration> {
        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            playbin
                .query_position::<gst::ClockTime>()
                .map(|pos| Duration::from_nanos(pos.nseconds()))
        } else {
            None
        }
    }

    pub async fn get_duration(&self) -> Option<Duration> {
        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            playbin
                .query_duration::<gst::ClockTime>()
                .map(|dur| Duration::from_nanos(dur.nseconds()))
        } else {
            None
        }
    }

    pub async fn set_volume(&self, volume: f64) -> Result<()> {
        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            playbin.set_property("volume", volume);
        }
        Ok(())
    }

    pub async fn get_video_dimensions(&self) -> Option<(i32, i32)> {
        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            // Ensure pipeline is at least in PAUSED state for dimensions to be available
            let (_, current, _) = playbin.state(gst::ClockTime::ZERO);
            if current < gst::State::Paused {
                debug!(
                    "GStreamerPlayer::get_video_dimensions() - Pipeline not yet in PAUSED state, attempting to reach it"
                );

                // Try to transition to PAUSED state to get dimensions
                match playbin.set_state(gst::State::Paused) {
                    Ok(gst::StateChangeSuccess::Success) => {
                        debug!("Successfully reached PAUSED state");
                    }
                    Ok(gst::StateChangeSuccess::Async) => {
                        // Wait for async state change to complete (max 2 seconds)
                        match playbin.state(gst::ClockTime::from_seconds(2)) {
                            (Ok(_), new_state, _) => {
                                debug!("Reached state: {:?}", new_state);
                            }
                            _ => {
                                warn!("Timeout waiting for PAUSED state");
                                return None;
                            }
                        }
                    }
                    _ => {
                        warn!("Failed to set PAUSED state for dimension detection");
                        return None;
                    }
                }
            }
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

            // For playbin3, video dimensions should be available from the video sink pad
            // TODO: Implement stream collection API for getting video stream info
            None
        } else {
            None
        }
    }

    fn handle_bus_message_sync(
        msg: &gst::Message,
        state: &Arc<RwLock<PlayerState>>,
        stream_collection: &Arc<Mutex<Option<gst::StreamCollection>>>,
        audio_streams: &Arc<Mutex<Vec<StreamInfo>>>,
        subtitle_streams: &Arc<Mutex<Vec<StreamInfo>>>,
        current_audio: &Arc<Mutex<Option<String>>>,
        current_subtitle: &Arc<Mutex<Option<String>>>,
        playbin: &Arc<Mutex<Option<gst::Element>>>,
    ) {
        use gst::MessageView;

        // Log all messages for debugging
        let msg_type = msg.type_();
        if !matches!(msg_type, gst::MessageType::Qos | gst::MessageType::Progress) {
            let src_name = msg
                .src()
                .map(|s| s.name().to_string())
                .unwrap_or_else(|| "unknown".to_string());

            // Highlight specific message types we're interested in
            if matches!(msg_type, gst::MessageType::StreamCollection) {
                debug!("Stream collection message from {}", src_name);
            } else if matches!(msg_type, gst::MessageType::StreamsSelected) {
                debug!("Streams selected message from {}", src_name);
            } else if matches!(msg_type, gst::MessageType::StreamStart) {
                debug!("Stream start message from {}", src_name);
            } else {
                trace!("Bus message: {:?} from {}", msg_type, src_name);
            }
        }

        match msg.view() {
            MessageView::Eos(_) => {
                info!("GStreamerPlayer - Bus message: End of stream");
                if let Ok(mut state_guard) = state.try_write() {
                    *state_guard = PlayerState::Stopped;
                }
            }
            MessageView::Error(err) => {
                error!(
                    "GStreamerPlayer - Bus error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                if let Ok(mut state_guard) = state.try_write() {
                    *state_guard = PlayerState::Error;
                }
            }
            MessageView::StateChanged(state_changed) => {
                // Only handle state changes from the playbin element itself
                if let Some(src) = state_changed.src() {
                    let element_name = src.name();
                    if element_name.starts_with("playbin") {
                        let new_state = state_changed.current();
                        let old_state = state_changed.old();

                        debug!(
                            "GStreamerPlayer - Playbin state changed from {:?} to {:?}",
                            old_state, new_state
                        );

                        // Update internal state based on pipeline state
                        if let Ok(mut state_guard) = state.try_write() {
                            match new_state {
                                gst::State::Playing => {
                                    *state_guard = PlayerState::Playing;
                                    debug!("State updated to Playing");
                                }
                                gst::State::Paused => {
                                    // Only set to Paused if we're not in Loading state
                                    // (Loading state transitions through Paused)
                                    if !matches!(*state_guard, PlayerState::Loading) {
                                        *state_guard = PlayerState::Paused;
                                        debug!("State updated to Paused");
                                    }
                                }
                                gst::State::Ready | gst::State::Null => {
                                    // Only update to Stopped if we're not in Error or Loading state
                                    if !matches!(
                                        *state_guard,
                                        PlayerState::Error | PlayerState::Loading
                                    ) {
                                        *state_guard = PlayerState::Stopped;
                                        debug!("State updated to Stopped");
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            MessageView::Buffering(buffering) => {
                let percent = buffering.percent();
                debug!("GStreamerPlayer - Buffering: {}%", percent);
            }
            MessageView::AsyncDone(_) => {
                info!(
                    "GStreamerPlayer - AsyncDone: Pipeline ready, dimensions should be available"
                );

                // Check if we have received a stream collection yet
                let has_collection = stream_collection
                    .lock()
                    .map(|guard| guard.is_some())
                    .unwrap_or(false);

                if !has_collection {
                    info!(
                        "⚠️  AsyncDone but no StreamCollection received yet - this might indicate an issue"
                    );

                    // Try to manually query for stream information
                    if let Ok(Some(pb)) = playbin.lock().map(|p| p.as_ref().cloned()) {
                        info!("Attempting to manually query stream information from playbin3...");

                        // Check if there are any stream-related signals we can query
                        let props = pb.list_properties();
                        for prop in props {
                            let name = prop.name();
                            if name.starts_with("n-")
                                && (name.contains("audio")
                                    || name.contains("video")
                                    || name.contains("text"))
                            {
                                let value = pb.property_value(name);
                                info!("  {}: {:?}", name, value);
                            }
                        }
                    }
                } else {
                    debug!("StreamCollection already received");
                }
            }
            MessageView::StreamCollection(collection_msg) => {
                let collection = collection_msg.stream_collection();
                debug!(
                    "Stream collection received with {} streams",
                    collection.len()
                );

                // Process the collection synchronously
                Self::process_stream_collection_sync(
                    &collection,
                    stream_collection,
                    audio_streams,
                    subtitle_streams,
                );

                // Send default stream selection
                if let Ok(Some(pb)) = playbin.lock().map(|p| p.as_ref().cloned()) {
                    Self::send_default_stream_selection(&collection, &pb);
                }
            }
            MessageView::StreamsSelected(selected_msg) => {
                debug!("Processing streams selected message");

                // Get the collection from the message
                let collection = selected_msg.stream_collection();
                debug!("StreamsSelected: {} total streams", collection.len());

                // Clear current selections
                if let Ok(mut audio_guard) = current_audio.try_lock() {
                    *audio_guard = None;
                }
                if let Ok(mut subtitle_guard) = current_subtitle.try_lock() {
                    *subtitle_guard = None;
                }

                // Count and categorize streams for summary
                let mut video_count = 0;
                let mut audio_count = 0;
                let mut subtitle_count = 0;
                let mut selected_audio: Option<String> = None;

                for i in 0..collection.len() {
                    let idx = i as u32;
                    if let Some(stream) = collection.stream(idx) {
                        let stream_id = stream
                            .stream_id()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("stream-{}", idx));
                        let stream_type = stream.stream_type();

                        if stream_type.contains(gst::StreamType::VIDEO) {
                            video_count += 1;
                        } else if stream_type.contains(gst::StreamType::AUDIO) {
                            audio_count += 1;
                            // Mark first audio as selected
                            if selected_audio.is_none() {
                                selected_audio = Some(stream_id.clone());
                                if let Ok(mut guard) = current_audio.try_lock() {
                                    *guard = Some(stream_id.clone());
                                }
                            }
                        } else if stream_type.contains(gst::StreamType::TEXT) {
                            subtitle_count += 1;
                        }

                        trace!("Stream {}: id={}, type={:?}", idx, stream_id, stream_type);
                    }
                }

                info!(
                    "Stream selection: {} video, {} audio, {} subtitle tracks",
                    video_count, audio_count, subtitle_count
                );
                if let Some(audio_id) = &selected_audio {
                    debug!("Auto-selected audio stream: {}", audio_id);
                }

                // Store this collection as our stream collection if we don't have one yet
                if stream_collection
                    .lock()
                    .map(|guard| guard.is_none())
                    .unwrap_or(false)
                {
                    debug!("Using StreamsSelected collection as stream collection");
                    Self::process_stream_collection_sync(
                        &collection,
                        stream_collection,
                        audio_streams,
                        subtitle_streams,
                    );
                }
            }
            MessageView::Tag(tag_msg) => {
                // Tags might contain stream information - only log at trace level
                let tags = tag_msg.tags();
                // Only log language tags at trace level since they're very verbose
                if let Some(lang) = tags.index::<gst::tags::LanguageCode>(0) {
                    trace!("Found language tag: {}", lang.get());
                }
            }
            MessageView::StreamStart(_stream_start_msg) => {
                debug!("Stream started - collection should follow soon");

                // StreamStart message doesn't provide direct stream access
                // We'll rely on the StreamCollection message that follows
                info!("  StreamStart received - waiting for StreamCollection");

                // Check if this is from a decodebin - they should emit collections
                if let Some(src) = msg.src() {
                    let src_name = src.name();
                    info!("  StreamStart source: {}", src_name);
                    if src_name.to_string().contains("decodebin") {
                        info!(
                            "  ⚠️  StreamStart from decodebin - StreamCollection should have been sent!"
                        );
                    }
                }
            }
            MessageView::Element(elem_msg) => {
                // Some elements might send custom messages about streams
                if let Some(s) = elem_msg.structure() {
                    let name = s.name();
                    if name.contains("stream")
                        || name.contains("collection")
                        || name.contains("select")
                    {
                        info!("Element message: {}", name);
                    }
                }
            }
            _ => {
                // Log other potentially relevant unhandled messages
                let msg_type = msg.type_();
                if matches!(
                    msg_type,
                    gst::MessageType::SegmentStart
                        | gst::MessageType::SegmentDone
                        | gst::MessageType::DurationChanged
                        | gst::MessageType::Latency
                        | gst::MessageType::Toc
                        | gst::MessageType::StreamStatus
                ) {
                    trace!("Unhandled message: {:?}", msg_type);
                }
            }
        }
    }

    fn process_stream_collection_sync(
        collection: &gst::StreamCollection,
        stream_collection: &Arc<Mutex<Option<gst::StreamCollection>>>,
        audio_streams: &Arc<Mutex<Vec<StreamInfo>>>,
        subtitle_streams: &Arc<Mutex<Vec<StreamInfo>>>,
    ) {
        debug!("Processing stream collection...");

        // Store the collection
        if let Ok(mut guard) = stream_collection.try_lock() {
            *guard = Some(collection.clone());
        }

        // Parse and categorize streams
        let mut audio = Vec::new();
        let mut subtitles = Vec::new();
        let mut audio_index = 0;
        let mut subtitle_index = 0;
        let mut video_count = 0;

        for i in 0..collection.len() {
            let idx = i as u32;
            if let Some(stream) = collection.stream(idx) {
                let stream_id = stream
                    .stream_id()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("stream-{}", idx));
                let stream_type = stream.stream_type();
                let tags = stream.tags();
                let caps = stream.caps();

                // Extract language from tags if available
                let language = tags.as_ref().and_then(|t| {
                    t.index::<gst::tags::LanguageCode>(0)
                        .map(|val| val.get().to_string())
                });

                // Extract codec from caps if available
                let codec = caps
                    .as_ref()
                    .and_then(|c| c.structure(0).map(|s| s.name().to_string()));

                let stream_info = StreamInfo {
                    stream_id: stream_id.clone(),
                    stream_type,
                    tags,
                    caps,
                    index: 0,
                    language: language.clone(),
                    codec: codec.clone(),
                };

                if stream_type.contains(gst::StreamType::VIDEO) {
                    video_count += 1;
                    trace!("Found VIDEO stream: {} (codec: {:?})", stream_id, codec);
                } else if stream_type.contains(gst::StreamType::AUDIO) {
                    let mut info = stream_info;
                    info.index = audio_index;
                    audio_index += 1;
                    trace!(
                        "Found AUDIO stream #{}: {} (language: {:?})",
                        info.index, info.stream_id, info.language
                    );
                    audio.push(info);
                } else if stream_type.contains(gst::StreamType::TEXT) {
                    let mut info = stream_info;
                    info.index = subtitle_index;
                    subtitle_index += 1;
                    trace!(
                        "Found TEXT stream #{}: {} (language: {:?})",
                        info.index, info.stream_id, info.language
                    );
                    subtitles.push(info);
                } else {
                    trace!("Found OTHER stream type: {:?}", stream_type);
                }
            }
        }

        debug!(
            "Stream collection: {} video, {} audio, {} subtitle streams",
            video_count,
            audio.len(),
            subtitles.len()
        );

        // Update stored streams
        if let Ok(mut guard) = audio_streams.try_lock() {
            *guard = audio;
        }
        if let Ok(mut guard) = subtitle_streams.try_lock() {
            *guard = subtitles;
        }
    }

    fn send_default_stream_selection(collection: &gst::StreamCollection, playbin: &gst::Element) {
        let mut selected_streams = Vec::new();
        let mut has_video = false;
        let mut has_audio = false;

        debug!("Building default stream selection...");
        for i in 0..collection.len() {
            let idx = i as u32;
            if let Some(stream) = collection.stream(idx) {
                let stream_id = stream
                    .stream_id()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("stream-{}", idx));
                let stream_type = stream.stream_type();

                // Select first stream of each type by default
                if !has_video && stream_type.contains(gst::StreamType::VIDEO) {
                    trace!("Selecting video stream: {}", stream_id);
                    selected_streams.push(stream_id);
                    has_video = true;
                } else if !has_audio && stream_type.contains(gst::StreamType::AUDIO) {
                    trace!("Selecting audio stream: {}", stream_id);
                    selected_streams.push(stream_id);
                    has_audio = true;
                } else if stream_type.contains(gst::StreamType::TEXT) {
                    // Don't select text by default - trace level only
                    trace!(
                        "Skipping text stream: {} (not selected by default)",
                        stream_id
                    );
                }
            }
        }

        if !selected_streams.is_empty() {
            debug!(
                "Sending SELECT_STREAMS event with {} streams",
                selected_streams.len()
            );
            let stream_refs: Vec<&str> = selected_streams.iter().map(|s| s.as_str()).collect();
            let event = gst::event::SelectStreams::new(stream_refs.iter().copied());
            if playbin.send_event(event) {
                debug!("Successfully sent SELECT_STREAMS event");
            } else {
                error!("Failed to send SELECT_STREAMS event");
            }
        } else {
            warn!("No streams selected - this might cause playback issues");
        }
    }

    fn process_selected_streams_sync(
        _collection: &gst::StreamCollection,
        _current_audio: &Arc<Mutex<Option<String>>>,
        _current_subtitle: &Arc<Mutex<Option<String>>>,
    ) {
        info!("Processing selected streams...");
        // Placeholder for now - proper implementation would track which streams are actually selected
        // The StreamsSelected message doesn't directly tell us which streams are selected
        // We need to infer this from the context
    }

    fn process_stream_collection(
        collection: &gst::StreamCollection,
        stream_collection: &Arc<Mutex<Option<gst::StreamCollection>>>,
        audio_streams: &Arc<Mutex<Vec<StreamInfo>>>,
        subtitle_streams: &Arc<Mutex<Vec<StreamInfo>>>,
    ) {
        info!("Processing stream collection...");

        // Store the collection
        *stream_collection.lock().unwrap() = Some(collection.clone());

        // Parse and categorize streams
        let mut audio = Vec::new();
        let mut subtitles = Vec::new();
        let mut audio_index = 0;
        let mut subtitle_index = 0;
        let mut video_count = 0;

        for i in 0..collection.len() {
            let idx = i as u32;
            if let Some(stream) = collection.stream(idx) {
                let stream_id = stream
                    .stream_id()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("stream-{}", idx));
                let stream_type = stream.stream_type();
                let tags = stream.tags();
                let caps = stream.caps();

                // Extract language from tags if available
                let language = tags.as_ref().and_then(|t| {
                    t.index::<gst::tags::LanguageCode>(0)
                        .map(|val| val.get().to_string())
                });

                // Extract codec from caps if available
                let codec = caps
                    .as_ref()
                    .and_then(|c| c.structure(0).map(|s| s.name().to_string()));

                let stream_info = StreamInfo {
                    stream_id: stream_id.clone(),
                    stream_type,
                    tags,
                    caps,
                    index: 0,
                    language: language.clone(),
                    codec: codec.clone(),
                };

                if stream_type.contains(gst::StreamType::VIDEO) {
                    video_count += 1;
                    trace!("Found VIDEO stream: {} (codec: {:?})", stream_id, codec);
                } else if stream_type.contains(gst::StreamType::AUDIO) {
                    let mut info = stream_info;
                    info.index = audio_index;
                    audio_index += 1;
                    trace!(
                        "Found AUDIO stream #{}: {} (language: {:?})",
                        info.index, info.stream_id, info.language
                    );
                    audio.push(info);
                } else if stream_type.contains(gst::StreamType::TEXT) {
                    let mut info = stream_info;
                    info.index = subtitle_index;
                    subtitle_index += 1;
                    trace!(
                        "Found TEXT stream #{}: {} (language: {:?})",
                        info.index, info.stream_id, info.language
                    );
                    subtitles.push(info);
                } else {
                    trace!("Found OTHER stream type: {:?}", stream_type);
                }
            }
        }

        debug!(
            "Stream collection: {} video, {} audio, {} subtitle streams",
            video_count,
            audio.len(),
            subtitles.len()
        );

        // Update stored streams
        *audio_streams.lock().unwrap() = audio;
        *subtitle_streams.lock().unwrap() = subtitles;
    }

    pub async fn get_state(&self) -> PlayerState {
        // Query GStreamer for the actual state instead of relying on cached state
        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            let (_, current, _) = playbin.state(gst::ClockTime::ZERO);

            match current {
                gst::State::Playing => return PlayerState::Playing,
                gst::State::Paused => return PlayerState::Paused,
                gst::State::Ready | gst::State::Null => return PlayerState::Stopped,
                _ => {}
            }
        }

        // Fall back to cached state if playbin is not available
        self.state.read().await.clone()
    }

    pub async fn get_audio_tracks(&self) -> Vec<(i32, String)> {
        // Stream collection should have been received via bus messages

        let mut tracks = Vec::new();
        let audio_streams = self.audio_streams.lock().unwrap();

        for stream in audio_streams.iter() {
            let track_name = if let Some(ref lang) = stream.language {
                // Format track name with language
                format!("Audio Track {} ({})", stream.index + 1, lang)
            } else if let Some(ref codec) = stream.codec {
                // Format with codec if no language
                format!("Audio Track {} [{}]", stream.index + 1, codec)
            } else {
                // Default format
                format!("Audio Track {}", stream.index + 1)
            };
            tracks.push((stream.index, track_name));
        }

        // If no streams found yet, provide default
        if tracks.is_empty()
            && let Some(playbin) = self.playbin.lock().unwrap().as_ref()
        {
            let timeout = if cfg!(target_os = "macos") {
                gst::ClockTime::from_mseconds(100)
            } else {
                gst::ClockTime::ZERO
            };
            let (_, current, _) = playbin.state(timeout);

            if current < gst::State::Paused {
                debug!("Playbin not in PAUSED/PLAYING state yet, no audio tracks available");
            } else {
                // Provide a default track if playbin is ready but no collection received yet
                debug!("No stream collection available, providing default audio track");
                tracks.push((0, "Audio Track 1".to_string()));
            }
        }

        tracks
    }

    pub async fn get_subtitle_tracks(&self) -> Vec<(i32, String)> {
        // Stream collection should have been received via bus messages

        let mut tracks = Vec::new();

        // Add "None" option first
        tracks.push((-1, "None".to_string()));

        let subtitle_streams = self.subtitle_streams.lock().unwrap();

        for stream in subtitle_streams.iter() {
            let track_name = if let Some(ref lang) = stream.language {
                // Format track name with language
                format!("Subtitle {} ({})", stream.index + 1, lang)
            } else {
                // Default format
                format!("Subtitle {}", stream.index + 1)
            };
            tracks.push((stream.index, track_name));
        }

        tracks
    }

    pub async fn set_audio_track(&self, track_index: i32) -> Result<()> {
        debug!("Selecting audio track: {}", track_index);

        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            let audio_streams = self.audio_streams.lock().unwrap();
            let subtitle_streams = self.subtitle_streams.lock().unwrap();

            debug!("Available audio streams: {}", audio_streams.len());

            // Find the audio stream with the given index
            let new_audio_stream = audio_streams.iter().find(|s| s.index == track_index);

            if let Some(new_stream) = new_audio_stream {
                debug!(
                    "Found audio stream for index {}: {}",
                    track_index, new_stream.stream_id
                );

                // Build list of streams to select
                let mut selected_streams = Vec::new();

                // Add the new audio stream
                selected_streams.push(new_stream.stream_id.clone());
                debug!("Adding audio: {}", new_stream.stream_id);

                // Keep the current subtitle stream if one is selected
                if let Some(ref current_sub) = *self.current_subtitle_stream.lock().unwrap()
                    && subtitle_streams.iter().any(|s| s.stream_id == *current_sub)
                {
                    selected_streams.push(current_sub.clone());
                    debug!("Keeping subtitle: {}", current_sub);
                }

                // Also need to include video stream (playbin3 requires all streams)
                // Get the current stream collection to find video streams
                if let Some(ref collection) = *self.stream_collection.lock().unwrap() {
                    for i in 0..collection.len() {
                        let idx = i as u32;
                        if let Some(stream) = collection.stream(idx)
                            && stream.stream_type().contains(gst::StreamType::VIDEO)
                        {
                            let stream_id = stream
                                .stream_id()
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| "video-stream".to_string());
                            selected_streams.push(stream_id.clone());
                            debug!("Adding video: {}", stream_id);
                            break; // Usually only one video stream
                        }
                    }
                }

                // Create and send the select-streams event
                debug!(
                    "Sending SELECT_STREAMS event with {} streams",
                    selected_streams.len()
                );
                let stream_refs: Vec<&str> = selected_streams.iter().map(|s| s.as_str()).collect();
                let event = gst::event::SelectStreams::new(stream_refs.iter().copied());
                if playbin.send_event(event) {
                    debug!("SELECT_STREAMS event sent successfully");
                    // Update current audio stream
                    *self.current_audio_stream.lock().unwrap() = Some(new_stream.stream_id.clone());
                    info!(
                        "Selected audio track {}: {}",
                        track_index,
                        new_stream.language.as_deref().unwrap_or("Unknown")
                    );
                } else {
                    error!("Failed to send SELECT_STREAMS event");
                    return Err(anyhow::anyhow!("Failed to select audio track"));
                }
            } else {
                error!(
                    "Audio track with index {} not found in {} available tracks",
                    track_index,
                    audio_streams.len()
                );
                return Err(anyhow::anyhow!("Audio track {} not found", track_index));
            }
        } else {
            error!("No playbin available for audio track selection");
            return Err(anyhow::anyhow!("No playbin available"));
        }

        Ok(())
    }

    pub async fn set_subtitle_track(&self, track_index: i32) -> Result<()> {
        debug!("Selecting subtitle track: {}", track_index);

        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            let audio_streams = self.audio_streams.lock().unwrap();
            let subtitle_streams = self.subtitle_streams.lock().unwrap();

            debug!("Available subtitle streams: {}", subtitle_streams.len());

            // Build list of streams to select
            let mut selected_streams = Vec::new();

            // Keep the current audio stream
            if let Some(ref current_audio) = *self.current_audio_stream.lock().unwrap() {
                if audio_streams.iter().any(|s| s.stream_id == *current_audio) {
                    selected_streams.push(current_audio.clone());
                    debug!("Keeping audio: {}", current_audio);
                }
            } else if let Some(first_audio) = audio_streams.first() {
                // If no current audio, select the first one
                selected_streams.push(first_audio.stream_id.clone());
                debug!("Adding first audio: {}", first_audio.stream_id);
            }

            // Add the subtitle stream if not "None" (-1)
            if track_index >= 0 {
                if let Some(subtitle_stream) =
                    subtitle_streams.iter().find(|s| s.index == track_index)
                {
                    debug!("Adding subtitle: {}", subtitle_stream.stream_id);
                    selected_streams.push(subtitle_stream.stream_id.clone());

                    // Ensure text flag is enabled
                    playbin.set_property_from_str(
                        "flags",
                        "soft-colorbalance+deinterlace+soft-volume+audio+video+text",
                    );
                    debug!("Enabled text flag in playbin");
                } else {
                    error!("Subtitle track with index {} not found", track_index);
                    return Err(anyhow::anyhow!("Subtitle track {} not found", track_index));
                }
            } else {
                // Disable subtitles by not including any text stream
                debug!("Disabling subtitles (index = -1)");
                *self.current_subtitle_stream.lock().unwrap() = None;

                // Can still keep text flag enabled, just don't select any text stream
                playbin.set_property_from_str(
                    "flags",
                    "soft-colorbalance+deinterlace+soft-volume+audio+video",
                );
                debug!("Disabled text flag in playbin");
            }

            // Include video stream (required for playbin3)
            if let Some(ref collection) = *self.stream_collection.lock().unwrap() {
                for i in 0..collection.len() {
                    let idx = i as u32;
                    if let Some(stream) = collection.stream(idx)
                        && stream.stream_type().contains(gst::StreamType::VIDEO)
                    {
                        let stream_id = stream
                            .stream_id()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "video-stream".to_string());
                        selected_streams.push(stream_id.clone());
                        debug!("Adding video: {}", stream_id);
                        break;
                    }
                }
            }

            // Send the select-streams event if we have streams to select
            if !selected_streams.is_empty() {
                debug!(
                    "Sending SELECT_STREAMS event with {} streams",
                    selected_streams.len()
                );
                let stream_refs: Vec<&str> = selected_streams.iter().map(|s| s.as_str()).collect();
                let event = gst::event::SelectStreams::new(stream_refs.iter().copied());
                if playbin.send_event(event) {
                    debug!("SELECT_STREAMS event sent successfully");
                    // Update current subtitle stream
                    if track_index >= 0 {
                        if let Some(sub_stream) =
                            subtitle_streams.iter().find(|s| s.index == track_index)
                        {
                            *self.current_subtitle_stream.lock().unwrap() =
                                Some(sub_stream.stream_id.clone());
                            info!(
                                "Selected subtitle track {}: {}",
                                track_index,
                                sub_stream.language.as_deref().unwrap_or("Unknown")
                            );
                        }
                    } else {
                        *self.current_subtitle_stream.lock().unwrap() = None;
                        info!("Disabled subtitles");
                    }
                } else {
                    error!("Failed to send SELECT_STREAMS event");
                    return Err(anyhow::anyhow!("Failed to select subtitle track"));
                }
            } else {
                warn!("No streams to select - this shouldn't happen");
            }
        } else {
            error!("No playbin available for subtitle track selection");
            return Err(anyhow::anyhow!("No playbin available"));
        }

        Ok(())
    }

    pub async fn get_current_audio_track(&self) -> i32 {
        if let Some(ref current_id) = *self.current_audio_stream.lock().unwrap() {
            // Find the index of the current audio stream
            let audio_streams = self.audio_streams.lock().unwrap();
            for stream in audio_streams.iter() {
                if stream.stream_id == *current_id {
                    return stream.index;
                }
            }
        }
        -1
    }

    pub async fn get_current_subtitle_track(&self) -> i32 {
        if let Some(ref current_id) = *self.current_subtitle_stream.lock().unwrap() {
            // Find the index of the current subtitle stream
            let subtitle_streams = self.subtitle_streams.lock().unwrap();
            for stream in subtitle_streams.iter() {
                if stream.stream_id == *current_id {
                    return stream.index;
                }
            }
        }
        -1 // No subtitle selected
    }

    pub async fn set_playback_speed(&self, speed: f64) -> Result<()> {
        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            // GStreamer uses a seek with rate to change playback speed
            let position = playbin.query_position::<gst::ClockTime>();
            if let Some(pos) = position {
                playbin
                    .seek(
                        speed,
                        gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                        gst::SeekType::Set,
                        pos,
                        gst::SeekType::None,
                        gst::ClockTime::NONE,
                    )
                    .map_err(|e| anyhow::anyhow!("Failed to set playback speed: {:?}", e))?;
            }
        }
        Ok(())
    }

    pub async fn get_playback_speed(&self) -> f64 {
        // GStreamer doesn't have a simple way to get current playback rate
        // We'd need to track it separately or query the segment
        1.0 // Default to normal speed for now
    }

    pub async fn frame_step_forward(&self) -> Result<()> {
        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            // GStreamer frame stepping requires pausing first and then seeking
            // Frame stepping with Step events is complex and not well-supported
            // Instead, we'll use a small seek forward
            let position = playbin.query_position::<gst::ClockTime>();
            if let Some(pos) = position {
                let new_pos = pos + gst::ClockTime::from_mseconds(40); // ~1 frame at 25fps
                playbin
                    .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::SNAP_BEFORE, new_pos)
                    .map_err(|e| anyhow::anyhow!("Failed to step forward: {:?}", e))?;
            }
        }
        Ok(())
    }

    pub async fn frame_step_backward(&self) -> Result<()> {
        // GStreamer doesn't natively support backward frame stepping
        // We'd need to implement this with seeking
        Err(anyhow::anyhow!(
            "Backward frame stepping not supported in GStreamer"
        ))
    }

    pub async fn toggle_mute(&self) -> Result<()> {
        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            let current_mute = playbin.property::<bool>("mute");
            playbin.set_property("mute", !current_mute);
        }
        Ok(())
    }

    pub async fn is_muted(&self) -> bool {
        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            playbin.property::<bool>("mute")
        } else {
            false
        }
    }

    pub async fn cycle_subtitle_track(&self) -> Result<()> {
        let subtitle_streams = self.subtitle_streams.lock().unwrap();
        let current = self.get_current_subtitle_track().await;

        let next_track = if subtitle_streams.is_empty() {
            -1 // No subtitles available
        } else if current == -1 {
            // Currently off, go to first subtitle
            0
        } else {
            // Find next track or loop back to "None"
            let next_idx = current + 1;
            if next_idx >= subtitle_streams.len() as i32 {
                -1 // Loop back to "None"
            } else {
                next_idx
            }
        };

        info!("Cycling subtitle track from {} to {}", current, next_track);
        self.set_subtitle_track(next_track).await
    }

    pub async fn cycle_audio_track(&self) -> Result<()> {
        let audio_streams = self.audio_streams.lock().unwrap();
        if audio_streams.is_empty() {
            return Ok(()); // No audio tracks to cycle
        }

        let current = self.get_current_audio_track().await;
        let next_track = if current == -1 {
            // No current track (shouldn't happen), select first
            0
        } else {
            // Cycle to next track or loop back to first
            (current + 1) % audio_streams.len() as i32
        };

        info!("Cycling audio track from {} to {}", current, next_track);
        self.set_audio_track(next_track).await
    }

    pub async fn set_zoom_mode(&self, mode: ZoomMode) -> Result<()> {
        // Update internal state
        *self.zoom_mode.lock().unwrap() = mode;

        // Apply zoom transformation to video widget
        if let Some(widget) = self.video_widget.lock().unwrap().as_ref() {
            // For GStreamer, we'll use CSS transforms on the widget

            // Remove previous zoom class
            widget.remove_css_class("zoom-fit");
            widget.remove_css_class("zoom-fill");
            widget.remove_css_class("zoom-16-9");
            widget.remove_css_class("zoom-4-3");
            widget.remove_css_class("zoom-2-35");
            widget.remove_css_class("zoom-custom");

            match mode {
                ZoomMode::Fit => {
                    widget.add_css_class("zoom-fit");
                    widget.set_size_request(-1, -1);
                }
                ZoomMode::Fill => {
                    widget.add_css_class("zoom-fill");
                    widget.set_size_request(-1, -1);
                }
                ZoomMode::Zoom16_9 => {
                    widget.add_css_class("zoom-16-9");
                    // Force aspect ratio through widget sizing hints
                    widget.set_size_request(-1, -1);
                }
                ZoomMode::Zoom4_3 => {
                    widget.add_css_class("zoom-4-3");
                    widget.set_size_request(-1, -1);
                }
                ZoomMode::Zoom2_35 => {
                    widget.add_css_class("zoom-2-35");
                    widget.set_size_request(-1, -1);
                }
                ZoomMode::Custom(level) => {
                    widget.add_css_class("zoom-custom");
                    // Apply custom scale transform through inline CSS
                    let css = format!("transform: scale({});", level);
                    widget.set_property("css-name", &css);
                }
            }
        }

        Ok(())
    }

    pub async fn get_zoom_mode(&self) -> ZoomMode {
        *self.zoom_mode.lock().unwrap()
    }
}
