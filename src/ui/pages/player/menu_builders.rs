use gtk::glib;
use gtk::prelude::ToVariant;
use gtk::prelude::*;
use libadwaita as adw;
use relm4::gtk;
use relm4::prelude::*;
use tracing::debug;

use super::{PlayerInput, PlayerPage};

/// Menu population methods for audio/subtitle/zoom/quality menus
impl PlayerPage {
    pub(super) fn populate_audio_menu(&self, sender: AsyncComponentSender<Self>) {
        if let Some(player) = &self.player {
            let player_clone = player.clone();
            let audio_menu_button = self.audio_menu_button.clone();
            let _current_track = self.current_audio_track;
            let sender = sender.clone();
            let popover_count = self.active_popover_count.clone();

            glib::spawn_future_local(async move {
                let tracks = player_clone.get_audio_tracks().await.unwrap_or_default();

                if tracks.is_empty() {
                    // No audio tracks available, disable the button
                    audio_menu_button.set_sensitive(false);
                    audio_menu_button.set_popover(None::<&gtk::Popover>);
                } else {
                    audio_menu_button.set_sensitive(true);

                    // Create menu
                    let menu = gtk::gio::Menu::new();

                    for (track_id, track_name) in &tracks {
                        let item = gtk::gio::MenuItem::new(Some(track_name), None);
                        let action_name = format!("player.audio-track-{}", track_id);
                        item.set_action_and_target_value(Some(&action_name), None);
                        menu.append_item(&item);
                    }

                    // Create popover from menu model
                    let popover = gtk::PopoverMenu::from_model(Some(&menu));

                    // Track popover state to prevent control hiding
                    let popover_count_clone = popover_count.clone();
                    popover.connect_show(move |_| {
                        *popover_count_clone.borrow_mut() += 1;
                        debug!(
                            "Audio popover shown, count: {}",
                            *popover_count_clone.borrow()
                        );
                    });
                    popover.connect_hide(move |_| {
                        let mut count = popover_count.borrow_mut();
                        if *count > 0 {
                            *count -= 1;
                        }
                        debug!("Audio popover hidden, count: {}", *count);
                    });

                    // Add actions for each track
                    let action_group = gtk::gio::SimpleActionGroup::new();
                    for (track_id, _) in &tracks {
                        let action_name = format!("audio-track-{}", track_id);
                        let action = gtk::gio::SimpleAction::new(&action_name, None);
                        let sender_clone = sender.clone();
                        let track_id_copy = *track_id;
                        action.connect_activate(move |_, _| {
                            sender_clone.input(PlayerInput::SetAudioTrack(track_id_copy));
                        });
                        action_group.add_action(&action);
                    }

                    // Insert the action group
                    audio_menu_button.insert_action_group("player", Some(&action_group));
                    audio_menu_button.set_popover(Some(&popover));
                }
            });
        }
    }

    pub(super) fn populate_subtitle_menu(&self, sender: AsyncComponentSender<Self>) {
        if let Some(player) = &self.player {
            let player_clone = player.clone();
            let subtitle_menu_button = self.subtitle_menu_button.clone();
            let _current_track = self.current_subtitle_track;
            let sender = sender.clone();
            let popover_count = self.active_popover_count.clone();

            glib::spawn_future_local(async move {
                let tracks = player_clone.get_subtitle_tracks().await.unwrap_or_default();

                if tracks.is_empty() || tracks.len() == 1 {
                    // No subtitle tracks available (only "None" option), disable the button
                    subtitle_menu_button.set_sensitive(false);
                    subtitle_menu_button.set_popover(None::<&gtk::Popover>);
                } else {
                    subtitle_menu_button.set_sensitive(true);

                    // Create menu
                    let menu = gtk::gio::Menu::new();

                    for (track_id, track_name) in &tracks {
                        let item = gtk::gio::MenuItem::new(Some(track_name), None);
                        let action_name = format!("player.subtitle-track-{}", track_id);
                        item.set_action_and_target_value(Some(&action_name), None);
                        menu.append_item(&item);
                    }

                    // Create popover from menu model
                    let popover = gtk::PopoverMenu::from_model(Some(&menu));

                    // Track popover state to prevent control hiding
                    let popover_count_clone = popover_count.clone();
                    popover.connect_show(move |_| {
                        *popover_count_clone.borrow_mut() += 1;
                        debug!(
                            "Subtitle popover shown, count: {}",
                            *popover_count_clone.borrow()
                        );
                    });
                    popover.connect_hide(move |_| {
                        let mut count = popover_count.borrow_mut();
                        if *count > 0 {
                            *count -= 1;
                        }
                        debug!("Subtitle popover hidden, count: {}", *count);
                    });

                    // Add actions for each track
                    let action_group = gtk::gio::SimpleActionGroup::new();
                    for (track_id, _) in &tracks {
                        let action_name = format!("subtitle-track-{}", track_id);
                        let action = gtk::gio::SimpleAction::new(&action_name, None);
                        let sender_clone = sender.clone();
                        let track_id_copy = *track_id;
                        action.connect_activate(move |_, _| {
                            sender_clone.input(PlayerInput::SetSubtitleTrack(track_id_copy));
                        });
                        action_group.add_action(&action);
                    }

                    // Insert the action group
                    subtitle_menu_button.insert_action_group("player", Some(&action_group));
                    subtitle_menu_button.set_popover(Some(&popover));
                }
            });
        }
    }

