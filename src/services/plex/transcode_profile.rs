//! Client capability profile for Plex's universal transcoder (KTD8, KTD12).
//!
//! Pure, dependency-free string builder — no GStreamer/GTK imports (services
//! layer is pure Rust). It produces the `X-Plex-Client-Profile-Extra` directive
//! string that tells the server what this client can direct-play, so the
//! decision endpoint routes incompatible media to a transcode.
//!
//! The supported-combo table below is the single source of truth. It was
//! derived from the decoders `playbin3` actually has on the target machine
//! (U1(h) probe: h264 + hevc via libav/VA-API, aac/ac3/eac3/mp3/flac/opus/dts
//! audio, mkv/mp4/mpegts/avi demuxers) and is deliberately **efficiency-first
//! but conservative**: HEVC is allowed only at ≤ 8-bit so 10-bit / HDR / Dolby
//! Vision content (virtually always ≥ 10-bit) routes to a transcode-to-SDR
//! rather than a washed-out direct play (KTD12). The manual quality override
//! (R10) is the safety net when this table mis-guesses.

/// Audio codecs `playbin3` decodes natively, shared across every direct-play
/// container entry. Comma-joined per Plex's `audioCodec=` grammar.
const DIRECT_PLAY_AUDIO: &str = "aac,mp3,ac3,eac3,flac,opus,dts,pcm";

/// A container + the video codecs we accept inside it for direct play. Audio is
/// the shared [`DIRECT_PLAY_AUDIO`] set.
struct DirectPlayProfile {
    /// Plex container name (`mp4`, `matroska`, `mpegts`, `avi`).
    container: &'static str,
    /// Comma-joined video codecs accepted in this container.
    video_codecs: &'static str,
}

/// The direct-play table. HEVC is constrained by the bit-depth limitation below,
/// not omitted, so 8-bit SDR HEVC still direct-plays (efficiency) while 10-bit
/// HDR HEVC transcodes (correctness).
const DIRECT_PLAY_PROFILES: &[DirectPlayProfile] = &[
    DirectPlayProfile {
        container: "mp4",
        video_codecs: "h264,hevc",
    },
    DirectPlayProfile {
        container: "matroska",
        video_codecs: "h264,hevc",
    },
    DirectPlayProfile {
        container: "mpegts",
        video_codecs: "h264,hevc",
    },
    DirectPlayProfile {
        container: "avi",
        video_codecs: "h264",
    },
];

/// The HLS transcode output the client accepts when the server must *convert*
/// incompatible media (as opposed to direct-play it). Without at least one
/// `add-transcode-target`, the universal transcoder has nothing to convert *to*
/// for the requested `protocol=hls` and returns `transcodeDecisionCode 4005`
/// ("No conversion profile found for protocol hls") — failing the whole decision
/// for any file that can't direct-play (e.g. 10-bit HEVC, which our bit-depth
/// ceiling deliberately routes to a transcode). The `Generic` platform profile
/// ships no HLS target, so we declare one explicitly.
struct TranscodeTarget {
    /// Streaming protocol of the output (`hls`).
    protocol: &'static str,
    /// Output container (`mpegts` is the canonical HLS segment container).
    container: &'static str,
    /// Output video codec. h264 is the conservative always-decodable target —
    /// the server tone-maps HDR/10-bit sources down to SDR h264 we can render.
    video_codec: &'static str,
    /// Output audio codecs, mirroring what `playbin3` decodes so multichannel
    /// (ac3/eac3) passes through and stereo falls back to aac/mp3.
    audio_codec: &'static str,
}

/// HLS h264 is the single output target. One target is enough to satisfy the
/// decision endpoint; the manual quality override (R10) covers edge cases.
const TRANSCODE_TARGETS: &[TranscodeTarget] = &[TranscodeTarget {
    protocol: "hls",
    container: "mpegts",
    video_codec: "h264",
    audio_codec: "aac,mp3,ac3,eac3",
}];

/// An upper-bound ceiling Plex must respect or it transcodes. All current
/// limitations are `upperBound`.
struct Limitation {
    /// The video codec the limitation scopes to.
    scope_name: &'static str,
    /// Plex limitation property name (e.g. `video.level`, `video.bitDepth`).
    name: &'static str,
    /// The inclusive upper bound.
    value: &'static str,
}

/// Ceilings. `video.bitDepth ≤ 8` on HEVC is the load-bearing KTD12 guard: HDR10
/// / HLG / Dolby Vision are ≥ 10-bit, so this forces them to a tone-mapped SDR
/// transcode. The H.264 level cap covers up to 4K (level 5.2).
const LIMITATIONS: &[Limitation] = &[
    Limitation {
        scope_name: "h264",
        name: "video.level",
        value: "52",
    },
    Limitation {
        scope_name: "hevc",
        name: "video.bitDepth",
        value: "8",
    },
];

