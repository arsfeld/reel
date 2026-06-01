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
//! audio, mkv/mp4/mpegts/avi demuxers).
//!
//! The HEVC bit-depth gate is **capability-conditional**. When the client can
//! render 10-bit locally (the pipeline's `glupload ! glcolorconvert` stage), the
//! ceiling is raised to 10-bit so 10-bit **SDR** direct-plays, and an HDR
//! `colorTrc` exclusion keeps HDR (PQ/HLG/Dolby Vision) on the transcode path —
//! HDR tone-mapping is hardware-conditional and handled in a follow-up. When the
//! client cannot render 10-bit, HEVC is capped at 8-bit and everything 10-bit
//! transcodes (the conservative default; 10-bit can't render without the convert
//! stage). The manual quality override (R10) remains the safety net.

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

/// HDR transfer characteristics (PQ + HLG). Excluding these from HEVC keeps
/// HDR10 / HLG / Dolby Vision on the transcode path even when 10-bit direct-play
/// is enabled, so only *SDR* 10-bit direct-plays. (Dolby Vision is ≥ 10-bit and
/// PQ/HLG-based, so it's covered too.)
///
/// NOTE: the exact `colorTrc` value names and `notMatch`/`list` grammar should
/// be confirmed against a live Plex server — this is the documented mechanism
/// but is the one piece of the gate not verified offline. If a server ignores
/// it, the render-failure fallback (U4) still prevents a stuck black screen.
const HDR_COLOR_TRC: &str = "smpte2084,arib-std-b67";

/// Format an `upperBound` limitation directive (Plex transcodes anything above
/// the bound).
fn upper_bound(scope: &str, name: &str, value: &str) -> String {
    format!(
        "add-limitation(scope=videoCodec&scopeName={scope}&type=upperBound&name={name}&value={value})"
    )
}

/// Format a `notMatch` limitation directive against a comma-separated list
/// (Plex transcodes anything whose property matches one of the listed values).
fn not_match_list(scope: &str, name: &str, list: &str) -> String {
    format!(
        "add-limitation(scope=videoCodec&scopeName={scope}&type=notMatch&name={name}&list={list})"
    )
}

