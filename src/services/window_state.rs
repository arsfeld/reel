use std::fs;
use std::path::PathBuf;

/// Window state to persist across sessions.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowState {
    pub width: i32,
    pub height: i32,
    pub maximized: bool,
    pub volume: f64,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            maximized: false,
            volume: 100.0,
        }
    }
}

/// Get the path to the window state file.
#[allow(dead_code)]
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
#[allow(dead_code)]
pub fn save(state: &WindowState) -> Result<(), String> {
    let path = state_file_path().ok_or("Could not determine config directory")?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {e}"))?;
    }

    let content = serialize(state);
    fs::write(&path, content).map_err(|e| format!("Failed to write window state: {e}"))
}

/// Load window state from disk.
#[allow(dead_code)]
pub fn load() -> WindowState {
    let Some(path) = state_file_path() else {
        return WindowState::default();
    };

    let Ok(content) = fs::read_to_string(&path) else {
        return WindowState::default();
    };

    deserialize(&content).unwrap_or_default()
}

/// Serialize window state to TOML.
fn serialize(state: &WindowState) -> String {
    format!(
        "width = {}\nheight = {}\nmaximized = {}\nvolume = {:.1}\n",
        state.width, state.height, state.maximized, state.volume
    )
}

/// Deserialize window state from TOML.
fn deserialize(content: &str) -> Option<WindowState> {
    let mut state = WindowState::default();

    for line in content.lines() {
        let line = line.trim();
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "width" => state.width = value.parse().ok()?,
                "height" => state.height = value.parse().ok()?,
                "maximized" => state.maximized = value.parse().ok()?,
                "volume" => state.volume = value.parse().ok()?,
                _ => {} // ignore unknown keys
            }
        }
    }

    Some(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_default_state() {
        let state = WindowState::default();
        let toml = serialize(&state);
        assert!(toml.contains("width = 1280"));
        assert!(toml.contains("height = 720"));
        assert!(toml.contains("maximized = false"));
        assert!(toml.contains("volume = 100.0"));
    }

    #[test]
    fn serialize_custom_state() {
        let state = WindowState {
            width: 1920,
            height: 1080,
            maximized: true,
            volume: 75.5,
        };
        let toml = serialize(&state);
        assert!(toml.contains("width = 1920"));
        assert!(toml.contains("height = 1080"));
        assert!(toml.contains("maximized = true"));
        assert!(toml.contains("volume = 75.5"));
    }

    #[test]
    fn roundtrip_default() {
        let state = WindowState::default();
        let toml = serialize(&state);
        let parsed = deserialize(&toml).unwrap();
        assert_eq!(state, parsed);
    }

    #[test]
    fn roundtrip_custom() {
        let state = WindowState {
            width: 800,
            height: 600,
            maximized: true,
            volume: 42.0,
        };
        let toml = serialize(&state);
        let parsed = deserialize(&toml).unwrap();
        assert_eq!(state, parsed);
    }

    #[test]
    fn deserialize_ignores_unknown_keys() {
        let toml = "width = 800\nheight = 600\nfoo = bar\nmaximized = false\nvolume = 100.0\n";
        let state = deserialize(toml).unwrap();
        assert_eq!(state.width, 800);
        assert_eq!(state.height, 600);
    }

    #[test]
    fn deserialize_invalid_value_returns_none() {
        let toml = "width = notanumber\nheight = 600\nmaximized = false\nvolume = 100.0\n";
        assert!(deserialize(toml).is_none());
    }

    #[test]
    fn deserialize_empty_returns_defaults() {
        let state = deserialize("").unwrap();
        assert_eq!(state, WindowState::default());
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
        };

        // Save manually to temp dir
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        std::fs::write(&config_path, serialize(&state)).unwrap();

        // Read back
        let content = std::fs::read_to_string(&config_path).unwrap();
        let loaded = deserialize(&content).unwrap();
        assert_eq!(state, loaded);
    }
}
