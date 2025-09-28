/// Common types used by player backends

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UpscalingMode {
    None,
    HighQuality,
    FSR,
    Anime,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZoomMode {
    Fit,         // Fit entire video in window (default, may show black bars)
    Fill,        // Fill window (may crop video)
    Zoom16_9,    // Crop to 16:9 aspect ratio
    Zoom4_3,     // Crop to 4:3 aspect ratio
    Zoom2_35,    // Crop to 2.35:1 (cinematic)
    Custom(f64), // Custom zoom level (1.0 = original, >1.0 = zoomed in)
}

impl UpscalingMode {
    pub fn to_string(&self) -> &'static str {
        match self {
            UpscalingMode::None => "None",
            UpscalingMode::HighQuality => "High Quality",
            UpscalingMode::FSR => "FSR",
            UpscalingMode::Anime => "Anime",
            UpscalingMode::Custom => "Custom",
        }
    }
}

impl Default for UpscalingMode {
    fn default() -> Self {
        Self::None
    }
}

impl ZoomMode {
    pub fn to_string(&self) -> String {
        match self {
            ZoomMode::Fit => "Fit".to_string(),
            ZoomMode::Fill => "Fill".to_string(),
            ZoomMode::Zoom16_9 => "16:9".to_string(),
            ZoomMode::Zoom4_3 => "4:3".to_string(),
            ZoomMode::Zoom2_35 => "2.35:1".to_string(),
            ZoomMode::Custom(level) => format!("{:.0}%", level * 100.0),
        }
    }

    pub fn zoom_level(&self) -> f64 {
        match self {
            ZoomMode::Fit => 1.0,
            ZoomMode::Fill => 1.0, // Will be calculated based on aspect ratios
            ZoomMode::Zoom16_9 => 1.0, // Will be calculated based on video
            ZoomMode::Zoom4_3 => 1.0, // Will be calculated based on video
            ZoomMode::Zoom2_35 => 1.0, // Will be calculated based on video
            ZoomMode::Custom(level) => *level,
        }
    }
}

impl Default for ZoomMode {
    fn default() -> Self {
        Self::Fit
    }
}
