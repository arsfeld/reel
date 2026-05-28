use std::path::PathBuf;

use adw;
use adw::prelude::*;

use crate::settings::Settings;

/// Bytes per gigabyte used by the download-budget spin row (decimal GB).
const BYTES_PER_GB: f64 = 1_000_000_000.0;

/// Human-readable size string for the usage readout (e.g. "1.5 GB").
fn format_size(bytes: i64) -> String {
    let b = bytes.max(0) as f64;
    if b >= BYTES_PER_GB {
        format!("{:.1} GB", b / BYTES_PER_GB)
    } else if b >= 1_000_000.0 {
        format!("{:.0} MB", b / 1_000_000.0)
    } else if b >= 1_000.0 {
        format!("{:.0} KB", b / 1_000.0)
    } else {
        format!("{b:.0} B")
    }
}

/// Convert a budget spin value (decimal GB, `0` = no budget) to an optional
/// byte count.
fn gb_to_bytes(gb: f64) -> Option<u64> {
    if gb <= 0.0 {
        None
    } else {
        Some((gb * BYTES_PER_GB) as u64)
    }
}

/// Convert an optional byte budget to a decimal-GB spin value (`None` = 0).
fn bytes_to_gb(bytes: Option<u64>) -> f64 {
    bytes.map(|b| b as f64 / BYTES_PER_GB).unwrap_or(0.0)
}

/// Build and present the preferences dialog. `downloads_usage_bytes` is the
/// total size of completed downloads, shown in the Downloads page readout.
#[allow(clippy::too_many_lines)]
pub fn show_preferences(
    parent: &impl IsA<gtk::Widget>,
    settings: &Settings,
    downloads_usage_bytes: i64,
) -> Settings {
    let dialog = adw::PreferencesDialog::builder()
        .title("Preferences")
        .search_enabled(true)
        .build();

    // --- Playback page ---
    let playback_page = adw::PreferencesPage::builder()
        .title("Playback")
        .icon_name("media-playback-start-symbolic")
        .name("playback")
        .build();

    let general_group = adw::PreferencesGroup::builder().title("General").build();

    let resume_switch = adw::SwitchRow::builder()
        .title("Resume Playback")
        .subtitle("Continue from where you left off")
        .active(settings.playback.resume_playback)
        .build();

    let hwdec_model =
        gtk::StringList::new(&["Auto (Safe)", "Auto", "VAAPI", "NVDEC", "Vulkan", "None"]);
    let hwdec_combo = adw::ComboRow::builder()
        .title("Hardware Decoding")
        .subtitle("GPU acceleration method")
        .model(&hwdec_model)
        .selected(hwdec_mode_to_index(&settings.playback.hwdec_mode))
        .build();

    let volume_spin = adw::SpinRow::with_range(0.0, 150.0, 5.0);
    volume_spin.set_title("Default Volume");
    volume_spin.set_value(settings.playback.default_volume);

    general_group.add(&resume_switch);
    general_group.add(&hwdec_combo);
    general_group.add(&volume_spin);

    let controls_group = adw::PreferencesGroup::builder()
        .title("Skip Intervals")
        .build();

    let skip_short_spin = adw::SpinRow::with_range(1.0, 120.0, 1.0);
    skip_short_spin.set_title("Short Skip (seconds)");
    skip_short_spin.set_subtitle("Arrow keys");
    skip_short_spin.set_value(settings.playback.skip_short_secs);

    let skip_long_spin = adw::SpinRow::with_range(1.0, 120.0, 5.0);
    skip_long_spin.set_title("Long Skip (seconds)");
    skip_long_spin.set_subtitle("Shift + Arrow keys");
    skip_long_spin.set_value(settings.playback.skip_long_secs);

    controls_group.add(&skip_short_spin);
    controls_group.add(&skip_long_spin);

    playback_page.add(&general_group);
    playback_page.add(&controls_group);

    // --- Subtitles page ---
    let subtitles_page = adw::PreferencesPage::builder()
        .title("Subtitles")
        .icon_name("media-view-subtitles-symbolic")
        .name("subtitles")
        .build();

    let sub_group = adw::PreferencesGroup::builder().title("Subtitles").build();

    let sub_lang_entry = adw::EntryRow::builder()
        .title("Preferred Language (e.g. en, es)")
        .show_apply_button(true)
        .build();
    if let Some(ref lang) = settings.subtitles.preferred_language {
        sub_lang_entry.set_text(lang);
    }

    let sub_font_entry = adw::EntryRow::builder()
        .title("Font Family")
        .show_apply_button(true)
        .build();
    sub_font_entry.set_text(&settings.subtitles.font_family);

    let sub_size_spin = adw::SpinRow::with_range(16.0, 72.0, 2.0);
    sub_size_spin.set_title("Font Size");
    sub_size_spin.set_value(settings.subtitles.font_size as f64);

    sub_group.add(&sub_lang_entry);
    sub_group.add(&sub_font_entry);
    sub_group.add(&sub_size_spin);

    subtitles_page.add(&sub_group);

    // --- Downloads page ---
    let downloads_page = adw::PreferencesPage::builder()
        .title("Downloads")
        .icon_name("folder-download-symbolic")
        .name("downloads")
        .build();

    let storage_group = adw::PreferencesGroup::builder().title("Storage").build();

    let folder_entry = adw::EntryRow::builder()
        .title("Downloads Folder")
        .show_apply_button(true)
        .build();
    folder_entry.set_text(&settings.downloads.effective_folder().to_string_lossy());

    let budget_spin = adw::SpinRow::with_range(0.0, 4096.0, 1.0);
    budget_spin.set_title("Maximum Size (GB)");
    budget_spin.set_subtitle("0 = no limit; oldest watched downloads are pruned when exceeded");
    budget_spin.set_value(bytes_to_gb(settings.downloads.max_bytes));

    let usage_row = adw::ActionRow::builder()
        .title("Space Used")
        .subtitle(format_size(downloads_usage_bytes))
        .build();

    storage_group.add(&folder_entry);
    storage_group.add(&budget_spin);
    storage_group.add(&usage_row);

    let queue_group = adw::PreferencesGroup::builder().title("Queue").build();
    let concurrency_spin = adw::SpinRow::with_range(1.0, 4.0, 1.0);
    concurrency_spin.set_title("Simultaneous Downloads");
    concurrency_spin.set_value(settings.downloads.effective_concurrency() as f64);
    queue_group.add(&concurrency_spin);

    downloads_page.add(&storage_group);
    downloads_page.add(&queue_group);

    // --- Add pages ---
    dialog.add(&playback_page);
    dialog.add(&subtitles_page);
    dialog.add(&downloads_page);

    // Persist on close, reading the *current* widget values (so user edits are
    // captured, not the initial values).
    let base = settings.clone();
    let default_folder = settings.downloads.effective_folder();
    dialog.connect_closed(move |_| {
        let mut updated = base.clone();
        updated.playback.resume_playback = resume_switch.is_active();
        updated.playback.hwdec_mode = index_to_hwdec_mode(hwdec_combo.selected());
        updated.playback.default_volume = volume_spin.value();
        updated.playback.skip_short_secs = skip_short_spin.value();
        updated.playback.skip_long_secs = skip_long_spin.value();
        let sub_lang = sub_lang_entry.text().to_string();
        updated.subtitles.preferred_language = if sub_lang.is_empty() {
            None
        } else {
            Some(sub_lang)
        };
        updated.subtitles.font_family = sub_font_entry.text().to_string();
        updated.subtitles.font_size = sub_size_spin.value() as u32;

        // Downloads: an empty / default-equal folder stores `None` (resolve to
        // the default at use); a budget of 0 stores `None` (no limit).
        let folder_text = folder_entry.text().to_string();
        updated.downloads.folder = match folder_text.as_str() {
            "" => None,
            s if default_folder.to_string_lossy() == s => None,
            s => Some(PathBuf::from(s)),
        };
        updated.downloads.max_bytes = gb_to_bytes(budget_spin.value());
        updated.downloads.concurrency = concurrency_spin.value() as usize;

        if let Err(e) = updated.save() {
            tracing::warn!("Failed to save settings: {e}");
        }
    });

    dialog.present(Some(parent));
    settings.clone()
}

