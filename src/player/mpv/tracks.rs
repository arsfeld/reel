use libmpv2::Mpv;

use crate::player::backend::{TrackInfo, TrackType};

/// Read and parse the track list from mpv.
///
/// mpv's `track-list/count` gives the number of tracks, and individual
/// properties can be queried via `track-list/N/property`.
pub fn get_track_list(mpv: &Mpv) -> Vec<TrackInfo> {
    let count = mpv.get_property::<i64>("track-list/count").unwrap_or(0);

    let mut tracks = Vec::new();
    for i in 0..count {
        if let Some(track) = parse_track_at_index(mpv, i) {
            tracks.push(track);
        }
    }
    tracks
}

fn parse_track_at_index(mpv: &Mpv, index: i64) -> Option<TrackInfo> {
    let prefix = format!("track-list/{index}");

    let type_str = mpv.get_property::<String>(&format!("{prefix}/type")).ok()?;

    let track_type = match type_str.as_str() {
        "audio" => TrackType::Audio,
        "video" => TrackType::Video,
        "sub" => TrackType::Subtitle,
        _ => return None,
    };

    let id = mpv.get_property::<i64>(&format!("{prefix}/id")).ok()?;

    Some(TrackInfo {
        id,
        track_type,
        title: mpv.get_property::<String>(&format!("{prefix}/title")).ok(),
        language: mpv.get_property::<String>(&format!("{prefix}/lang")).ok(),
        codec: mpv.get_property::<String>(&format!("{prefix}/codec")).ok(),
        selected: mpv
            .get_property::<bool>(&format!("{prefix}/selected"))
            .unwrap_or(false),
        default: mpv
            .get_property::<bool>(&format!("{prefix}/default"))
            .unwrap_or(false),
        forced: mpv
            .get_property::<bool>(&format!("{prefix}/forced"))
            .unwrap_or(false),
        external: mpv
            .get_property::<bool>(&format!("{prefix}/external"))
            .unwrap_or(false),
        channels: mpv
            .get_property::<i64>(&format!("{prefix}/demux-channel-count"))
            .ok(),
        width: mpv.get_property::<i64>(&format!("{prefix}/demux-w")).ok(),
        height: mpv.get_property::<i64>(&format!("{prefix}/demux-h")).ok(),
    })
}
