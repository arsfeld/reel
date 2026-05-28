use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum WindowStateError {
    #[error("Could not determine config directory")]
    NoConfigDir,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// Window state to persist across sessions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {
    pub width: i32,
    pub height: i32,
    pub maximized: bool,
    pub volume: f64,
    pub view_mode: String,
    pub grid_density: String,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            maximized: false,
            volume: 100.0,
            view_mode: "grid".to_string(),
            grid_density: "medium".to_string(),
        }
    }
}

/// Get the path to the window state file.
fn state_file_path() -> Option<PathBuf> {
    let config_dir = dirs_config_dir()?;
    Some(config_dir.join("reel").join("window.toml"))
}

/// Platform-independent config dir (wraps XDG).
fn dirs_config_dir() -> Option<PathBuf> {
    std::env::var("XDG_CONFIG_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".config"))
        })
}

/// Save window state to disk.
pub fn save(state: &WindowState) -> Result<(), WindowStateError> {
    let path = state_file_path().ok_or(WindowStateError::NoConfigDir)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = toml::to_string_pretty(state)?;
    fs::write(&path, content)?;
    Ok(())
}

/// Load window state from disk. Returns defaults on any error.
pub fn load() -> WindowState {
    let Some(path) = state_file_path() else {
        return WindowState::default();
    };

    let Ok(content) = fs::read_to_string(&path) else {
        return WindowState::default();
    };

    toml::from_str(&content).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_default_state() {
        let state = WindowState::default();
        let toml_str = toml::to_string_pretty(&state).unwrap();
        assert!(toml_str.contains("width = 1280"));
        assert!(toml_str.contains("height = 720"));
        assert!(toml_str.contains("maximized = false"));
        assert!(toml_str.contains("volume = 100.0"));
        assert!(toml_str.contains("view_mode = \"grid\""));
        assert!(toml_str.contains("grid_density = \"medium\""));
    }

    #[test]
    fn serialize_custom_state() {
        let state = WindowState {
            width: 1920,
            height: 1080,
            maximized: true,
            volume: 75.5,
            view_mode: "list".to_string(),
            grid_density: "large".to_string(),
        };
        let toml_str = toml::to_string_pretty(&state).unwrap();
        assert!(toml_str.contains("width = 1920"));
        assert!(toml_str.contains("height = 1080"));
        assert!(toml_str.contains("maximized = true"));
        assert!(toml_str.contains("volume = 75.5"));
        assert!(toml_str.contains("view_mode = \"list\""));
        assert!(toml_str.contains("grid_density = \"large\""));
    }

    #[test]
    fn roundtrip_default() {
        let state = WindowState::default();
        let toml_str = toml::to_string_pretty(&state).unwrap();
        let parsed: WindowState = toml::from_str(&toml_str).unwrap();
        assert_eq!(state, parsed);
    }

    #[test]
    fn roundtrip_custom() {
        let state = WindowState {
            width: 800,
            height: 600,
            maximized: true,
            volume: 42.0,
            view_mode: "list".to_string(),
            grid_density: "small".to_string(),
        };
        let toml_str = toml::to_string_pretty(&state).unwrap();
        let parsed: WindowState = toml::from_str(&toml_str).unwrap();
        assert_eq!(state, parsed);
    }

    #[test]
    fn deserialize_ignores_unknown_keys() {
        let toml_str =
            "width = 800\nheight = 600\nfoo = \"bar\"\nmaximized = false\nvolume = 100.0\n";
        let state: WindowState = toml::from_str(toml_str).unwrap();
        assert_eq!(state.width, 800);
        assert_eq!(state.height, 600);
    }

    #[test]
    fn deserialize_invalid_value_returns_error() {
        let toml_str = "width = \"notanumber\"\nheight = 600\nmaximized = false\nvolume = 100.0\n";
        assert!(toml::from_str::<WindowState>(toml_str).is_err());
    }

    #[test]
    fn deserialize_empty_returns_defaults() {
        let state: WindowState = toml::from_str("").unwrap();
        assert_eq!(state, WindowState::default());
    }

    #[test]
    fn deserialize_partial_fills_defaults() {
        let toml_str = "width = 1600\nheight = 900\n";
        let state: WindowState = toml::from_str(toml_str).unwrap();
        assert_eq!(state.width, 1600);
        assert_eq!(state.height, 900);
        // Remaining fields get defaults
        assert!(!state.maximized);
        assert_eq!(state.volume, 100.0);
        assert_eq!(state.view_mode, "grid");
        assert_eq!(state.grid_density, "medium");
    }

    #[test]
    fn deserialize_corrupt_toml_falls_back_to_defaults() {
        let bad_toml = "{{{{not valid toml!!!!";
        let result = toml::from_str::<WindowState>(bad_toml);
        assert!(result.is_err());
        // load() would call unwrap_or_default()
        let state = result.unwrap_or_default();
        assert_eq!(state, WindowState::default());
    }

    #[test]
    fn backward_compat_with_old_format() {
        // The old hand-rolled format is valid TOML, so serde should parse it
        let old_format = "width = 1600\nheight = 900\nmaximized = false\nvolume = 80.0\nview_mode = \"list\"\ngrid_density = \"large\"\n";
        let state: WindowState = toml::from_str(old_format).unwrap();
        assert_eq!(state.width, 1600);
        assert_eq!(state.height, 900);
        assert_eq!(state.volume, 80.0);
        assert_eq!(state.view_mode, "list");
        assert_eq!(state.grid_density, "large");
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("reel").join("window.toml");

        let state = WindowState {
            width: 1600,
            height: 900,
            maximized: false,
            volume: 80.0,
            view_mode: "list".to_string(),
            grid_density: "large".to_string(),
        };

        // Save manually to temp dir
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        let content = toml::to_string_pretty(&state).unwrap();
        std::fs::write(&config_path, content).unwrap();

        // Read back
        let content = std::fs::read_to_string(&config_path).unwrap();
        let loaded: WindowState = toml::from_str(&content).unwrap();
        assert_eq!(state, loaded);
    }
}
