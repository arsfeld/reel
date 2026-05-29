//! External subtitle file discovery helpers.

use std::path::{Path, PathBuf};

const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "ass", "ssa", "vtt", "sub", "idx"];

/// Returns true when `path` has a known subtitle extension.
pub fn is_subtitle_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| SUBTITLE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
}

/// Find subtitle files in the same directory as `video_path` with a matching stem.
///
/// Matches `movie.mkv` → `movie.srt`, `movie.en.srt`, `movie.japanese.ass`, etc.
pub fn find_matching_subtitles(video_path: &Path) -> Vec<PathBuf> {
    let Some(parent) = video_path.parent() else {
        return Vec::new();
    };
    let Some(stem) = video_path.file_stem().and_then(|s| s.to_str()) else {
        return Vec::new();
    };

    let Ok(entries) = std::fs::read_dir(parent) else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_subtitle_extension(&path) {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if name == stem || name.starts_with(&format!("{stem}.")) {
            matches.push(path);
        }
    }
    matches.sort();
    matches
}

/// Pick the best external subtitle from matches, preferring a language suffix when given.
pub fn best_matching_subtitle(
    matches: &[PathBuf],
    preferred_lang: Option<&str>,
) -> Option<PathBuf> {
    if matches.is_empty() {
        return None;
    }
    if let Some(lang) = preferred_lang.filter(|s| !s.is_empty()) {
        let lang_lower = lang.to_lowercase();
        if let Some(found) = matches.iter().find(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|stem| stem.to_lowercase().contains(&lang_lower))
        }) {
            return Some(found.clone());
        }
    }
    matches.first().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn is_subtitle_extension_recognizes_common_formats() {
        assert!(is_subtitle_extension(Path::new("foo.srt")));
        assert!(is_subtitle_extension(Path::new("foo.ASS")));
        assert!(!is_subtitle_extension(Path::new("foo.mkv")));
    }

    #[test]
    fn find_matching_subtitles_by_stem() {
        let dir = TempDir::new().unwrap();
        let video = dir.path().join("Movie Name (2024).mkv");
        fs::write(&video, b"").unwrap();
        fs::write(dir.path().join("Movie Name (2024).en.srt"), b"").unwrap();
        fs::write(dir.path().join("Movie Name (2024).srt"), b"").unwrap();
        fs::write(dir.path().join("Other.srt"), b"").unwrap();

        let found = find_matching_subtitles(&video);
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|p| {
            p.file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("Movie Name (2024)")
        }));
    }

    #[test]
    fn find_matching_subtitles_empty_when_no_matches() {
        let dir = TempDir::new().unwrap();
        let video = dir.path().join("clip.mkv");
        fs::write(&video, b"").unwrap();
        assert!(find_matching_subtitles(&video).is_empty());
    }

    #[test]
    fn best_matching_subtitle_prefers_language_suffix() {
        let dir = TempDir::new().unwrap();
        let plain = dir.path().join("movie.srt");
        let spanish = dir.path().join("movie.es.srt");
        let matches = vec![plain, spanish.clone()];
        assert_eq!(best_matching_subtitle(&matches, Some("es")), Some(spanish));
    }
}
