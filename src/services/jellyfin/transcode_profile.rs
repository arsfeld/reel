//! Jellyfin client capability profile — the `DeviceProfile` sent in the
//! `PlaybackInfo` request body (U2). The Jellyfin analog of Plex's
//! `client_profile_extra`: it declares what `playbin3` can direct-play so the
//! server only transcodes what it must, and carries the bitrate/resolution cap
//! from the selected quality rung.
//!
//! Pure data — no GStreamer/GTK/HTTP imports (services layer is pure Rust). The
//! capability ceiling deliberately mirrors `src/services/plex/transcode_profile.rs`
//! so the two backends transcode under the same conditions: HEVC is allowed only
//! at <= 8-bit, so 10-bit / HDR / Dolby Vision content routes to an SDR transcode
//! rather than a washed-out direct play. The manual quality override is the
//! safety net when this table mis-guesses.

use serde_json::{Value, json};

use crate::models::playback::{QualityPreset, QualitySelection};

/// The effective quality cap for a request: a manual rung, the remote default
/// when Auto on a remote connection, or `None` (uncapped) when Auto on local.
///
/// Duplicated from the Plex `effective_preset` rather than shared, keeping the
/// backends' transcode code independent (KTD7 — the cost is five lines).
fn effective_preset(quality: QualitySelection, is_remote: bool) -> Option<QualityPreset> {
    match quality {
        QualitySelection::Manual(p) => Some(p),
        QualitySelection::Auto if is_remote => Some(QualityPreset::REMOTE_DEFAULT),
        QualitySelection::Auto => None,
    }
}

/// `(width, height)` resolution cap for a preset, mirroring the Plex ladder, or
/// `None` for `Original` (source resolution, no cap).
fn resolution_cap(preset: QualityPreset) -> Option<(u32, u32)> {
    match preset {
        QualityPreset::Original => None,
        QualityPreset::P1080Mbps20 | QualityPreset::P1080Mbps12 | QualityPreset::P1080Mbps8 => {
            Some((1920, 1080))
        }
        QualityPreset::P720Mbps4 | QualityPreset::P720Mbps3 => Some((1280, 720)),
        QualityPreset::P480Mbps2 | QualityPreset::P480Kbps720 => Some((720, 480)),
    }
}

