use std::path::{Path, PathBuf};

/// Subtitle file extensions we recognize.
const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "ass", "ssa", "vtt", "sub", "idx"];

/// Given a video file path, find matching subtitle files in the same directory.
///
/// Matching rules:
/// - Same filename stem with a subtitle extension (e.g., `movie.srt` for `movie.mkv`)
/// - Same stem + language suffix (e.g., `movie.en.srt`, `movie.english.ass`)
#[allow(dead_code)]
pub fn find_matching_subtitles(video_path: &Path) -> Vec<PathBuf> {
    let Some(parent) = video_path.parent() else {
        return vec![];
    };
    let Some(stem) = video_path.file_stem().and_then(|s| s.to_str()) else {
        return vec![];
    };

    let Ok(entries) = std::fs::read_dir(parent) else {
        return vec![];
    };

    let mut matches = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };

        if !SUBTITLE_EXTENSIONS
            .iter()
            .any(|&se| se.eq_ignore_ascii_case(ext))
        {
            continue;
        }

        let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };

        // Exact stem match: movie.srt for movie.mkv
        if file_stem.eq_ignore_ascii_case(stem) {
            matches.push(path);
            continue;
        }

        // Language suffix match: movie.en.srt, movie.english.srt for movie.mkv
        if file_stem.starts_with(stem) && file_stem.as_bytes().get(stem.len()) == Some(&b'.') {
            matches.push(path);
        }
    }

    matches.sort();
    matches
}

/// Check if a file extension is a subtitle format.
#[allow(dead_code)]
pub fn is_subtitle_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            SUBTITLE_EXTENSIONS
                .iter()
                .any(|&se| se.eq_ignore_ascii_case(ext))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn touch(dir: &Path, name: &str) {
        fs::write(dir.join(name), "").unwrap();
    }

    // --- find_matching_subtitles ---

    #[test]
    fn finds_exact_stem_match() {
        let dir = create_temp_dir();
        touch(dir.path(), "movie.mkv");
        touch(dir.path(), "movie.srt");

        let subs = find_matching_subtitles(&dir.path().join("movie.mkv"));
        assert_eq!(subs.len(), 1);
        assert!(subs[0].ends_with("movie.srt"));
    }

    #[test]
    fn finds_multiple_subtitle_formats() {
        let dir = create_temp_dir();
        touch(dir.path(), "movie.mkv");
        touch(dir.path(), "movie.srt");
        touch(dir.path(), "movie.ass");

        let subs = find_matching_subtitles(&dir.path().join("movie.mkv"));
        assert_eq!(subs.len(), 2);
    }

    #[test]
    fn finds_language_suffixed_subtitles() {
        let dir = create_temp_dir();
        touch(dir.path(), "movie.mkv");
        touch(dir.path(), "movie.en.srt");
        touch(dir.path(), "movie.japanese.ass");

        let subs = find_matching_subtitles(&dir.path().join("movie.mkv"));
        assert_eq!(subs.len(), 2);
    }

    #[test]
    fn ignores_non_subtitle_files() {
        let dir = create_temp_dir();
        touch(dir.path(), "movie.mkv");
        touch(dir.path(), "movie.txt");
        touch(dir.path(), "movie.nfo");
        touch(dir.path(), "movie.jpg");

        let subs = find_matching_subtitles(&dir.path().join("movie.mkv"));
        assert!(subs.is_empty());
    }

    #[test]
    fn ignores_different_stem() {
        let dir = create_temp_dir();
        touch(dir.path(), "movie.mkv");
        touch(dir.path(), "other.srt");

        let subs = find_matching_subtitles(&dir.path().join("movie.mkv"));
        assert!(subs.is_empty());
    }

    #[test]
    fn empty_directory_returns_empty() {
        let dir = create_temp_dir();
        touch(dir.path(), "movie.mkv");

        let subs = find_matching_subtitles(&dir.path().join("movie.mkv"));
        assert!(subs.is_empty());
    }

    #[test]
    fn nonexistent_parent_returns_empty() {
        let subs = find_matching_subtitles(Path::new("/nonexistent/movie.mkv"));
        assert!(subs.is_empty());
    }

    #[test]
    fn case_insensitive_extension_match() {
        let dir = create_temp_dir();
        touch(dir.path(), "movie.mkv");
        touch(dir.path(), "movie.SRT");

        let subs = find_matching_subtitles(&dir.path().join("movie.mkv"));
        assert_eq!(subs.len(), 1);
    }

    #[test]
    fn does_not_match_partial_stem() {
        let dir = create_temp_dir();
        touch(dir.path(), "movie.mkv");
        touch(dir.path(), "movie2.srt");
        touch(dir.path(), "moviex.srt");

        let subs = find_matching_subtitles(&dir.path().join("movie.mkv"));
        assert!(subs.is_empty());
    }

    #[test]
    fn all_subtitle_extensions_recognized() {
        let dir = create_temp_dir();
        touch(dir.path(), "movie.mkv");
        touch(dir.path(), "movie.srt");
        touch(dir.path(), "movie.ass");
        touch(dir.path(), "movie.ssa");
        touch(dir.path(), "movie.vtt");
        touch(dir.path(), "movie.sub");
        touch(dir.path(), "movie.idx");

        let subs = find_matching_subtitles(&dir.path().join("movie.mkv"));
        assert_eq!(subs.len(), 6);
    }

    // --- is_subtitle_extension ---

    #[test]
    fn srt_is_subtitle() {
        assert!(is_subtitle_extension(Path::new("movie.srt")));
    }

    #[test]
    fn ass_is_subtitle() {
        assert!(is_subtitle_extension(Path::new("movie.ass")));
    }

    #[test]
    fn mkv_is_not_subtitle() {
        assert!(!is_subtitle_extension(Path::new("movie.mkv")));
    }

    #[test]
    fn no_extension_is_not_subtitle() {
        assert!(!is_subtitle_extension(Path::new("movie")));
    }
}
