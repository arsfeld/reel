use anyhow::Result;
use gtk4;
use std::time::Duration;
use tracing::info;

use super::gstreamer_player::PlayerState as GstPlayerState;
use super::mpv_player::PlayerState as MpvPlayerState;
use super::{GStreamerPlayer, MpvPlayer};
use crate::config::Config;

pub enum PlayerBackend {
    GStreamer,
    Mpv,
}

impl From<&str> for PlayerBackend {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "mpv" => PlayerBackend::Mpv,
            _ => PlayerBackend::GStreamer, // Default to GStreamer
        }
    }
}

#[derive(Debug, Clone)]
pub enum PlayerState {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopped,
    Error(String),
}

pub enum Player {
    GStreamer(GStreamerPlayer),
    Mpv(MpvPlayer),
}

impl Player {
    pub fn new(config: &Config) -> Result<Self> {
        let backend = PlayerBackend::from(config.playback.player_backend.as_str());

        match backend {
            PlayerBackend::GStreamer => {
                info!("Creating GStreamer player backend");
                Ok(Player::GStreamer(GStreamerPlayer::new()?))
            }
            PlayerBackend::Mpv => {
                info!("Creating MPV player backend");
                Ok(Player::Mpv(MpvPlayer::new()?))
            }
        }
    }

    pub fn create_video_widget(&self) -> gtk4::Widget {
        match self {
            Player::GStreamer(p) => p.create_video_widget(),
            Player::Mpv(p) => p.create_video_widget(),
        }
    }

    pub async fn load_media(&self, url: &str) -> Result<()> {
        match self {
            Player::GStreamer(p) => p.load_media(url, None).await,
            Player::Mpv(p) => p.load_media(url, None).await,
        }
    }

    pub async fn play(&self) -> Result<()> {
        match self {
            Player::GStreamer(p) => p.play().await,
            Player::Mpv(p) => p.play().await,
        }
    }

    pub async fn pause(&self) -> Result<()> {
        match self {
            Player::GStreamer(p) => p.pause().await,
            Player::Mpv(p) => p.pause().await,
        }
    }

    pub async fn stop(&self) -> Result<()> {
        match self {
            Player::GStreamer(p) => p.stop().await,
            Player::Mpv(p) => p.stop().await,
        }
    }

    pub async fn seek(&self, position: Duration) -> Result<()> {
        match self {
            Player::GStreamer(p) => p.seek(position).await,
            Player::Mpv(p) => p.seek(position).await,
        }
    }

    pub async fn get_position(&self) -> Option<Duration> {
        match self {
            Player::GStreamer(p) => p.get_position().await,
            Player::Mpv(p) => p.get_position().await,
        }
    }

    pub async fn get_duration(&self) -> Option<Duration> {
        match self {
            Player::GStreamer(p) => p.get_duration().await,
            Player::Mpv(p) => p.get_duration().await,
        }
    }

    pub async fn set_volume(&self, volume: f64) -> Result<()> {
        match self {
            Player::GStreamer(p) => p.set_volume(volume).await,
            Player::Mpv(p) => p.set_volume(volume).await,
        }
    }

    pub fn get_video_widget(&self) -> Option<gtk4::Widget> {
        match self {
            Player::GStreamer(p) => p.get_video_widget(),
            Player::Mpv(p) => p.get_video_widget(),
        }
    }

    pub async fn get_video_dimensions(&self) -> Option<(i32, i32)> {
        match self {
            Player::GStreamer(p) => p.get_video_dimensions().await,
            Player::Mpv(p) => p.get_video_dimensions().await,
        }
    }

    pub async fn get_state(&self) -> PlayerState {
        match self {
            Player::GStreamer(p) => match p.get_state().await {
                GstPlayerState::Idle => PlayerState::Idle,
                GstPlayerState::Loading => PlayerState::Loading,
                GstPlayerState::Playing => PlayerState::Playing,
                GstPlayerState::Paused => PlayerState::Paused,
                GstPlayerState::Stopped => PlayerState::Stopped,
                GstPlayerState::Error(e) => PlayerState::Error(e),
            },
            Player::Mpv(p) => match p.get_state().await {
                MpvPlayerState::Idle => PlayerState::Idle,
                MpvPlayerState::Loading => PlayerState::Loading,
                MpvPlayerState::Playing => PlayerState::Playing,
                MpvPlayerState::Paused => PlayerState::Paused,
                MpvPlayerState::Stopped => PlayerState::Stopped,
                MpvPlayerState::Error(e) => PlayerState::Error(e),
            },
        }
    }

    pub async fn get_audio_tracks(&self) -> Vec<(i32, String)> {
        match self {
            Player::GStreamer(p) => p.get_audio_tracks().await,
            Player::Mpv(p) => p.get_audio_tracks().await,
        }
    }

    pub async fn get_subtitle_tracks(&self) -> Vec<(i32, String)> {
        match self {
            Player::GStreamer(p) => p.get_subtitle_tracks().await,
            Player::Mpv(p) => p.get_subtitle_tracks().await,
        }
    }

    pub async fn set_audio_track(&self, track_index: i32) -> Result<()> {
        match self {
            Player::GStreamer(p) => p.set_audio_track(track_index).await,
            Player::Mpv(p) => p.set_audio_track(track_index).await,
        }
    }

    pub async fn set_subtitle_track(&self, track_index: i32) -> Result<()> {
        match self {
            Player::GStreamer(p) => p.set_subtitle_track(track_index).await,
            Player::Mpv(p) => p.set_subtitle_track(track_index).await,
        }
    }

    pub async fn get_current_audio_track(&self) -> i32 {
        match self {
            Player::GStreamer(p) => p.get_current_audio_track().await,
            Player::Mpv(p) => p.get_current_audio_track().await,
        }
    }

    pub async fn get_current_subtitle_track(&self) -> i32 {
        match self {
            Player::GStreamer(p) => p.get_current_subtitle_track().await,
            Player::Mpv(p) => p.get_current_subtitle_track().await,
        }
    }
}
