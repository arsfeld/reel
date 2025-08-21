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
use tracing::{debug, error, info};

#[derive(Debug, Clone)]
pub enum PlayerState {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopped,
    Error(String),
}

pub struct GStreamerPlayer {
    playbin: RefCell<Option<gst::Element>>,
    state: Arc<RwLock<PlayerState>>,
    video_widget: RefCell<Option<gtk4::Widget>>,
    video_sink: RefCell<Option<gst::Element>>,
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
        })
    }

    fn check_gstreamer_plugins() {
        info!("Checking GStreamer plugin availability");

        let required_elements = vec![
            "playbin",
            "playbin3",
            "autovideosink",
            "autoaudiosink",
            "gtk4paintablesink",
            "glimagesink",
            "videoconvert",
            "videoscale",
            "capsfilter",
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

        // Try to create gtk4paintablesink with proper video conversion pipeline
        let gtksink_result = if !force_fallback && !use_gl_sink {
            gst::ElementFactory::make("gtk4paintablesink")
                .name("videosink")
                .build()
        } else if use_gl_sink {
            info!("GStreamerPlayer::create_video_widget() - Using GL sink (REEL_USE_GL_SINK set)");
            Err(gst::glib::bool_error!("Use GL sink"))
        } else {
            info!(
                "GStreamerPlayer::create_video_widget() - Forcing fallback sink (REEL_FORCE_FALLBACK_SINK set)"
            );
            Err(gst::glib::bool_error!("Forced fallback"))
        };

        match gtksink_result {
            Ok(gtk_sink) => {
                info!(
                    "GStreamerPlayer::create_video_widget() - Successfully created gtk4paintablesink"
                );

                // Create a simpler, more robust pipeline for gtk4paintablesink
                let bin = gst::Bin::new();

                // Single videoconvert with proper configuration
                let videoconvert = gst::ElementFactory::make("videoconvert")
                    .name("videoconvert")
                    .build()
                    .expect("Failed to create videoconvert");

                // Configure videoconvert to handle subtitle overlays properly
                // Disable passthrough to force conversion even if formats match
                videoconvert.set_property_from_str("n-threads", "4");

                // Add videoscale for proper scaling
                let videoscale = gst::ElementFactory::make("videoscale")
                    .name("videoscale")
                    .build()
                    .expect("Failed to create videoscale");

                // Force output to RGBA - critical for gtk4paintablesink with subtitles
                let capsfilter = gst::ElementFactory::make("capsfilter")
                    .name("capsfilter")
                    .build()
                    .expect("Failed to create capsfilter");

                // Force RGBA format and disable DMA-BUF to avoid YUV colorspace issues
                // This is critical for subtitle rendering
                let caps = gst::Caps::builder("video/x-raw")
                    .field("format", "RGBA")
                    .build();
                capsfilter.set_property("caps", &caps);

                // Add elements to the bin
                bin.add(&videoconvert).expect("Failed to add videoconvert");
                bin.add(&videoscale).expect("Failed to add videoscale");
                bin.add(&capsfilter).expect("Failed to add capsfilter");
                bin.add(&gtk_sink).expect("Failed to add gtk_sink");

                // Link the elements
                videoconvert
                    .link(&videoscale)
                    .expect("Failed to link videoconvert to videoscale");
                videoscale
                    .link(&capsfilter)
                    .expect("Failed to link videoscale to capsfilter");
                capsfilter
                    .link(&gtk_sink)
                    .expect("Failed to link capsfilter to gtk_sink");

                // Create ghost pad for the bin
                let sink_pad = videoconvert
                    .static_pad("sink")
                    .expect("Failed to get sink pad");
                let ghost_pad =
                    gst::GhostPad::with_target(&sink_pad).expect("Failed to create ghost pad");
                bin.add_pad(&ghost_pad).expect("Failed to add ghost pad");

                // Get the paintable from the sink and set it on the picture
                let paintable = gtk_sink.property::<gdk::Paintable>("paintable");
                picture.set_paintable(Some(&paintable));

                // Store the bin (which includes the sink) for later use
                self.video_sink.replace(Some(bin.upcast()));
                debug!(
                    "GStreamerPlayer::create_video_widget() - gtk4paintablesink with conversion pipeline configured"
                );
            }
            Err(e) => {
                info!(
                    "GStreamerPlayer::create_video_widget() - gtk4paintablesink not available ({}), using fallback widget",
                    e
                );

                // Try glimagesink first as it has better colorspace handling
                let sink_result = if use_gl_sink {
                    gst::ElementFactory::make("glimagesink")
                        .name("glimagesink")
                        .build()
                } else {
                    gst::ElementFactory::make("autovideosink")
                        .name("autovideosink")
                        .build()
                };

                if let Ok(video_sink) = sink_result {
                    let bin = gst::Bin::new();

                    // Create robust conversion pipeline for fallback
                    let videoconvert = gst::ElementFactory::make("videoconvert")
                        .name("videoconvert")
                        .build()
                        .expect("Failed to create videoconvert");

                    // Force RGBA to handle subtitles properly
                    let capsfilter = gst::ElementFactory::make("capsfilter")
                        .name("capsfilter")
                        .build()
                        .expect("Failed to create capsfilter");

                    let caps = gst::Caps::builder("video/x-raw")
                        .field("format", "RGBA")
                        .build();
                    capsfilter.set_property("caps", &caps);

                    bin.add(&videoconvert).expect("Failed to add videoconvert");
                    bin.add(&capsfilter).expect("Failed to add capsfilter");
                    bin.add(&video_sink).expect("Failed to add video sink");

                    videoconvert
                        .link(&capsfilter)
                        .expect("Failed to link videoconvert to capsfilter");
                    capsfilter
                        .link(&video_sink)
                        .expect("Failed to link capsfilter to sink");

                    let sink_pad = videoconvert
                        .static_pad("sink")
                        .expect("Failed to get sink pad");
                    let ghost_pad =
                        gst::GhostPad::with_target(&sink_pad).expect("Failed to create ghost pad");
                    bin.add_pad(&ghost_pad).expect("Failed to add ghost pad");

                    self.video_sink.replace(Some(bin.upcast()));
                    info!(
                        "Using {} as video sink",
                        if use_gl_sink {
                            "glimagesink"
                        } else {
                            "autovideosink"
                        }
                    );
                } else {
                    self.video_sink.replace(None);
                }
            }
        }

        // Store the widget
        let widget = picture.upcast::<gtk4::Widget>();
        self.video_widget.replace(Some(widget.clone()));

        info!("GStreamerPlayer::create_video_widget() - Video widget creation complete");
        widget
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

        // Try to create playbin - note: playbin3 might not exist in all configurations
        let playbin = if gst::ElementFactory::find("playbin").is_some() {
            info!("GStreamerPlayer::load_media() - Creating playbin element");
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

            // Create a video filter bin to ensure proper colorspace for subtitles
            // This prevents the green bar issue when subtitles appear
            let filter_bin = gst::Bin::new();

            // Add videoconvert to handle colorspace changes from subtitle overlay
            if let Ok(videoconvert) = gst::ElementFactory::make("videoconvert")
                .name("subtitle_videoconvert")
                .build()
            {
                // Add capsfilter to force RGBA after subtitle compositing
                if let Ok(capsfilter) = gst::ElementFactory::make("capsfilter")
                    .name("subtitle_capsfilter")
                    .build()
                {
                    // Force RGBA to prevent YUV issues with subtitles
                    let caps = gst::Caps::builder("video/x-raw")
                        .field("format", "RGBA")
                        .build();
                    capsfilter.set_property("caps", &caps);

                    filter_bin
                        .add(&videoconvert)
                        .expect("Failed to add videoconvert to filter bin");
                    filter_bin
                        .add(&capsfilter)
                        .expect("Failed to add capsfilter to filter bin");

                    videoconvert
                        .link(&capsfilter)
                        .expect("Failed to link videoconvert to capsfilter");

                    // Create ghost pads for the bin
                    let sink_pad = videoconvert
                        .static_pad("sink")
                        .expect("Failed to get sink pad");
                    let src_pad = capsfilter.static_pad("src").expect("Failed to get src pad");

                    let ghost_sink =
                        gst::GhostPad::with_target(&sink_pad).expect("Failed to create ghost sink");
                    let ghost_src =
                        gst::GhostPad::with_target(&src_pad).expect("Failed to create ghost src");

                    filter_bin
                        .add_pad(&ghost_sink)
                        .expect("Failed to add ghost sink");
                    filter_bin
                        .add_pad(&ghost_src)
                        .expect("Failed to add ghost src");

                    pb.set_property("video-filter", &filter_bin);
                    info!("Added video-filter bin to force RGBA for subtitle colorspace");
                }
            }

            info!("GStreamerPlayer::load_media() - Playbin created successfully");
            pb
        } else {
            error!("GStreamerPlayer::load_media() - No playbin element available!");
            return Err(anyhow::anyhow!(
                "No playbin element available - GStreamer plugins may not be properly installed"
            ));
        };

        // Use our stored video sink if available
        if let Some(sink) = self.video_sink.borrow().as_ref() {
            debug!("GStreamerPlayer::load_media() - Setting video sink on playbin");
            playbin.set_property("video-sink", sink);
            info!("GStreamerPlayer::load_media() - Video sink configured");
        } else {
            // Create a fallback video sink with proper conversion
            info!(
                "GStreamerPlayer::load_media() - No pre-configured sink, creating fallback with conversion"
            );

            // Create a bin with videoconvert and autovideosink for proper colorspace handling
            let bin = gst::Bin::new();

            if let Ok(videoconvert) = gst::ElementFactory::make("videoconvert")
                .name("videoconvert")
                .build()
            {
                if let Ok(autosink) = gst::ElementFactory::make("autovideosink")
                    .name("autovideosink")
                    .build()
                {
                    bin.add(&videoconvert).expect("Failed to add videoconvert");
                    bin.add(&autosink).expect("Failed to add autosink");

                    videoconvert
                        .link(&autosink)
                        .expect("Failed to link videoconvert to autosink");

                    let sink_pad = videoconvert
                        .static_pad("sink")
                        .expect("Failed to get sink pad");
                    let ghost_pad =
                        gst::GhostPad::with_target(&sink_pad).expect("Failed to create ghost pad");
                    bin.add_pad(&ghost_pad).expect("Failed to add ghost pad");

                    playbin.set_property("video-sink", &bin);
                    info!(
                        "GStreamerPlayer::load_media() - Fallback video sink with conversion configured"
                    );
                } else {
                    error!("GStreamerPlayer::load_media() - Failed to create autovideosink!");
                }
            } else {
                error!("GStreamerPlayer::load_media() - Failed to create videoconvert!");
            }
        }

        // Store the playbin
        self.playbin.replace(Some(playbin.clone()));
        debug!("GStreamerPlayer::load_media() - Playbin stored");

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
                }
                Ok(gst::StateChangeSuccess::Async) => {
                    info!("GStreamerPlayer::play() - Playbin state change is async, waiting...");
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

    pub fn get_video_widget(&self) -> Option<gtk4::Widget> {
        self.video_widget.borrow().clone()
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
            let n_video = playbin.property::<i32>("n-video");
            if n_video > 0
                && let Some(pad) =
                    playbin.emit_by_name::<Option<gst::Pad>>("get-video-pad", &[&0i32])
                && let Some(caps) = pad.current_caps()
                && let Some(structure) = caps.structure(0)
            {
                let width = structure.get::<i32>("width").ok();
                let height = structure.get::<i32>("height").ok();
                if let (Some(w), Some(h)) = (width, height) {
                    return Some((w, h));
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
                *state = PlayerState::Error(err.error().to_string());
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
            let n_audio = playbin.property::<i32>("n-audio");
            info!("Found {} audio tracks", n_audio);

            for i in 0..n_audio {
                // Get audio stream tags
                if let Some(tags) =
                    playbin.emit_by_name::<Option<gst::TagList>>("get-audio-tags", &[&i])
                {
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
            let n_text = playbin.property::<i32>("n-text");
            info!("Found {} subtitle tracks", n_text);

            // Add "None" option
            tracks.push((-1, "None".to_string()));

            for i in 0..n_text {
                // Get subtitle stream tags
                if let Some(tags) =
                    playbin.emit_by_name::<Option<gst::TagList>>("get-text-tags", &[&i])
                {
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

    pub async fn set_audio_track(&self, track_index: i32) -> Result<()> {
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            playbin.set_property("current-audio", track_index);
            info!("Set audio track to {}", track_index);
        }
        Ok(())
    }

    pub async fn set_subtitle_track(&self, track_index: i32) -> Result<()> {
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            if track_index < 0 {
                // Disable subtitles
                playbin.set_property_from_str(
                    "flags",
                    "soft-colorbalance+deinterlace+soft-volume+audio+video",
                );
                info!("Disabled subtitles");
            } else {
                // Enable subtitles and set track
                playbin.set_property_from_str(
                    "flags",
                    "soft-colorbalance+deinterlace+soft-volume+audio+video+text",
                );
                playbin.set_property("current-text", track_index);
                info!("Set subtitle track to {}", track_index);
            }
        }
        Ok(())
    }

    pub async fn get_current_audio_track(&self) -> i32 {
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            playbin.property::<i32>("current-audio")
        } else {
            -1
        }
    }

    pub async fn get_current_subtitle_track(&self) -> i32 {
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            // Check if we have any subtitle tracks available
            let n_text = playbin.property::<i32>("n-text");
            if n_text <= 0 {
                return -1; // No subtitle tracks available
            }

            // Get the current subtitle track
            // If subtitles are disabled, this will return -1
            playbin.property::<i32>("current-text")
        } else {
            -1
        }
    }
}
