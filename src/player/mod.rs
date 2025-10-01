pub mod controller;
pub mod factory;
#[cfg(feature = "gstreamer")]
pub mod gstreamer_player;
#[cfg(feature = "mpv")]
pub mod mpv_player;
pub mod types;

pub use controller::{PlayerController, PlayerHandle};
pub use factory::Player;
#[allow(unused_imports)]
pub use factory::PlayerState;
pub use types::{UpscalingMode, ZoomMode};

#[cfg(feature = "gstreamer")]
pub use gstreamer_player::GStreamerPlayer;
#[cfg(feature = "mpv")]
pub use mpv_player::MpvPlayer;
