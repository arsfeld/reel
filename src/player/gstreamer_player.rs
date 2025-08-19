use anyhow::{Context, Result};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer::glib;
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
    pipeline: gst::Pipeline,
    state: Arc<RwLock<PlayerState>>,
}

impl GStreamerPlayer {
    pub fn new() -> Result<Self> {
        let pipeline = gst::Pipeline::new();
        
        Ok(Self {
            pipeline,
            state: Arc::new(RwLock::new(PlayerState::Idle)),
        })
    }
    
    pub async fn load_media(&self, url: &str) -> Result<()> {
        info!("Loading media: {}", url);
        
        // Update state
        {
            let mut state = self.state.write().await;
            *state = PlayerState::Loading;
        }
        
        // Clear existing pipeline
        self.pipeline.set_state(gst::State::Null)
            .context("Failed to set pipeline to null state")?;
        
        // Create playbin element
        let playbin = gst::ElementFactory::make("playbin3")
            .property("uri", url)
            .build()
            .context("Failed to create playbin")?;
        
        // Add to pipeline
        self.pipeline.add(&playbin)
            .context("Failed to add playbin to pipeline")?;
        
        // Set up message handling
        let bus = self.pipeline.bus()
            .context("Failed to get pipeline bus")?;
        
        let state_clone = self.state.clone();
        bus.add_watch(move |_, msg| {
            let state = state_clone.clone();
            let msg = msg.clone();
            glib::spawn_future_local(async move {
                Self::handle_bus_message(&msg, state).await;
            });
            glib::ControlFlow::Continue
        })
        .context("Failed to add bus watch")?;
        
        // Set to ready state
        self.pipeline.set_state(gst::State::Ready)
            .context("Failed to set pipeline to ready state")?;
        
        Ok(())
    }
    
    pub async fn play(&self) -> Result<()> {
        debug!("Starting playback");
        
        self.pipeline.set_state(gst::State::Playing)
            .context("Failed to set pipeline to playing state")?;
        
        let mut state = self.state.write().await;
        *state = PlayerState::Playing;
        
        Ok(())
    }
    
    pub async fn pause(&self) -> Result<()> {
        debug!("Pausing playback");
        
        self.pipeline.set_state(gst::State::Paused)
            .context("Failed to set pipeline to paused state")?;
        
        let mut state = self.state.write().await;
        *state = PlayerState::Paused;
        
        Ok(())
    }
    
    pub async fn stop(&self) -> Result<()> {
        debug!("Stopping playback");
        
        self.pipeline.set_state(gst::State::Null)
            .context("Failed to set pipeline to null state")?;
        
        let mut state = self.state.write().await;
        *state = PlayerState::Stopped;
        
        Ok(())
    }
    
    pub async fn seek(&self, position: Duration) -> Result<()> {
        debug!("Seeking to {:?}", position);
        
        let position_ns = position.as_nanos() as i64;
        
        self.pipeline.seek_simple(
            gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
            gst::ClockTime::from_nseconds(position_ns as u64),
        )
        .context("Failed to seek")?;
        
        Ok(())
    }
    
    pub async fn get_position(&self) -> Option<Duration> {
        self.pipeline.query_position::<gst::ClockTime>()
            .map(|pos| Duration::from_nanos(pos.nseconds()))
    }
    
    pub async fn get_duration(&self) -> Option<Duration> {
        self.pipeline.query_duration::<gst::ClockTime>()
            .map(|dur| Duration::from_nanos(dur.nseconds()))
    }
    
    pub async fn set_volume(&self, volume: f64) -> Result<()> {
        if let Some(playbin) = self.pipeline.by_name("playbin3") {
            playbin.set_property("volume", volume);
        }
        Ok(())
    }
    
    pub fn get_video_widget(&self) -> Option<gtk4::Widget> {
        // TODO: Create and return a GTK widget for video display
        None
    }
    
    async fn handle_bus_message(msg: &gst::Message, state: Arc<RwLock<PlayerState>>) {
        use gst::MessageView;
        
        match msg.view() {
            MessageView::Eos(..) => {
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
                if let Some(element) = msg.src() {
                    if element.type_() == gst::Pipeline::static_type() {
                        debug!(
                            "Pipeline state changed from {:?} to {:?}",
                            state_changed.old(),
                            state_changed.current()
                        );
                    }
                }
            }
            MessageView::Buffering(buffering) => {
                let percent = buffering.percent();
                debug!("Buffering: {}%", percent);
            }
            _ => {}
        }
    }
}