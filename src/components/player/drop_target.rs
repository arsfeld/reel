use crate::player::backend::{FileKind, classify_file_extension};

/// Result of classifying a dropped file.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum DropAction {
    /// Load as a new video file
    PlayVideo(String),
    /// Load as subtitle for current video
    LoadSubtitle(String),
    /// Unsupported file type
    Unsupported,
}

/// Classify a dropped URI into an action.
///
/// - Video files → PlayVideo
/// - Subtitle files when video is playing → LoadSubtitle
/// - Subtitle files when no video → Unsupported
/// - Other files → Unsupported
#[allow(dead_code)]
pub fn classify_drop(uri: &str, has_video_loaded: bool) -> DropAction {
    // Strip file:// prefix if present
    let path = uri.strip_prefix("file://").unwrap_or(uri);

    // URL-decode the path
    let path = urldecode(path);

    match classify_file_extension(&path) {
        FileKind::Video => DropAction::PlayVideo(path),
        FileKind::Subtitle if has_video_loaded => DropAction::LoadSubtitle(path),
        _ => DropAction::Unsupported,
    }
}

/// Extract file paths from a text/uri-list drop.
#[allow(dead_code)]
pub fn parse_uri_list(data: &str) -> Vec<String> {
    data.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

/// Simple percent-decoding for file:// URIs.
fn urldecode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().and_then(hex_val);
            let lo = chars.next().and_then(hex_val);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                result.push((hi << 4 | lo) as char);
            }
        } else {
            result.push(b as char);
        }
    }
    result
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Check if a path refers to a subtitle file.
#[allow(dead_code)]
pub fn is_subtitle_path(path: &str) -> bool {
    classify_file_extension(path) == FileKind::Subtitle
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- classify_drop ---

    #[test]
    fn video_file_plays() {
        assert_eq!(
            classify_drop("file:///home/user/movie.mkv", false),
            DropAction::PlayVideo("/home/user/movie.mkv".into())
        );
    }

    #[test]
    fn video_file_replaces_current() {
        assert_eq!(
            classify_drop("file:///home/user/movie.mp4", true),
            DropAction::PlayVideo("/home/user/movie.mp4".into())
        );
    }

    #[test]
    fn subtitle_file_loads_when_video_playing() {
        assert_eq!(
            classify_drop("file:///home/user/movie.srt", true),
            DropAction::LoadSubtitle("/home/user/movie.srt".into())
        );
    }

    #[test]
    fn subtitle_file_unsupported_when_no_video() {
        assert_eq!(
            classify_drop("file:///home/user/movie.srt", false),
            DropAction::Unsupported
        );
    }

    #[test]
    fn unknown_file_unsupported() {
        assert_eq!(
            classify_drop("file:///home/user/readme.txt", false),
            DropAction::Unsupported
        );
    }

    #[test]
    fn handles_url_encoded_spaces() {
        assert_eq!(
            classify_drop("file:///home/user/my%20movie.mkv", false),
            DropAction::PlayVideo("/home/user/my movie.mkv".into())
        );
    }

    #[test]
    fn handles_path_without_file_prefix() {
        assert_eq!(
            classify_drop("/home/user/movie.mkv", false),
            DropAction::PlayVideo("/home/user/movie.mkv".into())
        );
    }

    // --- parse_uri_list ---

    #[test]
    fn parse_single_uri() {
        let uris = parse_uri_list("file:///movie.mkv\n");
        assert_eq!(uris, vec!["file:///movie.mkv"]);
    }

    #[test]
    fn parse_multiple_uris() {
        let uris = parse_uri_list("file:///a.mkv\nfile:///b.mp4\n");
        assert_eq!(uris, vec!["file:///a.mkv", "file:///b.mp4"]);
    }

    #[test]
    fn parse_ignores_comments() {
        let uris = parse_uri_list("# comment\nfile:///a.mkv\n");
        assert_eq!(uris, vec!["file:///a.mkv"]);
    }

    #[test]
    fn parse_ignores_empty_lines() {
        let uris = parse_uri_list("\nfile:///a.mkv\n\n");
        assert_eq!(uris, vec!["file:///a.mkv"]);
    }

    // --- urldecode ---

    #[test]
    fn decodes_percent_encoding() {
        assert_eq!(urldecode("hello%20world"), "hello world");
    }

    #[test]
    fn passthrough_no_encoding() {
        assert_eq!(urldecode("/home/user/movie.mkv"), "/home/user/movie.mkv");
    }

    // --- is_subtitle_path ---

    #[test]
    fn srt_is_subtitle_path() {
        assert!(is_subtitle_path("movie.srt"));
    }

    #[test]
    fn mkv_is_not_subtitle_path() {
        assert!(!is_subtitle_path("movie.mkv"));
    }
}
