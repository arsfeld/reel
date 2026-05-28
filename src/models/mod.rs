#[allow(dead_code)]
pub mod detail;
pub mod hub;
pub mod library;
pub mod media;
// Constructors land in U6/U7 (resolve_playback + play-path wiring); allow until
// then, mirroring `detail`.
#[allow(dead_code)]
pub mod playback;
pub mod source;
pub mod watch;
