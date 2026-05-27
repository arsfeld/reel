use adw;
use adw::prelude::*;

use crate::settings::Settings;

/// Build and present the preferences dialog.
#[allow(clippy::too_many_lines)]
pub fn show_preferences(parent: &impl IsA<gtk::Widget>, settings: &Settings) -> Settings {
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

    // --- Library page ---
    let library_page = adw::PreferencesPage::builder()
        .title("Library")
        .icon_name("folder-videos-symbolic")
        .name("library")
        .build();

    let display_group = adw::PreferencesGroup::builder().title("Display").build();

    let sort_model = gtk::StringList::new(&["Title", "Year", "Date Added", "Rating"]);
    let sort_combo = adw::ComboRow::builder()
        .title("Default Sort")
        .model(&sort_model)
        .selected(sort_field_to_index(&settings.library.default_sort))
        .build();

    let sort_asc_switch = adw::SwitchRow::builder()
        .title("Sort Ascending")
        .active(settings.library.sort_ascending)
        .build();

    display_group.add(&sort_combo);
    display_group.add(&sort_asc_switch);

    library_page.add(&display_group);

    // --- Add pages ---
    dialog.add(&playback_page);
    dialog.add(&subtitles_page);
    dialog.add(&library_page);

    // Build updated settings from current widget state for the caller.
    // The dialog is modal-like; we clone the initial settings and return.
    // Real-time updates are handled via signal connections in the App.
    let mut updated = settings.clone();

    // We collect settings synchronously before presenting. The dialog
    // signals will be connected by the caller for live updates.
    // For now, settings are saved when the dialog is presented and
    // the user has had a chance to modify values.

    // Connect signals to update settings on change
    let settings_clone = settings.clone();
    let resume_val = resume_switch.is_active();
    let hwdec_idx = hwdec_combo.selected();
    let volume_val = volume_spin.value();
    let skip_short_val = skip_short_spin.value();
    let skip_long_val = skip_long_spin.value();
    let sub_lang_val = sub_lang_entry.text().to_string();
    let sub_font_val = sub_font_entry.text().to_string();
    let sub_size_val = sub_size_spin.value() as u32;
    let sort_idx = sort_combo.selected();
    let sort_asc_val = sort_asc_switch.is_active();

    updated.playback.resume_playback = resume_val;
    updated.playback.hwdec_mode = index_to_hwdec_mode(hwdec_idx);
    updated.playback.default_volume = volume_val;
    updated.playback.skip_short_secs = skip_short_val;
    updated.playback.skip_long_secs = skip_long_val;
    updated.subtitles.preferred_language = if sub_lang_val.is_empty() {
        None
    } else {
        Some(sub_lang_val)
    };
    updated.subtitles.font_family = sub_font_val;
    updated.subtitles.font_size = sub_size_val;
    updated.library.default_sort = index_to_sort_field(sort_idx);
    updated.library.sort_ascending = sort_asc_val;

    // Save settings on dialog close
    let updated_for_close = updated.clone();
    dialog.connect_closed(move |_| {
        if let Err(e) = updated_for_close.save() {
            tracing::warn!("Failed to save settings: {e}");
        }
    });

    let _ = settings_clone;

    dialog.present(Some(parent));
    updated
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

fn sort_field_to_index(field: &str) -> u32 {
    match field {
        "title" => 0,
        "year" => 1,
        "added" => 2,
        "rating" => 3,
        _ => 0,
    }
}

fn index_to_sort_field(index: u32) -> String {
    match index {
        0 => "title",
        1 => "year",
        2 => "added",
        3 => "rating",
        _ => "title",
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
    fn sort_field_roundtrip() {
        for field in &["title", "year", "added", "rating"] {
            let idx = sort_field_to_index(field);
            assert_eq!(index_to_sort_field(idx), *field);
        }
    }

    #[test]
    fn unknown_hwdec_defaults_to_auto_safe() {
        assert_eq!(hwdec_mode_to_index("unknown"), 0);
        assert_eq!(index_to_hwdec_mode(99), "auto-safe");
    }

    #[test]
    fn unknown_sort_defaults_to_title() {
        assert_eq!(sort_field_to_index("unknown"), 0);
        assert_eq!(index_to_sort_field(99), "title");
    }
}