/// Build the `X-Plex-Client-Profile-Extra` value: `add-direct-play-profile(...)`
/// entries for each supported container/codec combo, an `add-transcode-target`,
/// and `add-limitation(...)` ceilings, joined by `+`.
///
/// `can_direct_play_10bit` flips the HEVC bit-depth gate: when the client can
/// render 10-bit locally (the pipeline's color-convert stage), the HEVC ceiling
/// is raised to 10-bit so 10-bit **SDR** direct-plays, and an HDR `colorTrc`
/// exclusion keeps HDR transcoding. When false (no convert stage), HEVC is
/// capped at 8-bit and everything 10-bit transcodes — the conservative default.
///
/// The returned string is a query-parameter *value*; the caller percent-encodes
/// it when building the request URL.
pub fn client_profile_extra(can_direct_play_10bit: bool, can_direct_play_hdr: bool) -> String {
    let mut directives: Vec<String> =
        Vec::with_capacity(DIRECT_PLAY_PROFILES.len() + TRANSCODE_TARGETS.len() + 3);

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

    // H.264 level ceiling (covers up to 4K, level 5.2) — always.
    directives.push(upper_bound("h264", "video.level", "52"));

    // HEVC bit-depth gate (the load-bearing change for 10-bit direct-play).
    if can_direct_play_10bit {
        directives.push(upper_bound("hevc", "video.bitDepth", "10"));
        if !can_direct_play_hdr {
            directives.push(not_match_list("hevc", "video.colorTrc", HDR_COLOR_TRC));
        }
    } else {
        directives.push(upper_bound("hevc", "video.bitDepth", "8"));
    }

    directives.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_direct_play_entry_for_known_supported_combo() {
        // matroska + h264 + aac is a combo playbin3 plays natively.
        let extra = client_profile_extra(false, false);
        assert!(extra.contains(
            "add-direct-play-profile(type=videoProfile&container=matroska&videoCodec=h264,hevc&audioCodec=aac,mp3,ac3,eac3,flac,opus,dts,pcm)"
        ));
    }

    #[test]
    fn declares_hls_h264_transcode_target() {
        // The load-bearing fix: without an HLS transcode target the decision
        // endpoint returns 4005 ("No conversion profile found for protocol hls")
        // and fails outright for any file that can't direct-play.
        let extra = client_profile_extra(false, false);
        assert!(extra.contains(
            "add-transcode-target(type=videoProfile&context=streaming&protocol=hls&container=mpegts&videoCodec=h264&audioCodec=aac,mp3,ac3,eac3)"
        ));
    }

    #[test]
    fn encodes_h264_level_limitation() {
        // Covers R2: a level ceiling is expressed in the add-limitation grammar.
        let extra = client_profile_extra(false, false);
        assert!(extra.contains(
            "add-limitation(scope=videoCodec&scopeName=h264&type=upperBound&name=video.level&value=52)"
        ));
    }

    #[test]
    fn incapable_keeps_bitdepth_8_ceiling() {
        // Without a render capability, HEVC is capped at 8-bit so all 10-bit
        // (SDR and HDR) transcodes — parity with the pre-feature behavior.
        let extra = client_profile_extra(false, false);
        assert!(extra.contains(
            "add-limitation(scope=videoCodec&scopeName=hevc&type=upperBound&name=video.bitDepth&value=8)"
        ));
        // Dolby Vision codec ids must never appear as direct-play targets.
        assert!(!extra.contains("dvhe"));
        assert!(!extra.contains("dvh1"));
        assert!(!extra.contains("dolbyvision"));
    }

    #[test]
    fn capable_raises_bitdepth_ceiling_to_10() {
        // With a render capability, 10-bit HEVC is allowed (so 10-bit SDR
        // direct-plays) — the 8-bit ceiling is lifted to 10-bit.
        let extra = client_profile_extra(true, false);
        assert!(extra.contains(
            "add-limitation(scope=videoCodec&scopeName=hevc&type=upperBound&name=video.bitDepth&value=10)"
        ));
        assert!(
            !extra.contains("name=video.bitDepth&value=8)"),
            "the 8-bit ceiling must be lifted when capable"
        );
    }

    #[test]
    fn capable_still_excludes_hdr_via_colortrc() {
        // 10-bit direct-play must not let HDR through: the colorTrc exclusion
        // keeps PQ/HLG (and Dolby Vision) on the transcode path.
        let extra = client_profile_extra(true, false);
        assert!(extra.contains(
            "add-limitation(scope=videoCodec&scopeName=hevc&type=notMatch&name=video.colorTrc&list=smpte2084,arib-std-b67)"
        ));
        assert!(!extra.contains("dvhe"));
        assert!(!extra.contains("dolbyvision"));
    }

    #[test]
    fn directives_are_plus_joined_without_trailing_separator() {
        let extra = client_profile_extra(false, false);
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
        // Check both gate states — the capable path adds a notMatch directive.
        for extra in [
            client_profile_extra(false, false),
            client_profile_extra(true, false),
        ] {
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
    }

    #[test]
    fn survives_percent_encoding_round_trip() {
        // The value is carried as a query parameter; confirm it round-trips
        // through reqwest's URL query encoding without corruption. Use the
        // capable output (the richer string, with a comma list).
        let extra = client_profile_extra(true, false);
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

    #[test]
    fn hdr_capable_omits_colortrc_limitation() {
        let extra = client_profile_extra(true, true);
        assert!(extra.contains(
            "add-limitation(scope=videoCodec&scopeName=hevc&type=upperBound&name=video.bitDepth&value=10)"
        ));
        assert!(
            !extra.contains("name=video.colorTrc"),
            "colorTrc restriction must be omitted when HDR capable"
        );
    }
}
