#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    Playing,
    Paused,
    #[allow(dead_code)]
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    Finished,
    #[allow(dead_code)]
    Stopped,
    #[allow(dead_code)]
    Error,
}
