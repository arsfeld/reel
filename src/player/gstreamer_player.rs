use crate::player::ZoomMode;
use crate::player::gstreamer::bus_handler;
use crate::player::gstreamer::sink_factory;
use crate::player::gstreamer::stream_manager::{StreamInfo, StreamManager};
use anyhow::{Context, Result};
use gdk4 as gdk;
use gstreamer as gst;
use gstreamer::glib;
use gstreamer::prelude::*;
use gtk4::{self, prelude::*};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
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
pub struct BufferingState {
    pub is_buffering: bool,
    pub percentage: i32,
}

pub struct GStreamerPlayer {
    playbin: Arc<Mutex<Option<gst::Element>>>,
    state: Arc<RwLock<PlayerState>>,
    video_sink: Arc<Mutex<Option<gst::Element>>>,
    zoom_mode: Arc<Mutex<ZoomMode>>,
    video_widget: Arc<Mutex<Option<gtk4::Widget>>>,
    stream_manager: StreamManager,
    pipeline_ready: Arc<Mutex<bool>>,
    seek_pending: Arc<Mutex<Option<(f64, Instant)>>>,
    last_seek_target: Arc<Mutex<Option<f64>>>,
    buffering_state: Arc<RwLock<BufferingState>>,
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

        // Check for required elements
        Self::check_gstreamer_plugins();