/// Build the `DeviceProfile` JSON for a `PlaybackInfo` request body, plus the
/// resolved `MaxStreamingBitrate` in **bits/sec** (`None` = uncapped). The
/// caller (U3) embeds the profile in the body and also sets the body-level
/// `MaxStreamingBitrate` from the returned value.
pub fn device_profile(quality: QualitySelection, is_remote: bool) -> (Value, Option<u64>) {
    let preset = effective_preset(quality, is_remote);
    let max_bitrate_bps = preset
        .and_then(|p| p.max_video_bitrate_kbps())
        .map(|kbps| kbps as u64 * 1000);

    // Codec ceilings mirroring the Plex profile: H.264 up to level 5.2 (4K),
    // HEVC capped at 8-bit so HDR/Dolby Vision route to an SDR transcode.
    let mut codec_profiles = vec![
        json!({
            "Type": "Video",
            "Codec": "h264",
            "Conditions": [
                {"Condition": "LessThanEqual", "Property": "VideoLevel", "Value": "52", "IsRequired": false}
            ]
        }),
        json!({
            "Type": "Video",
            "Codec": "hevc",
            "Conditions": [
                {"Condition": "LessThanEqual", "Property": "VideoBitDepth", "Value": "8", "IsRequired": false}
            ]
        }),
    ];

    // Resolution cap (all video codecs) when the selected preset bounds it.
    if let Some((w, h)) = preset.and_then(resolution_cap) {
        codec_profiles.push(json!({
            "Type": "Video",
            "Conditions": [
                {"Condition": "LessThanEqual", "Property": "Width", "Value": w.to_string(), "IsRequired": true},
                {"Condition": "LessThanEqual", "Property": "Height", "Value": h.to_string(), "IsRequired": true}
            ]
        }));
    }

    let profile = json!({
        "MaxStreamingBitrate": max_bitrate_bps,
        "DirectPlayProfiles": [
            {"Type": "Video", "Container": "mkv,mp4,m4v,mov,webm,ts", "VideoCodec": "h264,hevc,vp8,vp9,av1", "AudioCodec": "aac,mp3,ac3,eac3,flac,opus,dts,pcm,alac"},
            {"Type": "Audio", "Container": "mp3,aac,flac,alac,ogg,opus,wav"}
        ],
        "TranscodingProfiles": [
            {"Type": "Video", "Container": "mp4", "VideoCodec": "h264", "AudioCodec": "aac", "Protocol": "hls", "Context": "Streaming", "MinSegments": 2, "MaxAudioChannels": "6"},
            {"Type": "Video", "Container": "ts", "VideoCodec": "h264", "AudioCodec": "aac,mp3", "Protocol": "hls", "Context": "Streaming", "MinSegments": 2, "MaxAudioChannels": "2"}
        ],
        "CodecProfiles": codec_profiles,
        "SubtitleProfiles": [
            {"Format": "srt", "Method": "External"},
            {"Format": "ass", "Method": "External"},
            {"Format": "ssa", "Method": "External"},
            {"Format": "srt", "Method": "Embed"},
            {"Format": "ass", "Method": "Embed"},
            {"Format": "ssa", "Method": "Embed"},
            {"Format": "pgssub", "Method": "Embed"},
            {"Format": "dvdsub", "Method": "Embed"}
        ]
    });

    (profile, max_bitrate_bps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conditions_for_property<'a>(profile: &'a Value, property: &str) -> Vec<&'a Value> {
        profile["CodecProfiles"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|cp| cp["Conditions"].as_array().unwrap().iter())
            .filter(|c| c["Property"] == property)
            .collect()
    }

    #[test]
    fn original_has_no_resolution_cap_and_no_bitrate() {
        let (profile, max) =
            device_profile(QualitySelection::Manual(QualityPreset::Original), true);
        assert_eq!(max, None);
        assert!(profile["MaxStreamingBitrate"].is_null());
        // No Width/Height conditions for Original.
        assert!(conditions_for_property(&profile, "Width").is_empty());
        assert!(conditions_for_property(&profile, "Height").is_empty());
    }

    #[test]
    fn manual_720p_caps_resolution_and_bitrate() {
        let (profile, max) =
            device_profile(QualitySelection::Manual(QualityPreset::P720Mbps4), true);
        assert_eq!(max, Some(4_000_000));
        assert_eq!(profile["MaxStreamingBitrate"], json!(4_000_000u64));
        let widths = conditions_for_property(&profile, "Width");
        assert_eq!(widths.len(), 1);
        assert_eq!(widths[0]["Value"], "1280");
        let heights = conditions_for_property(&profile, "Height");
        assert_eq!(heights[0]["Value"], "720");
    }

    #[test]
    fn auto_remote_applies_remote_default() {
        let (_, max) = device_profile(QualitySelection::Auto, true);
        // REMOTE_DEFAULT is 1080p / 8 Mbps.
        assert_eq!(max, Some(8_000_000));
    }

    #[test]
    fn auto_local_is_uncapped() {
        let (profile, max) = device_profile(QualitySelection::Auto, false);
        assert_eq!(max, None);
        assert!(conditions_for_property(&profile, "Width").is_empty());
    }

    #[test]
    fn profile_has_all_required_sections() {
        let (profile, _) = device_profile(QualitySelection::Auto, false);
        assert!(profile["DirectPlayProfiles"].is_array());
        assert!(profile["TranscodingProfiles"].is_array());
        assert!(profile["CodecProfiles"].is_array());
        assert!(profile["SubtitleProfiles"].is_array());
        // HEVC 8-bit ceiling is always present (HDR -> SDR transcode).
        let bitdepth = conditions_for_property(&profile, "VideoBitDepth");
        assert_eq!(bitdepth.len(), 1);
        assert_eq!(bitdepth[0]["Value"], "8");
    }
}
