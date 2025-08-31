// Platform-agnostic player traits
// For now, we'll keep this minimal and add platform abstractions when needed

use anyhow::Result;
use async_trait::async_trait;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum PlayerState {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopped,
    Error(String),
}

// Platform-agnostic media player trait
// The actual video widget handling will be platform-specific
// and implemented in the platform modules
#[async_trait]
pub trait MediaPlayer: Send + Sync {
    async fn load_media(&self, url: &str) -> Result<()>;
    async fn play(&self) -> Result<()>;
    async fn pause(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn seek(&self, position: Duration) -> Result<()>;
    async fn get_position(&self) -> Option<Duration>;
    async fn get_duration(&self) -> Option<Duration>;
    async fn set_volume(&self, volume: f64) -> Result<()>;
    async fn get_video_dimensions(&self) -> Option<(i32, i32)>;
    async fn get_state(&self) -> PlayerState;
    async fn get_audio_tracks(&self) -> Vec<(i32, String)>;
    async fn get_subtitle_tracks(&self) -> Vec<(i32, String)>;
    async fn set_audio_track(&self, track_index: i32) -> Result<()>;
    async fn set_subtitle_track(&self, track_index: i32) -> Result<()>;
    async fn get_current_audio_track(&self) -> i32;
    async fn get_current_subtitle_track(&self) -> i32;
    async fn get_buffer_percentage(&self) -> Option<f64>;
}

// Platform-specific video widget trait
// Each platform will implement this differently
pub trait PlatformVideoWidget: Send + Sync {
    // Platform-specific implementation will be added per platform
}
