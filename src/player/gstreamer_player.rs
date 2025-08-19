use anyhow::{Context, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer::glib;
use gstreamer_video as gst_video;
use gtk4::{self, prelude::*};
use gdk4 as gdk;
use std::sync::Arc;
use std::cell::RefCell;
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
        // Initialize GStreamer if not already done
        match gst::init() {
            Ok(_) => info!("GStreamer initialized successfully"),
            Err(e) => {
                error!("Failed to initialize GStreamer: {}", e);
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
        
        info!("Available playback-related elements: {:?}", playback_factories);
    }
    
    pub fn create_video_widget(&self) -> gtk4::Widget {
        info!("Creating video widget");
        
        // Create a GTK Picture widget for video display
        let picture = gtk4::Picture::new();
        picture.set_can_shrink(true);
        picture.set_vexpand(true);
        picture.set_hexpand(true);
        
        // Try to create gtk4paintablesink
        let gtksink_result = gst::ElementFactory::make("gtk4paintablesink")
            .name("videosink")
            .build();
        
        match gtksink_result {
            Ok(sink) => {
                info!("Successfully created gtk4paintablesink");
                // Get the paintable from the sink and set it on the picture
                let paintable = sink.property::<gdk::Paintable>("paintable");
                picture.set_paintable(Some(&paintable));
                // Store the sink for later use
                self.video_sink.replace(Some(sink));
            }
            Err(e) => {
                info!("gtk4paintablesink not available ({}), using fallback widget", e);
                // For fallback, we'll use a simple DrawingArea
                // The actual video will be rendered using autovideosink
                self.video_sink.replace(None);
            }
        }
        
        // Store the widget
        let widget = picture.upcast::<gtk4::Widget>();
        self.video_widget.replace(Some(widget.clone()));
        
        widget
    }
    
    pub async fn load_media(&self, url: &str, _video_sink: Option<&gst::Element>) -> Result<()> {
        info!("Loading media: {}", url);
        
        // Update state
        {
            let mut state = self.state.write().await;
            *state = PlayerState::Loading;
        }
        
        // Clear existing playbin if any
        if let Some(old_playbin) = self.playbin.borrow().as_ref() {
            old_playbin.set_state(gst::State::Null)
                .context("Failed to set old playbin to null state")?;
        }
        
        // Try to create playbin - note: playbin3 might not exist in all configurations
        let playbin = if gst::ElementFactory::find("playbin").is_some() {
            info!("Creating playbin element");
            gst::ElementFactory::make("playbin")
                .name("player")
                .property("uri", url)
                .build()
                .context("Failed to create playbin element")?
        } else {
            return Err(anyhow::anyhow!("No playbin element available - GStreamer plugins may not be properly installed"));
        };
        
        // Use our stored video sink if available
        if let Some(sink) = self.video_sink.borrow().as_ref() {
            debug!("Setting video sink on playbin");
            playbin.set_property("video-sink", sink);
        } else {
            // Try to create a fallback video sink
            info!("No gtk4paintablesink available, trying autovideosink");
            if let Ok(autosink) = gst::ElementFactory::make("autovideosink")
                .name("videosink")
                .build() {
                playbin.set_property("video-sink", &autosink);
            }
        }
        
        // Store the playbin
        self.playbin.replace(Some(playbin.clone()));
        
        // Set up message handling
        let bus = playbin.bus()
            .context("Failed to get playbin bus")?;
        
        let state_clone = self.state.clone();
        let _ = bus.add_watch(move |_, msg| {
            let state = state_clone.clone();
            let msg = msg.clone();
            glib::spawn_future_local(async move {
                Self::handle_bus_message(&msg, state).await;
            });
            glib::ControlFlow::Continue
        })
        .context("Failed to add bus watch")?;
        
        // Set to ready state first
        playbin.set_state(gst::State::Ready)
            .context("Failed to set playbin to ready state")?;
        
        Ok(())
    }
    
    pub async fn play(&self) -> Result<()> {
        debug!("Starting playback");
        
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            match playbin.set_state(gst::State::Playing) {
                Ok(gst::StateChangeSuccess::Success) => {
                    info!("Successfully set playbin to playing state");
                }
                Ok(gst::StateChangeSuccess::Async) => {
                    info!("Playbin state change is async, waiting...");
                }
                Ok(gst::StateChangeSuccess::NoPreroll) => {
                    info!("Playbin state change: no preroll");
                }
                Err(gst::StateChangeError) => {
                    // Get more details about the error
                    let state = playbin.state(gst::ClockTime::from_seconds(1));
                    error!("Failed to set playbin to playing state");
                    error!("Current state: {:?}", state);
                    
                    // Get the bus to check for error messages
                    if let Some(bus) = playbin.bus() {
                        while let Some(msg) = bus.pop() {
                            use gst::MessageView;
                            if let MessageView::Error(err) = msg.view() {
                                error!("Bus error: {} ({:?})", err.error(), err.debug());
                            }
                        }
                    }
                    
                    return Err(anyhow::anyhow!("Failed to set playbin to playing state"));
                }
            }
            
            let mut state = self.state.write().await;
            *state = PlayerState::Playing;
        }
        Ok(())
    }
    
    pub async fn pause(&self) -> Result<()> {
        debug!("Pausing playback");
        
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            playbin.set_state(gst::State::Paused)
                .context("Failed to set playbin to paused state")?;
            
            let mut state = self.state.write().await;
            *state = PlayerState::Paused;
        }
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        debug!("Stopping playback");
        
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            playbin.set_state(gst::State::Null)
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
            playbin.seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                gst::ClockTime::from_nseconds(position_ns as u64),
            )
            .context("Failed to seek")?;
        }
        Ok(())
    }
    
    pub async fn get_position(&self) -> Option<Duration> {
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            playbin.query_position::<gst::ClockTime>()
                .map(|pos| Duration::from_nanos(pos.nseconds()))
        } else {
            None
        }
    }
    
    pub async fn get_duration(&self) -> Option<Duration> {
        if let Some(playbin) = self.playbin.borrow().as_ref() {
            playbin.query_duration::<gst::ClockTime>()
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
    
    async fn handle_bus_message(msg: &gst::Message, state: Arc<RwLock<PlayerState>>) {
        use gst::MessageView;
        
        match msg.view() {
            MessageView::Eos(_) => {
                info!("End of stream");
                let mut state = state.write().await;
                *state = PlayerState::Stopped;
            }
            MessageView::Error(err) => {
                error!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                let mut state = state.write().await;
                *state = PlayerState::Error(err.error().to_string());
            }
            MessageView::StateChanged(state_changed) => {
                if state_changed.src().map(|s| s == state_changed.src().unwrap()).unwrap_or(false) {
                    debug!(
                        "State changed from {:?} to {:?}",
                        state_changed.old(),
                        state_changed.current()
                    );
                }
            }
            MessageView::Buffering(buffering) => {
                let percent = buffering.percent();
                debug!("Buffering: {}%", percent);
            }
            _ => {}
        }
    }
    
    pub async fn get_state(&self) -> PlayerState {
        self.state.read().await.clone()
    }
}