/// Build and present the About dialog.
pub fn show_about(parent: &impl IsA<gtk::Widget>) {
    let about = adw::AboutDialog::builder()
        .application_name("Reel")
        .application_icon("dev.arsfeld.Reel")
        .version(env!("CARGO_PKG_VERSION"))
        .comments("A modern, native media player for the Linux desktop")
        .website("https://github.com/arosenfeld/reel")
        .issue_url("https://github.com/arosenfeld/reel/issues")
        .license_type(gtk::License::Gpl30)
        .developers(vec!["Alexandre Rosenfeld".to_string()])
        .build();

    about.present(Some(parent));
}

fn hwdec_mode_to_index(mode: &str) -> u32 {
    match mode {
        "auto-safe" => 0,
        "auto" => 1,
        "vaapi" => 2,
        "nvdec" => 3,
        "vulkan" => 4,
        "no" | "none" => 5,
        _ => 0,
    }
}

fn index_to_hwdec_mode(index: u32) -> String {
    match index {
        0 => "auto-safe",
        1 => "auto",
        2 => "vaapi",
        3 => "nvdec",
        4 => "vulkan",
        5 => "no",
        _ => "auto-safe",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hwdec_mode_roundtrip() {
        for mode in &["auto-safe", "auto", "vaapi", "nvdec", "vulkan", "no"] {
            let idx = hwdec_mode_to_index(mode);
            assert_eq!(index_to_hwdec_mode(idx), *mode);
        }
    }

    #[test]
    fn unknown_hwdec_defaults_to_auto_safe() {
        assert_eq!(hwdec_mode_to_index("unknown"), 0);
        assert_eq!(index_to_hwdec_mode(99), "auto-safe");
    }

    #[test]
    fn format_size_scales_units() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2_000), "2 KB");
        assert_eq!(format_size(5_000_000), "5 MB");
        assert_eq!(format_size(1_500_000_000), "1.5 GB");
        // Negative (shouldn't happen) clamps to 0.
        assert_eq!(format_size(-5), "0 B");
    }

    #[test]
    fn budget_gb_conversion_roundtrips_and_treats_zero_as_none() {
        assert_eq!(gb_to_bytes(0.0), None);
        assert_eq!(gb_to_bytes(-1.0), None);
        assert_eq!(gb_to_bytes(2.0), Some(2_000_000_000));
        assert_eq!(bytes_to_gb(None), 0.0);
        assert_eq!(bytes_to_gb(Some(2_000_000_000)), 2.0);
    }
}