        Ok(Self {
            playbin: Arc::new(Mutex::new(None)),
            state: Arc::new(RwLock::new(PlayerState::Idle)),
            video_sink: Arc::new(Mutex::new(None)),
            zoom_mode: Arc::new(Mutex::new(ZoomMode::default())),
            video_widget: Arc::new(Mutex::new(None)),
            stream_manager: StreamManager::new(),
            pipeline_ready: Arc::new(Mutex::new(false)),
            seek_pending: Arc::new(Mutex::new(None)),
            last_seek_target: Arc::new(Mutex::new(None)),
            buffering_state: Arc::new(RwLock::new(BufferingState {
                is_buffering: false,
                percentage: 100,
            })),
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
        let video_sink = sink_factory::create_optimized_video_sink(force_fallback, use_gl_sink);

        // If we have a gtk4paintablesink, extract and set its paintable
        if let Some(ref sink) = video_sink
            && let Some(gtk_sink) = sink_factory::extract_gtk4_sink(sink)
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

    pub async fn load_media(&self, url: &str, _video_sink: Option<&gst::Element>) -> Result<()> {
        info!("Loading media: {}", url);
        debug!("GStreamerPlayer::load_media() - Full URL: {}", url);

        // Reset pipeline ready flag for new media
        *self.pipeline_ready.lock().unwrap() = false;

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
        self.stream_manager.clear();

        // Create playbin3 element - NEVER fallback
        trace!("Creating playbin3 element");
        let playbin = gst::ElementFactory::make("playbin3")
            .name("player")
            .property("uri", url)
            .build()
            .context("Failed to create playbin3 element - GStreamer plugins may not be properly installed")?;

        trace!("Successfully created playbin3");

        // Verify we're using playbin3
        if let Some(factory) = playbin.factory() {
            debug!(
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

                if let Some(fallback_sink) = sink_factory::create_auto_fallback_sink() {
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
        let (
            stream_collection_clone,
            audio_streams_clone,
            subtitle_streams_clone,
            current_audio_clone,
            current_subtitle_clone,
        ) = self.stream_manager.get_refs_for_message_handler();
        let pipeline_ready_clone = self.pipeline_ready.clone();
        let playbin_clone = self.playbin.clone();
        let buffering_state_clone = self.buffering_state.clone();

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
            let buffering_state = buffering_state_clone.clone();
            let msg = msg.clone();

            // Handle message synchronously to avoid context issues
            bus_handler::handle_bus_message_sync(
                &msg,
                &state,
                &stream_collection,
                &audio_streams,
                &subtitle_streams,
                &current_audio,
                &current_subtitle,
                &pipeline_ready_clone,
                &playbin,
                &buffering_state,
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

        // Check if pipeline is ready for seeking (HTTP sources need ASYNC_DONE)
        let is_ready = *self.pipeline_ready.lock().unwrap();
        if !is_ready {
            warn!(
                "GStreamerPlayer::seek() - Pipeline not ready for seeking (waiting for ASYNC_DONE)"
            );
            return Err(anyhow::anyhow!("Pipeline not ready for seeking"));
        }

        let position_ns = position.as_nanos() as i64;
        let position_secs = position.as_secs_f64();

        // Update the last seek target for position tracking
        {
            let mut last_target = self.last_seek_target.lock().unwrap();
            *last_target = Some(position_secs);
        }

        // Store the pending seek position
        {
            let mut pending = self.seek_pending.lock().unwrap();
            *pending = Some((position_secs, Instant::now()));
        }

        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            // Remember if we were playing to resume after seek
            let was_playing = matches!(*self.state.read().await, PlayerState::Playing);

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
                                    return Err(anyhow::anyhow!(
                                        "Failed to reach PAUSED state for seeking"
                                    ));
                                }
                            }
                            Err(_) => {
                                return Err(anyhow::anyhow!(
                                    "Timeout waiting for PAUSED state before seeking"
                                ));
                            }
                        }
                    }
                    Ok(gst::StateChangeSuccess::NoPreroll) => {
                        debug!("Live source, no preroll needed");
                    }
                    Err(_) => {
                        return Err(anyhow::anyhow!("Failed to set PAUSED state for seeking"));
                    }
                }
            }

            // Check if media is seekable
            let mut query = gst::query::Seeking::new(gst::Format::Time);
            if playbin.query(&mut query) {
                let (seekable, start, end) = query.result();
                debug!(
                    "GStreamerPlayer::seek() - Media seekable: {}, range: {:?} - {:?}",
                    seekable, start, end
                );

                if !seekable {
                    return Err(anyhow::anyhow!("Media is not seekable"));
                }
            } else {
                warn!("GStreamerPlayer::seek() - Unable to query seeking capability");
            }

            // Use FLUSH | KEY_UNIT flags for network streams
            let seek_flags = gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT;
            let seek_position = gst::ClockTime::from_nseconds(position_ns as u64);

            debug!(
                "GStreamerPlayer::seek() - Seeking to {} with flags {:?}",
                seek_position.display(),
                seek_flags
            );

            // Use seek_simple - it's simpler and should work for most cases
            let seek_result = playbin.seek_simple(seek_flags, seek_position);

            if seek_result.is_err() {
                error!("GStreamerPlayer::seek() - Seek failed");

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
                            return Err(anyhow::anyhow!("Seek failed: {}", err.error()));
                        }
                    }
                }

                return Err(anyhow::anyhow!("Failed to seek to position {:?}", position));
            }

            debug!("GStreamerPlayer::seek() - Seek initiated successfully");

            // Wait for the seek to complete (ASYNC_DONE message)
            // Give it a moment to process
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Resume playing if we were playing before the seek
            if was_playing {
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
        // If we have a pending seek, return that as the effective position
        // This prevents showing stale values immediately after seeking
        {
            let last_target = self.last_seek_target.lock().unwrap();
            if let Some(target_pos) = *last_target {
                // Check if the seek is recent (within 200ms)
                if let Some((_, timestamp)) = *self.seek_pending.lock().unwrap() {
                    if timestamp.elapsed() < Duration::from_millis(200) {
                        return Some(Duration::from_secs_f64(target_pos.max(0.0)));
                    }
                }
            }
        }

        // Otherwise return the actual position from GStreamer
        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            if let Some(pos) = playbin.query_position::<gst::ClockTime>() {
                // Clear the last seek target since we're at the actual position now
                let mut last_target = self.last_seek_target.lock().unwrap();
                *last_target = None;
                return Some(Duration::from_nanos(pos.nseconds()));
            }
        }
        None
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
        let playbin = self.playbin.lock().unwrap();
        self.stream_manager.get_audio_tracks(playbin.as_ref())
    }

    pub async fn get_subtitle_tracks(&self) -> Vec<(i32, String)> {
        self.stream_manager.get_subtitle_tracks()
    }

    pub async fn set_audio_track(&self, track_index: i32) -> Result<()> {
        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            self.stream_manager.set_audio_track(track_index, playbin)
        } else {
            Err(anyhow::anyhow!("No playbin available"))
        }
    }

    pub async fn set_subtitle_track(&self, track_index: i32) -> Result<()> {
        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            self.stream_manager.set_subtitle_track(track_index, playbin)
        } else {
            Err(anyhow::anyhow!("No playbin available"))
        }
    }

    pub async fn get_current_audio_track(&self) -> i32 {
        self.stream_manager.get_current_audio_track()
    }

    pub async fn get_current_subtitle_track(&self) -> i32 {
        self.stream_manager.get_current_subtitle_track()
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
        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            self.stream_manager.cycle_subtitle_track(playbin)
        } else {
            Err(anyhow::anyhow!("No playbin available"))
        }
    }

    pub async fn cycle_audio_track(&self) -> Result<()> {
        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            self.stream_manager.cycle_audio_track(playbin)
        } else {
            Err(anyhow::anyhow!("No playbin available"))
        }
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

    pub async fn get_buffering_state(&self) -> BufferingState {
        self.buffering_state.read().await.clone()
    }
}
