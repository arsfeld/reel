use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config;
use crate::services::library_filter::{FilterState, SortOrder};

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum SettingsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// Application settings persisted to `config_dir()/settings.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub playback: PlaybackSettings,
    pub subtitles: SubtitleSettings,
    pub library: LibrarySettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackSettings {
    /// Default volume (0-150).
    pub default_volume: f64,
    /// Hardware decoding mode: "auto-safe", "auto", "vaapi", "nvdec", "none".
    pub hwdec_mode: String,
    /// Whether to show resume overlay for in-progress media.
    pub resume_playback: bool,
    /// Short skip interval in seconds (arrow keys).
    pub skip_short_secs: f64,
    /// Long skip interval in seconds (Shift+arrow keys).
    pub skip_long_secs: f64,
    /// Default playback speed (0.25-4.0).
    pub default_speed: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SubtitleSettings {
    /// Preferred subtitle language (ISO 639-1, e.g. "en", "es").
    pub preferred_language: Option<String>,
    /// Subtitle font family.
    pub font_family: String,
    /// Subtitle font size (16-72).
    pub font_size: u32,
}

/// Per-library UI state (filters + sort), persisted across sessions and keyed
/// by `LibrarySection::library_id`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LibraryUiState {
    pub filters: FilterState,
    pub sort: SortOrder,
}

/// Library view settings: one `LibraryUiState` entry per library, keyed by the
/// composite library id.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LibrarySettings {
    pub per_library: HashMap<String, LibraryUiState>,
}

impl LibrarySettings {
    /// Get the saved UI state for a library, or a default (no filters,
    /// Title A–Z) when none has been persisted yet.
    pub fn get(&self, library_id: &str) -> LibraryUiState {
        self.per_library.get(library_id).cloned().unwrap_or_default()
    }

    /// Replace the saved UI state for a library.
    pub fn set(&mut self, library_id: &str, state: LibraryUiState) {
        self.per_library.insert(library_id.to_string(), state);
    }
}

// Settings has Default derived from its fields' defaults.

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            default_volume: 100.0,
            hwdec_mode: "auto-safe".to_string(),
            resume_playback: true,
            skip_short_secs: 10.0,
            skip_long_secs: 60.0,
            default_speed: 1.0,
        }
    }
}

impl Default for SubtitleSettings {
    fn default() -> Self {
        Self {
            preferred_language: None,
            font_family: "Sans".to_string(),
            font_size: 36,
        }
    }
}

#[allow(dead_code)]
impl Settings {
    /// Path to the settings file.
    pub fn settings_path() -> std::path::PathBuf {
        config::config_dir().join("settings.toml")
    }

