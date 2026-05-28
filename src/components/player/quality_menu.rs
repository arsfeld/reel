//! Quality-menu popover for the OSD transport row (U9). A radio group with an
//! `Auto` item on top followed by the `QualityPreset` ladder, plus a decision
//! indicator label ("Direct Play" / "Transcoding 1080p · 8 Mbps", R16).
//!
//! Kept in this sibling module (not `video_player.rs`) to stay under the
//! 2000-line file cap (KTD9). Mirrors `rebuild_track_popovers`: a signature
//! gate avoids rebuilding the radios unless the selection or indicator changed,
//! and buttons are constructed with their `active` state *before* `connect_
//! toggled` is wired, so a rebuild never emits a spurious `SelectQuality`.

use gtk::prelude::*;
use relm4::ComponentSender;
use relm4::gtk;

use super::video_player::{VideoPlayer, VideoPlayerMsg, VideoPlayerWidgets};
use crate::models::playback::{QualityPreset, QualitySelection};

/// Menu label for a selection: `Auto` is distinct from `Manual(Original)` (U9).
pub(super) fn selection_label(selection: QualitySelection) -> &'static str {
    match selection {
        QualitySelection::Auto => "Auto",
        QualitySelection::Manual(preset) => preset.label(),
    }
}

/// A stable signature of the quality-menu state, used to skip rebuilding the
/// radio group when nothing the menu renders has changed.
fn quality_ui_signature(selection: QualitySelection, indicator: &str) -> String {
    format!("{}|{indicator}", selection_label(selection))
}

/// Rebuild the quality radio popover + indicator when the selection or the
/// server decision changed. `signature` is the caller-owned latch.
pub(super) fn rebuild_quality_popover(
    widgets: &VideoPlayerWidgets,
    selection: QualitySelection,
    indicator: &str,
    signature: &mut String,
    sender: &ComponentSender<VideoPlayer>,
) {
    let sig = quality_ui_signature(selection, indicator);
    if sig == *signature {
        return;
    }
    *signature = sig;

    widgets.quality_indicator.set_label(indicator);
    widgets.quality_indicator.set_visible(!indicator.is_empty());

    while let Some(child) = widgets.quality_box.first_child() {
        widgets.quality_box.remove(&child);
    }

    // Auto sits at the top of the radio group; it is the selected item when no
    // manual override is active (the revert-to-auto path is "pick Auto").
    let auto_btn = gtk::CheckButton::builder()
        .label(selection_label(QualitySelection::Auto))
        .active(selection == QualitySelection::Auto)
        .build();
    {
        let sender = sender.clone();
        auto_btn.connect_toggled(move |b| {
            if b.is_active() {
                sender.input(VideoPlayerMsg::SelectQuality(QualitySelection::Auto));
            }
        });
    }
    widgets.quality_box.append(&auto_btn);

    for &preset in QualityPreset::LADDER {
        let active = selection == QualitySelection::Manual(preset);
        let btn = gtk::CheckButton::builder()
            .label(preset.label())
            .active(active)
            .build();
        btn.set_group(Some(&auto_btn));
        let sender = sender.clone();
        btn.connect_toggled(move |b| {
            if b.is_active() {
                sender.input(VideoPlayerMsg::SelectQuality(QualitySelection::Manual(
                    preset,
                )));
            }
        });
        widgets.quality_box.append(&btn);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_label_distinct_from_original() {
        assert_eq!(selection_label(QualitySelection::Auto), "Auto");
        assert_eq!(
            selection_label(QualitySelection::Manual(QualityPreset::Original)),
            "Original"
        );
    }

    #[test]
    fn manual_rung_label_maps_to_preset() {
        assert_eq!(
            selection_label(QualitySelection::Manual(QualityPreset::P720Mbps4)),
            "720p · 4 Mbps"
        );
    }

    #[test]
    fn signature_changes_with_selection_and_indicator() {
        let a = quality_ui_signature(QualitySelection::Auto, "Direct Play");
        let b = quality_ui_signature(
            QualitySelection::Manual(QualityPreset::P720Mbps4),
            "Direct Play",
        );
        let c = quality_ui_signature(QualitySelection::Auto, "Transcoding 720p · 4 Mbps");
        assert_ne!(a, b);
        assert_ne!(a, c);
    }
}