    pub(super) fn populate_zoom_menu(&self, sender: AsyncComponentSender<Self>) {
        let zoom_menu_button = self.zoom_menu_button.clone();
        let popover_count = self.active_popover_count.clone();
        let current_mode = self.current_zoom_mode;

        zoom_menu_button.set_sensitive(true);
        zoom_menu_button.set_tooltip_text(Some("Video Zoom"));

        // Create menu
        let menu = gtk::gio::Menu::new();

        // Add zoom modes
        let modes = [
            (crate::player::ZoomMode::Fit, "Fit", "Fit video to window"),
            (
                crate::player::ZoomMode::Fill,
                "Fill",
                "Fill window (may crop)",
            ),
            (
                crate::player::ZoomMode::Zoom16_9,
                "16:9",
                "Force 16:9 aspect ratio",
            ),
            (
                crate::player::ZoomMode::Zoom4_3,
                "4:3",
                "Force 4:3 aspect ratio",
            ),
            (
                crate::player::ZoomMode::Zoom2_35,
                "2.35:1",
                "Cinematic aspect ratio",
            ),
        ];

        for (mode, label, _description) in modes {
            let item = gtk::gio::MenuItem::new(Some(label), None);
            let action_name = format!(
                "player.zoom-{}",
                label.to_lowercase().replace([':', '.'], "-")
            );
            item.set_action_and_target_value(Some(&action_name), None);

            // Add checkmark for current mode
            if mode == current_mode {
                item.set_attribute_value("icon", Some(&"object-select-symbolic".to_variant()));
            }

            menu.append_item(&item);
        }

        // Add custom zoom levels
        menu.append_item(&gtk::gio::MenuItem::new(Some("──────"), None));
        let custom_zooms = [(1.1, "110%"), (1.2, "120%"), (1.3, "130%"), (1.5, "150%")];

        for (level, label) in custom_zooms {
            let item = gtk::gio::MenuItem::new(Some(label), None);
            let action_name = format!("player.zoom-custom-{}", label.replace('%', ""));
            item.set_action_and_target_value(Some(&action_name), None);

            // Check if it matches custom zoom
            if let crate::player::ZoomMode::Custom(current_level) = current_mode
                && (current_level - level).abs() < 0.01
            {
                item.set_attribute_value("icon", Some(&"object-select-symbolic".to_variant()));
            }

            menu.append_item(&item);
        }

        // Create popover
        let popover = gtk::PopoverMenu::from_model(Some(&menu));

        // Track popover state to prevent control hiding
        let popover_count_clone = popover_count.clone();
        popover.connect_show(move |_| {
            *popover_count_clone.borrow_mut() += 1;
            debug!(
                "Zoom popover shown, count: {}",
                *popover_count_clone.borrow()
            );
        });
        popover.connect_hide(move |_| {
            let mut count = popover_count.borrow_mut();
            if *count > 0 {
                *count -= 1;
            }
            debug!("Zoom popover hidden, count: {}", *count);
        });

        // Create action group
        let action_group = gtk::gio::SimpleActionGroup::new();

        // Add actions for preset modes
        for (mode, label, _) in modes {
            let action_name = format!("zoom-{}", label.to_lowercase().replace([':', '.'], "-"));
            let action = gtk::gio::SimpleAction::new(&action_name, None);
            let sender_clone = sender.clone();
            let mode_copy = mode;
            action.connect_activate(move |_, _| {
                sender_clone.input(PlayerInput::SetZoomMode(mode_copy));
            });
            action_group.add_action(&action);
        }

        // Add actions for custom zoom levels
        for (level, label) in custom_zooms {
            let action_name = format!("zoom-custom-{}", label.replace('%', ""));
            let action = gtk::gio::SimpleAction::new(&action_name, None);
            let sender_clone = sender.clone();
            action.connect_activate(move |_, _| {
                sender_clone.input(PlayerInput::SetZoomMode(crate::player::ZoomMode::Custom(
                    level,
                )));
            });
            action_group.add_action(&action);
        }

        // Insert the action group
        zoom_menu_button.insert_action_group("player", Some(&action_group));
        zoom_menu_button.set_popover(Some(&popover));
    }

