use crate::player::ZoomMode;
use crate::player::gstreamer::bus_handler;
use crate::player::gstreamer::sink_factory;
use crate::player::gstreamer::stream_manager::StreamManager;
use anyhow::{Context, Result};
use gdk4 as gdk;
use gstreamer as gst;
use gstreamer::bus::BusWatchGuard;
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
    buffering_state: Arc<RwLock<BufferingState>>,
    bus_watch_guard: Arc<Mutex<Option<BusWatchGuard>>>,
    current_playback_speed: Arc<Mutex<f64>>,
    paused_for_buffering: Arc<Mutex<bool>>,
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

        // Ensure proper audio sink selection by raising ranks
        Self::configure_audio_sink_ranks();

        Ok(Self {
            playbin: Arc::new(Mutex::new(None)),
            state: Arc::new(RwLock::new(PlayerState::Idle)),
            video_sink: Arc::new(Mutex::new(None)),
            zoom_mode: Arc::new(Mutex::new(ZoomMode::default())),
            video_widget: Arc::new(Mutex::new(None)),
            stream_manager: StreamManager::new(),
            pipeline_ready: Arc::new(Mutex::new(false)),
            buffering_state: Arc::new(RwLock::new(BufferingState {
                is_buffering: false,
                percentage: 100,
            })),
            bus_watch_guard: Arc::new(Mutex::new(None)),
            current_playback_speed: Arc::new(Mutex::new(1.0)),
            paused_for_buffering: Arc::new(Mutex::new(false)),
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

    fn configure_audio_sink_ranks() {
        info!("Configuring audio sink ranks for proper autoplugging");
        let registry = gst::Registry::get();

        // Raise osxaudiosink to PRIMARY+1 (higher than autoaudiosink) for macOS
        if let Some(feature) = registry.lookup_feature("osxaudiosink") {
            let old_rank = feature.rank();
            feature.set_rank(gst::Rank::PRIMARY + 1);
            info!("Raised osxaudiosink rank from {} to PRIMARY+1", old_rank);
        }

        // Raise pulsesink to PRIMARY+1 for Linux
        if let Some(feature) = registry.lookup_feature("pulsesink") {
            let old_rank = feature.rank();
            feature.set_rank(gst::Rank::PRIMARY + 1);
            info!("Raised pulsesink rank from {} to PRIMARY+1", old_rank);
        }

        // Ensure autoaudiosink has at least MARGINAL rank so it can be used as fallback
        if let Some(feature) = registry.lookup_feature("autoaudiosink") {
            let old_rank = feature.rank();
            if old_rank < gst::Rank::MARGINAL {
                feature.set_rank(gst::Rank::MARGINAL);
                info!("Raised autoaudiosink rank from {} to MARGINAL", old_rank);
            }
        }

        // Lower fakesink rank to NONE to prevent it being selected for audio
        if let Some(feature) = registry.lookup_feature("fakesink") {
            let old_rank = feature.rank();
            feature.set_rank(gst::Rank::NONE);
            debug!("Lowered fakesink rank from {} to NONE", old_rank);
        }
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

        // Clear buffering flag for new media
        *self.paused_for_buffering.lock().unwrap() = false;

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

        // Use playbin3 for modern stream handling and better performance
        //
        // playbin3 is the recommended playback element as of GStreamer 1.22+:
        // - No longer experimental (stable API since GStreamer 1.22)
        // - Default in GStreamer 1.24+
        // - Better stream selection via GstStreamCollection API
        // - Improved handling of high-bitrate content
        // - Used by WebKit and other major projects
        //
        // Note: playbin3 requires proper state transition handling (Null → Ready → Paused → Playing)
        // which is implemented in the play() method to avoid preroll hangs.
        trace!("Creating playbin3 element");
        let playbin = gst::ElementFactory::make("playbin3")
            .name("player")
            .property("uri", url)
            .build()
            .context(
                "Failed to create playbin3 element - GStreamer plugins may not be properly installed",
            )?;

        trace!("Successfully created playbin3");

        // Log which playbin we're using
        if let Some(factory) = playbin.factory() {
            info!(
                "Using element: {} (factory: {})",
                playbin.name(),
                factory.name()
            );
        }

        // Enable all features including text overlay
        playbin.set_property_from_str(
            "flags",
            "soft-colorbalance+deinterlace+soft-volume+audio+video+text",
        );

        info!("Configuring playbin3 settings...");

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
            // Create a fallback video sink on other platforms
            debug!("No pre-configured sink, creating fallback");

            if let Some(fallback_sink) = sink_factory::create_auto_fallback_sink() {
                playbin.set_property("video-sink", &fallback_sink);
                debug!("Fallback video sink configured");
            } else {
                error!("GStreamerPlayer::load_media() - Failed to create any fallback video sink!");
            }
        }

        // DON'T set audio-sink - let playbin3 autoplugging handle it
        // Setting both video-sink and audio-sink explicitly causes playbin3 to bypass decodebin3,
        // which means no audio decoder is created. Let playbin3 autoplugging handle audio.
        // Note: We only set video-sink because we need to extract the paintable for GTK rendering.
        debug!("Letting playbin3 autoplugging handle audio sink selection");

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

        // Set up async bus watch FIRST, before any state changes
        // This follows playbin3 best practices: handle everything asynchronously
        // The bus watch will process all messages including StreamCollection, ASYNC_DONE, errors, etc.
        info!("Setting up async bus watch with glib main loop integration...");

        // Remove any existing bus watch before adding a new one
        if let Some(_old_guard) = self.bus_watch_guard.lock().unwrap().take() {
            debug!("Removed previous bus watch (via guard drop)");
        }

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
        let paused_for_buffering_clone = self.paused_for_buffering.clone();

        let watch_guard = bus
            .add_watch(move |_, msg| {
                let msg_type = msg.type_();
                if !matches!(msg_type, gst::MessageType::Qos | gst::MessageType::Progress) {
                    let src_name = msg
                        .src()
                        .map(|s| s.name().to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    if matches!(msg_type, gst::MessageType::StreamCollection) {
                        info!("Stream collection message from {}", src_name);
                    } else if matches!(msg_type, gst::MessageType::StreamsSelected) {
                        info!("Streams selected message from {}", src_name);
                    } else if matches!(msg_type, gst::MessageType::StreamStart) {
                        info!("Stream start message from {}", src_name);
                    } else if !matches!(
                        msg_type,
                        gst::MessageType::StateChanged
                            | gst::MessageType::Tag
                            | gst::MessageType::StreamStatus
                    ) {
                        trace!("Bus message: {:?} from {}", msg_type, src_name);
                    }
                }

                bus_handler::handle_bus_message_sync(
                    msg,
                    &state_clone,
                    &stream_collection_clone,
                    &audio_streams_clone,
                    &subtitle_streams_clone,
                    &current_audio_clone,
                    &current_subtitle_clone,
                    &pipeline_ready_clone,
                    &playbin_clone,
                    &buffering_state_clone,
                    &paused_for_buffering_clone,
                );

                glib::ControlFlow::Continue
            })
            .context("Failed to add bus watch")?;

        *self.bus_watch_guard.lock().unwrap() = Some(watch_guard);
        info!("Async bus watch set up successfully");

        // Now start pipeline preroll by setting to PAUSED state
        // playbin3 will asynchronously:
        //  1. Discover streams and send STREAM_COLLECTION messages
        //  2. Preroll the pipeline
        //  3. Send ASYNC_DONE when ready for seeking/playback
        // All messages are handled by the async bus watch above
        debug!("Setting pipeline to PAUSED for preroll and stream discovery");
        playbin
            .set_state(gst::State::Paused)
            .context("Failed to set pipeline to PAUSED state")?;

        info!("Pipeline preroll initiated - messages will be handled asynchronously");
        debug!("Media loading complete");
        Ok(())
    }

    pub async fn play(&self) -> Result<()> {
        info!("Starting playback");

        let playbin = self
            .playbin
            .lock()
            .unwrap()
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No playbin available"))?;

        // playbin3 automatically handles state transitions: Null → Ready → Paused → Playing
        // We don't query the current state because the pipeline may be in async transition.
        // Just set to Playing and let playbin3 handle the transition from whatever state
        // it's currently in (which is typically PAUSED after load_media()).
        // State updates will come through the bus handler via StateChanged messages.

        match playbin.set_state(gst::State::Playing) {
            Ok(gst::StateChangeSuccess::Success) => {
                info!("Playback started successfully");
                // State will be updated by bus handler's StateChanged message
            }
            Ok(gst::StateChangeSuccess::Async) => {
                info!("Playback starting asynchronously");
                // State transitions will be reported via bus handler's StateChanged messages
                // No need to wait synchronously - the async bus watch handles this
            }
            Ok(gst::StateChangeSuccess::NoPreroll) => {
                info!("Playback started (live source, no preroll)");
                // State will be updated by bus handler's StateChanged message
            }
            Err(gst::StateChangeError) => {
                error!("Failed to start playback");

                // Check bus for specific error details
                let mut error_details = Vec::new();
                if let Some(bus) = playbin.bus() {
                    while let Some(msg) = bus.pop() {
                        use gst::MessageView;
                        if let MessageView::Error(err) = msg.view() {
                            let error_msg = format!(
                                "{} (from: {:?})",
                                err.error(),
                                err.src().map(|s| s.path_string())
                            );
                            error!("Bus error: {} ({:?})", err.error(), err.debug());
                            error_details.push(error_msg);
                        }
                    }
                }

                let error_msg = if !error_details.is_empty() {
                    format!("Failed to play media: {}", error_details.join("; "))
                } else {
                    "Failed to set playbin to playing state".to_string()
                };

                return Err(anyhow::anyhow!(error_msg));
            }
        }

        Ok(())
    }

    pub async fn pause(&self) -> Result<()> {
        debug!("Pausing playback");

        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            playbin
                .set_state(gst::State::Paused)
                .context("Failed to set playbin to paused state")?;

            // State will be updated by bus handler's StateChanged message

            // Clear the buffering flag since this is a user-initiated pause
            *self.paused_for_buffering.lock().unwrap() = false;
        }
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        debug!("Stopping playback");

        if let Some(playbin) = self.playbin.lock().unwrap().as_ref() {
            playbin
                .set_state(gst::State::Null)
                .context("Failed to set playbin to null state")?;

            // State will be updated by bus handler's StateChanged message

            // Clear the buffering flag
            *self.paused_for_buffering.lock().unwrap() = false;
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

        let playbin = self
            .playbin
            .lock()
            .unwrap()
            .as_ref()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No playbin available for seeking"))?;

        // Remember if we were playing to resume after seek
        let was_playing = matches!(*self.state.read().await, PlayerState::Playing);

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

        // Use FLUSH | KEY_UNIT flags for accurate seeking with flushing
        // FLUSH will clear the pipeline and restart playback at the new position
        let seek_flags = gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT;
        let seek_position = gst::ClockTime::from_nseconds(position.as_nanos() as u64);

        debug!(
            "GStreamerPlayer::seek() - Seeking to {} with flags {:?}",
            seek_position.display(),
            seek_flags
        );

        // Perform the seek operation
        // With FLUSH flag, seek works at any state (even PLAYING)
        // The pipeline will handle state transitions automatically
        playbin
            .seek_simple(seek_flags, seek_position)
            .map_err(|_| {
                error!("GStreamerPlayer::seek() - Seek operation failed");

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

                anyhow::anyhow!("Failed to seek to position {:?}", position)
            })?;

        debug!("GStreamerPlayer::seek() - Seek initiated successfully");

        // Seek completion will be signaled via ASYNC_DONE bus message
        // No need to wait synchronously - the bus handler will process it

        // Resume playing if we were playing before the seek
        // The FLUSH seek automatically pauses, so we need to resume if needed
        if was_playing {
            debug!("GStreamerPlayer::seek() - Resuming playback after seek");
            playbin.set_state(gst::State::Playing).ok();
        }

        Ok(())
    }

    pub async fn get_position(&self) -> Option<Duration> {
        // Query the pipeline for the current position
        // This is the single source of truth for position tracking
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
            // Note: This synchronous state query is justified because:
            // 1. Video dimensions are only available after pipeline reaches PAUSED
            // 2. We need to verify state before querying caps
            // 3. If needed, we initiate state change and wait for completion
            // This is an edge case that doesn't affect normal playback flow
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
                        // This timeout is justified: we just initiated the state change
                        // and need to wait for it to complete before querying dimensions
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
        // Return cached state updated by bus handler's StateChanged messages
        // This is async-aware and doesn't block on GStreamer queries
        // The bus handler keeps this state synchronized with the actual pipeline state
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

                // Update stored playback speed
                *self.current_playback_speed.lock().unwrap() = speed;
                debug!("Playback speed set to {}", speed);
            }
        }
        Ok(())
    }

    pub async fn get_playback_speed(&self) -> f64 {
        // Return the currently stored playback speed
        *self.current_playback_speed.lock().unwrap()
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

    /// Wait for pipeline to complete preroll and be ready for seeking.
    /// This waits for the ASYNC_DONE message which signals that:
    /// - Stream collection has been discovered
    /// - Pipeline has prerolled and buffered initial data
    /// - Seeking operations will work correctly
    pub async fn wait_until_ready(&self, timeout: Duration) -> Result<()> {
        use std::time::Instant;

        let start = Instant::now();
        while !*self.pipeline_ready.lock().unwrap() {
            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!(
                    "Timeout waiting for GStreamer pipeline ready (ASYNC_DONE not received)"
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        debug!("GStreamer pipeline ready for seeking");
        Ok(())
    }
}

impl Drop for GStreamerPlayer {
    fn drop(&mut self) {
        debug!("GStreamerPlayer - Dropping player, cleaning up resources");

        // Remove bus watch to prevent callbacks after drop
        // BusWatchGuard automatically removes the watch when dropped
        if let Some(_guard) = self.bus_watch_guard.lock().unwrap().take() {
            debug!("GStreamerPlayer - Bus watch will be removed (via guard drop)");
        }

        // Set pipeline to NULL state to release resources
        if let Some(playbin) = self.playbin.lock().unwrap().take() {
            debug!("GStreamerPlayer - Setting pipeline to NULL state");
            if let Err(e) = playbin.set_state(gst::State::Null) {
                error!(
                    "GStreamerPlayer - Failed to set pipeline to NULL on drop: {:?}",
                    e
                );
            } else {
                debug!("GStreamerPlayer - Pipeline set to NULL state successfully");
            }
        }

        debug!("GStreamerPlayer - Drop complete");
    }
}