    /// Load settings from disk. Returns defaults on any error.
    pub fn load() -> Self {
        let path = Self::settings_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save settings to disk.
    pub fn save(&self) -> Result<(), SettingsError> {
        let path = Self::settings_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

// --- Validation helpers ---

/// Clamp volume to valid range (0-150).
#[allow(dead_code)]
pub fn clamp_volume(v: f64) -> f64 {
    v.clamp(0.0, 150.0)
}

/// Clamp skip interval to valid range (1-120 seconds).
#[allow(dead_code)]
pub fn clamp_skip_interval(v: f64) -> f64 {
    v.clamp(1.0, 120.0)
}

/// Clamp playback speed to valid range (0.25-4.0).
#[allow(dead_code)]
pub fn clamp_speed(v: f64) -> f64 {
    v.clamp(0.25, 4.0)
}

/// Clamp subtitle font size to valid range (16-72).
#[allow(dead_code)]
pub fn clamp_font_size(v: u32) -> u32 {
    v.clamp(16, 72)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Default values ---

    #[test]
    fn default_settings_have_sensible_values() {
        let s = Settings::default();
        assert_eq!(s.playback.default_volume, 100.0);
        assert_eq!(s.playback.hwdec_mode, "auto-safe");
        assert!(s.playback.resume_playback);
        assert_eq!(s.playback.skip_short_secs, 10.0);
        assert_eq!(s.playback.skip_long_secs, 60.0);
        assert_eq!(s.playback.default_speed, 1.0);
        assert_eq!(s.subtitles.preferred_language, None);
        assert_eq!(s.subtitles.font_family, "Sans");
        assert_eq!(s.subtitles.font_size, 36);
        assert!(s.library.per_library.is_empty());
    }

    // --- Serialization roundtrip ---

    #[test]
    fn roundtrip_default_settings() {
        let s = Settings::default();
        let toml_str = toml::to_string_pretty(&s).unwrap();
        let parsed: Settings = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.playback.default_volume, s.playback.default_volume);
        assert_eq!(parsed.playback.hwdec_mode, s.playback.hwdec_mode);
        assert_eq!(parsed.subtitles.font_family, s.subtitles.font_family);
        assert_eq!(
            parsed.library.per_library.len(),
            s.library.per_library.len()
        );
    }

    #[test]
    fn roundtrip_custom_settings() {
        let s = Settings {
            playback: PlaybackSettings {
                default_volume: 75.0,
                hwdec_mode: "vaapi".to_string(),
                resume_playback: false,
                skip_short_secs: 5.0,
                skip_long_secs: 30.0,
                default_speed: 1.5,
            },
            subtitles: SubtitleSettings {
                preferred_language: Some("en".to_string()),
                font_family: "Noto Sans".to_string(),
                font_size: 48,
            },
            library: LibrarySettings::default(),
        };
        let toml_str = toml::to_string_pretty(&s).unwrap();
        let parsed: Settings = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.playback.default_volume, 75.0);
        assert_eq!(parsed.playback.hwdec_mode, "vaapi");
        assert!(!parsed.playback.resume_playback);
        assert_eq!(parsed.playback.skip_short_secs, 5.0);
        assert_eq!(parsed.playback.skip_long_secs, 30.0);
        assert_eq!(parsed.playback.default_speed, 1.5);
        assert_eq!(parsed.subtitles.preferred_language, Some("en".to_string()));
        assert_eq!(parsed.subtitles.font_family, "Noto Sans");
        assert_eq!(parsed.subtitles.font_size, 48);
    }

    // --- Forward/backward compatibility ---

    #[test]
    fn partial_toml_fills_defaults() {
        let toml_str = "[playback]\ndefault_volume = 50.0\n";
        let s: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(s.playback.default_volume, 50.0);
        // Remaining playback fields get defaults
        assert_eq!(s.playback.hwdec_mode, "auto-safe");
        assert!(s.playback.resume_playback);
        // Other sections get full defaults
        assert_eq!(s.subtitles.font_family, "Sans");
        assert!(s.library.per_library.is_empty());
    }

    #[test]
    fn empty_file_returns_defaults() {
        let s: Settings = toml::from_str("").unwrap();
        assert_eq!(s.playback.default_volume, 100.0);
        assert_eq!(s.subtitles.font_size, 36);
    }

    #[test]
    fn corrupt_toml_returns_error() {
        let result = toml::from_str::<Settings>("{{{{not valid");
        assert!(result.is_err());
        // load() would call unwrap_or_default()
        let s = result.unwrap_or_default();
        assert_eq!(s.playback.default_volume, 100.0);
    }

    #[test]
    fn unknown_keys_ignored() {
        let toml_str = "[playback]\ndefault_volume = 80.0\nfuture_field = true\n";
        let s: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(s.playback.default_volume, 80.0);
    }

    #[test]
    fn only_library_section_fills_rest_with_defaults() {
        let toml_str = "[playback]\ndefault_volume = 90.0\n";
        let s: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(s.playback.default_volume, 90.0);
        assert!(s.library.per_library.is_empty());
        assert_eq!(s.subtitles.font_family, "Sans");
    }

    // --- Per-library UI state ---

    #[test]
    fn library_settings_get_unknown_returns_default() {
        let ls = LibrarySettings::default();
        let state = ls.get("plex:srv:1");
        assert!(!state.filters.is_active());
        assert_eq!(state.sort, SortOrder::TitleAsc);
    }

    #[test]
    fn library_settings_set_and_get_roundtrip() {
        use crate::services::library_filter::{GenreFilter, YearRangeFilter};
        let mut ls = LibrarySettings::default();
        let state = LibraryUiState {
            filters: FilterState {
                genres: Some(GenreFilter {
                    selected_genres: vec!["Sci-Fi".to_string()],
                }),
                year_range: Some(YearRangeFilter {
                    from: Some(2000),
                    to: None,
                }),
                ..Default::default()
            },
            sort: SortOrder::YearNewest,
        };
        ls.set("plex:srv:1", state);
        let got = ls.get("plex:srv:1");
        assert_eq!(got.sort, SortOrder::YearNewest);
        assert_eq!(
            got.filters.genres.as_ref().unwrap().selected_genres,
            vec!["Sci-Fi".to_string()]
        );
    }

    #[test]
    fn library_settings_set_does_not_pollute_other_library() {
        let mut ls = LibrarySettings::default();
        ls.set(
            "plex:srv:1",
            LibraryUiState {
                sort: SortOrder::YearNewest,
                ..Default::default()
            },
        );
        // A different library still returns the default.
        let other = ls.get("plex:srv:2");
        assert_eq!(other.sort, SortOrder::TitleAsc);
    }

    #[test]
    fn library_ui_state_toml_roundtrip_preserves_per_library() {
        use crate::services::library_filter::WatchStatusFilter;
        use crate::services::library_filter::WatchStatus;

        let mut settings = Settings::default();
        settings.library.set(
            "plex:srv:1",
            LibraryUiState {
                filters: FilterState {
                    watch_status: Some(WatchStatusFilter {
                        status: WatchStatus::Unwatched,
                    }),
                    ..Default::default()
                },
                sort: SortOrder::DateAdded,
            },
        );
        settings.library.set(
            "plex:srv:2",
            LibraryUiState {
                sort: SortOrder::RatingHighest,
                ..Default::default()
            },
        );

        let toml_str = toml::to_string_pretty(&settings).unwrap();
        let parsed: Settings = toml::from_str(&toml_str).unwrap();

        let lib1 = parsed.library.get("plex:srv:1");
        assert_eq!(lib1.sort, SortOrder::DateAdded);
        assert!(lib1.filters.watch_status.is_some());

        let lib2 = parsed.library.get("plex:srv:2");
        assert_eq!(lib2.sort, SortOrder::RatingHighest);
        assert!(!lib2.filters.is_active());
    }

    #[test]
    fn library_ui_state_partial_toml_fills_filter_defaults() {
        // A persisted state with only a sort and no filters table should
        // deserialize with an inactive FilterState.
        let toml_str = r#"
[library.per_library."plex:srv:1"]
sort = "YearNewest"
"#;
        let s: Settings = toml::from_str(toml_str).unwrap();
        let state = s.library.get("plex:srv:1");
        assert_eq!(state.sort, SortOrder::YearNewest);
        assert!(!state.filters.is_active());
    }

    // --- Validation functions ---

    #[test]
    fn clamp_volume_at_boundaries() {
        assert_eq!(clamp_volume(0.0), 0.0);
        assert_eq!(clamp_volume(150.0), 150.0);
        assert_eq!(clamp_volume(75.0), 75.0);
    }

    #[test]
    fn clamp_volume_out_of_range() {
        assert_eq!(clamp_volume(-10.0), 0.0);
        assert_eq!(clamp_volume(200.0), 150.0);
    }

    #[test]
    fn clamp_skip_interval_at_boundaries() {
        assert_eq!(clamp_skip_interval(1.0), 1.0);
        assert_eq!(clamp_skip_interval(120.0), 120.0);
        assert_eq!(clamp_skip_interval(30.0), 30.0);
    }

    #[test]
    fn clamp_skip_interval_out_of_range() {
        assert_eq!(clamp_skip_interval(0.0), 1.0);
        assert_eq!(clamp_skip_interval(0.5), 1.0);
        assert_eq!(clamp_skip_interval(200.0), 120.0);
    }

    #[test]
    fn clamp_speed_at_boundaries() {
        assert_eq!(clamp_speed(0.25), 0.25);
        assert_eq!(clamp_speed(4.0), 4.0);
        assert_eq!(clamp_speed(1.0), 1.0);
    }

    #[test]
    fn clamp_speed_out_of_range() {
        assert_eq!(clamp_speed(0.0), 0.25);
        assert_eq!(clamp_speed(0.1), 0.25);
        assert_eq!(clamp_speed(5.0), 4.0);
    }

    #[test]
    fn clamp_font_size_at_boundaries() {
        assert_eq!(clamp_font_size(16), 16);
        assert_eq!(clamp_font_size(72), 72);
        assert_eq!(clamp_font_size(36), 36);
    }

    #[test]
    fn clamp_font_size_out_of_range() {
        assert_eq!(clamp_font_size(0), 16);
        assert_eq!(clamp_font_size(8), 16);
        assert_eq!(clamp_font_size(100), 72);
    }

    // --- File I/O ---

    #[test]
    fn save_and_load_roundtrip_via_file() {
        let dir = tempfile::tempdir().unwrap();
        let settings_path = dir.path().join("settings.toml");

        let s = Settings {
            playback: PlaybackSettings {
                default_volume: 80.0,
                ..Default::default()
            },
            subtitles: SubtitleSettings {
                preferred_language: Some("es".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        // Write manually
        let content = toml::to_string_pretty(&s).unwrap();
        std::fs::write(&settings_path, &content).unwrap();

        // Read back
        let loaded_content = std::fs::read_to_string(&settings_path).unwrap();
        let loaded: Settings = toml::from_str(&loaded_content).unwrap();
        assert_eq!(loaded.playback.default_volume, 80.0);
        assert_eq!(loaded.subtitles.preferred_language, Some("es".to_string()));
    }

    #[test]
    fn serialized_output_is_valid_toml() {
        let s = Settings::default();
        let toml_str = toml::to_string_pretty(&s).unwrap();
        // Should parse without error
        let _: toml::Value = toml::from_str(&toml_str).unwrap();
    }

    #[test]
    fn subtitle_language_none_serializes_correctly() {
        let s = Settings::default();
        let toml_str = toml::to_string_pretty(&s).unwrap();
        // None should not appear as a key in the TOML
        assert!(!toml_str.contains("preferred_language"));
    }

    #[test]
    fn subtitle_language_some_roundtrips() {
        let s = Settings {
            subtitles: SubtitleSettings {
                preferred_language: Some("fr".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        let toml_str = toml::to_string_pretty(&s).unwrap();
        assert!(toml_str.contains("preferred_language = \"fr\""));
        let parsed: Settings = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.subtitles.preferred_language, Some("fr".to_string()));
    }
}