    pub(super) fn populate_quality_menu(&self, sender: AsyncComponentSender<Self>) {
        let quality_menu_button = self.quality_menu_button.clone();
        let popover_count = self.active_popover_count.clone();
        let current_mode = self.current_upscaling_mode;
        let is_mpv = self.is_mpv_backend;

        if !is_mpv {
            // Disable button for non-MPV backends
            quality_menu_button.set_sensitive(false);
            quality_menu_button.set_tooltip_text(Some("Upscaling only available with MPV player"));
            return;
        }

        quality_menu_button.set_sensitive(true);
        quality_menu_button.set_tooltip_text(Some("Video Quality"));

        // Create menu
        let menu = gtk::gio::Menu::new();

        // Add upscaling modes
        let modes = [
            (crate::player::UpscalingMode::None, "None", "No upscaling"),
            (
                crate::player::UpscalingMode::HighQuality,
                "High Quality",
                "Enhanced quality upscaling",
            ),
            (
                crate::player::UpscalingMode::FSR,
                "FSR",
                "AMD FidelityFX Super Resolution",
            ),
            (
                crate::player::UpscalingMode::Anime,
                "Anime",
                "Optimized for anime content",
            ),
        ];

        for (mode, label, _description) in modes {
            let item = gtk::gio::MenuItem::new(Some(label), None);
            let action_name = format!("player.quality-{}", label.to_lowercase().replace(' ', "-"));
            item.set_action_and_target_value(Some(&action_name), None);

            // Add checkmark for current mode
            if mode == current_mode {
                item.set_attribute_value("icon", Some(&"object-select-symbolic".to_variant()));
            }

            menu.append_item(&item);
        }

        // Create popover from menu model
        let popover = gtk::PopoverMenu::from_model(Some(&menu));

        // Track popover state to prevent control hiding
        let popover_count_clone = popover_count.clone();
        popover.connect_show(move |_| {
            *popover_count_clone.borrow_mut() += 1;
            debug!(
                "Quality popover shown, count: {}",
                *popover_count_clone.borrow()
            );
        });
        popover.connect_hide(move |_| {
            let mut count = popover_count.borrow_mut();
            if *count > 0 {
                *count -= 1;
            }
            debug!("Quality popover hidden, count: {}", *count);
        });

        // Add actions for each mode
        let action_group = gtk::gio::SimpleActionGroup::new();
        for (mode, label, _) in modes {
            let action_name = format!("quality-{}", label.to_lowercase().replace(' ', "-"));
            let action = gtk::gio::SimpleAction::new(&action_name, None);
            let sender_clone = sender.clone();
            let mode_copy = mode;
            action.connect_activate(move |_, _| {
                sender_clone.input(PlayerInput::SetUpscalingMode(mode_copy));
            });
            action_group.add_action(&action);
        }

        // Insert the action group
        quality_menu_button.insert_action_group("player", Some(&action_group));
        quality_menu_button.set_popover(Some(&popover));
    }
}
