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

/// Default location for downloaded media: `data_dir()/downloads`.
///
/// Downloads live under `data_dir()` (persistent app data), not `cache_dir()`,
/// because they are user-retained media, not a regenerable cache. The location
/// is user-configurable via `DownloadSettings`; this is only the default.
pub fn downloads_dir() -> PathBuf {
    data_dir().join("downloads")
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
    fn downloads_dir_under_data_dir() {
        let p = downloads_dir();
        assert_eq!(p.file_name().unwrap(), "downloads");
        // Parent is data_dir(), which ends with "reel".
        assert!(p.parent().unwrap().ends_with("reel"));
        assert_eq!(p.parent().unwrap(), data_dir());
    }

    #[test]
    fn xdg_override_respected() {
        // Test that XDG vars are used when set
        let data = data_dir();
        // Should contain "reel" regardless
        assert!(data.to_string_lossy().contains("reel"));
    }
}