/// Build the `X-Plex-Client-Profile-Extra` value: `add-direct-play-profile(...)`
/// entries for each supported container/codec combo, followed by
/// `add-limitation(...)` ceilings, joined by `+`.
///
/// The returned string is a query-parameter *value*; the caller (U5) percent-
/// encodes it when building the request URL.
pub fn client_profile_extra() -> String {
    let mut directives: Vec<String> = Vec::with_capacity(
        DIRECT_PLAY_PROFILES.len() + TRANSCODE_TARGETS.len() + LIMITATIONS.len(),
    );

    for p in DIRECT_PLAY_PROFILES {
        directives.push(format!(
            "add-direct-play-profile(type=videoProfile&container={}&videoCodec={}&audioCodec={})",
            p.container, p.video_codecs, DIRECT_PLAY_AUDIO
        ));
    }
    for t in TRANSCODE_TARGETS {
        directives.push(format!(
            "add-transcode-target(type=videoProfile&context=streaming&protocol={}&container={}&videoCodec={}&audioCodec={})",
            t.protocol, t.container, t.video_codec, t.audio_codec
        ));
    }
    for l in LIMITATIONS {
        directives.push(format!(
            "add-limitation(scope=videoCodec&scopeName={}&type=upperBound&name={}&value={})",
            l.scope_name, l.name, l.value
        ));
    }

    directives.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_direct_play_entry_for_known_supported_combo() {
        // matroska + h264 + aac is a combo playbin3 plays natively.
        let extra = client_profile_extra();
        assert!(extra.contains(
            "add-direct-play-profile(type=videoProfile&container=matroska&videoCodec=h264,hevc&audioCodec=aac,mp3,ac3,eac3,flac,opus,dts,pcm)"
        ));
    }

    #[test]
    fn declares_hls_h264_transcode_target() {
        // The load-bearing fix: without an HLS transcode target the decision
        // endpoint returns 4005 ("No conversion profile found for protocol hls")
        // and fails outright for any file that can't direct-play.
        let extra = client_profile_extra();
        assert!(extra.contains(
            "add-transcode-target(type=videoProfile&context=streaming&protocol=hls&container=mpegts&videoCodec=h264&audioCodec=aac,mp3,ac3,eac3)"
        ));
    }

    #[test]
    fn encodes_h264_level_limitation() {
        // Covers R2: a level ceiling is expressed in the add-limitation grammar.
        let extra = client_profile_extra();
        assert!(extra.contains(
            "add-limitation(scope=videoCodec&scopeName=h264&type=upperBound&name=video.level&value=52)"
        ));
    }

    #[test]
    fn hdr_dolby_vision_excluded_via_bitdepth_ceiling() {
        // Covers R2 / KTD12: HDR & Dolby Vision are >= 10-bit, so the HEVC
        // bitDepth<=8 ceiling forces them to transcode. No direct-play entry
        // declares a Dolby Vision codec, and no entry lifts HEVC above 8-bit.
        let extra = client_profile_extra();
        assert!(extra.contains(
            "add-limitation(scope=videoCodec&scopeName=hevc&type=upperBound&name=video.bitDepth&value=8)"
        ));
        // Dolby Vision codec ids must never appear as direct-play targets.
        assert!(!extra.contains("dvhe"));
        assert!(!extra.contains("dvh1"));
        assert!(!extra.contains("dolbyvision"));
    }

    #[test]
    fn directives_are_plus_joined_without_trailing_separator() {
        let extra = client_profile_extra();
        assert!(!extra.is_empty());
        assert!(!extra.starts_with('+'));
        assert!(!extra.ends_with('+'));
        assert!(!extra.contains("++"));
        // One directive per profile + one per transcode target + one per limitation.
        let count = extra.matches("add-direct-play-profile").count()
            + extra.matches("add-transcode-target").count()
            + extra.matches("add-limitation").count();
        assert_eq!(count, extra.split('+').count());
    }

    #[test]
    fn every_directive_has_balanced_parentheses() {
        // Guards against a malformed directive that would break server parsing.
        let extra = client_profile_extra();
        let opens = extra.matches('(').count();
        let closes = extra.matches(')').count();
        assert_eq!(opens, closes);
        for directive in extra.split('+') {
            assert!(
                directive.ends_with(')'),
                "directive not closed: {directive}"
            );
            assert!(
                directive.starts_with("add-direct-play-profile(")
                    || directive.starts_with("add-transcode-target(")
                    || directive.starts_with("add-limitation("),
                "unexpected directive: {directive}"
            );
        }
    }

    #[test]
    fn survives_percent_encoding_round_trip() {
        // The value is carried as a query parameter; confirm it round-trips
        // through reqwest's URL query encoding without corruption.
        let extra = client_profile_extra();
        let mut url = reqwest::Url::parse("https://example/decision").unwrap();
        url.query_pairs_mut()
            .append_pair("X-Plex-Client-Profile-Extra", &extra);
        let decoded = url
            .query_pairs()
            .find(|(k, _)| k == "X-Plex-Client-Profile-Extra")
            .map(|(_, v)| v.into_owned())
            .unwrap();
        assert_eq!(decoded, extra);
    }
}
