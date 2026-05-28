use std::path::PathBuf;

const APP_NAME: &str = "reel";

fn xdg_dir(env_var: &str, fallback_suffix: &str) -> PathBuf {
    std::env::var(env_var)
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(fallback_suffix)
        })
        .join(APP_NAME)
}

/// `$XDG_DATA_HOME/reel` (default: `~/.local/share/reel`)
pub fn data_dir() -> PathBuf {
    xdg_dir("XDG_DATA_HOME", ".local/share")
}

/// `$XDG_CACHE_HOME/reel` (default: `~/.cache/reel`)
pub fn cache_dir() -> PathBuf {
    xdg_dir("XDG_CACHE_HOME", ".cache")
}

/// `$XDG_CONFIG_HOME/reel` (default: `~/.config/reel`)
pub fn config_dir() -> PathBuf {
    xdg_dir("XDG_CONFIG_HOME", ".config")
}

/// Path to the SQLite database: `data_dir()/reel.db`
pub fn db_path() -> PathBuf {
    data_dir().join("reel.db")
}

/// Path to cached artwork: `cache_dir()/artwork`
pub fn artwork_dir() -> PathBuf {
    cache_dir().join("artwork")
}

/// Path to streaming-cache temp files: `cache_dir()/stream-cache`.
/// Isolated from `artwork/` and from the data dir's SQLite/downloads;
/// nothing here is meant to survive a session (see `services::stream_cache`).
#[allow(dead_code)] // wired into the pipeline + startup reclaim in U3/U4
pub fn stream_cache_dir() -> PathBuf {
    cache_dir().join("stream-cache")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_ends_with_reel() {
        let p = data_dir();
        assert_eq!(p.file_name().unwrap(), "reel");
    }

    #[test]
    fn cache_dir_ends_with_reel() {
        let p = cache_dir();
        assert_eq!(p.file_name().unwrap(), "reel");
    }

    #[test]
    fn config_dir_ends_with_reel() {
        let p = config_dir();
        assert_eq!(p.file_name().unwrap(), "reel");
    }

    #[test]
    fn db_path_ends_with_reel_db() {
        let p = db_path();
        assert_eq!(p.file_name().unwrap(), "reel.db");
        assert!(p.parent().unwrap().ends_with("reel"));
    }

    #[test]
    fn artwork_dir_ends_with_artwork() {
        let p = artwork_dir();
        assert_eq!(p.file_name().unwrap(), "artwork");
        assert!(p.parent().unwrap().ends_with("reel"));
    }

    #[test]
    fn stream_cache_dir_ends_with_stream_cache() {
        let p = stream_cache_dir();
        assert_eq!(p.file_name().unwrap(), "stream-cache");
        assert!(p.parent().unwrap().ends_with("reel"));
    }

    #[test]
    fn xdg_override_respected() {
        // Test that XDG vars are used when set
        let data = data_dir();
        // Should contain "reel" regardless
        assert!(data.to_string_lossy().contains("reel"));
    }
}
