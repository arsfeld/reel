#[cfg(feature = "gtk")]
pub mod factory;
#[cfg(feature = "gtk")]
pub mod gstreamer_player;
#[cfg(feature = "gtk")]
pub mod mpv_player;
#[cfg(feature = "gtk")]
pub mod traits;

#[cfg(feature = "gtk")]
pub use factory::{Player, PlayerState};
#[cfg(feature = "gtk")]
pub use gstreamer_player::GStreamerPlayer;
#[cfg(feature = "gtk")]
pub use mpv_player::{MpvPlayer, UpscalingMode};
