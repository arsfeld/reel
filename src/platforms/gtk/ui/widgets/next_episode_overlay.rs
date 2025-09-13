#![allow(dead_code)]

use gtk4::{glib, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::core::viewmodels::player_view_model::{AutoPlayState, NextEpisodeInfo, PlayerViewModel};

pub struct NextEpisodeOverlay {
    pub container: gtk4::Overlay,
    overlay_box: gtk4::Box,
    mini_player: MiniPlayer,
    countdown_timer: CountdownTimer,
    action_buttons: ActionButtons,
    _binding_handles: Rc<RefCell<Vec<glib::SignalHandlerId>>>,
}

struct MiniPlayer {
    container: gtk4::Box,
    thumbnail: gtk4::Picture,
    title_label: gtk4::Label,
    show_label: gtk4::Label,
    episode_label: gtk4::Label,
    duration_label: gtk4::Label,
    summary_label: gtk4::Label,
    progress_bar: gtk4::ProgressBar,
}

struct CountdownTimer {
    container: gtk4::Box,
    circular_progress: gtk4::DrawingArea,
    time_label: gtk4::Label,
    message_label: gtk4::Label,
}

struct ActionButtons {
    container: gtk4::Box,
    play_now_button: gtk4::Button,
    cancel_button: gtk4::Button,
    auto_play_switch: gtk4::Switch,
    auto_play_label: gtk4::Label,
}

impl NextEpisodeOverlay {
    pub fn new() -> Self {
        let container = gtk4::Overlay::new();
        container.add_css_class("next-episode-overlay");

        // Create main overlay box
        let overlay_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 20);
        overlay_box.set_halign(gtk4::Align::End);
        overlay_box.set_valign(gtk4::Align::End);
        overlay_box.set_margin_end(20);
        overlay_box.set_margin_bottom(100); // Above player controls
        overlay_box.add_css_class("next-episode-box");

        // Create mini player
        let mini_player = Self::create_mini_player();
        overlay_box.append(&mini_player.container);

        // Create countdown timer
        let countdown_timer = Self::create_countdown_timer();
        overlay_box.append(&countdown_timer.container);

        // Create action buttons
        let action_buttons = Self::create_action_buttons();
        overlay_box.append(&action_buttons.container);

        container.add_overlay(&overlay_box);

        // Initially hidden
        overlay_box.set_visible(false);

        Self {
            container,
            overlay_box,
            mini_player,
            countdown_timer,
            action_buttons,
            _binding_handles: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn create_mini_player() -> MiniPlayer {
        let container = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        container.set_size_request(320, -1);
        container.add_css_class("mini-player");

        // Thumbnail with 16:9 aspect ratio
        let thumbnail = gtk4::Picture::new();
        thumbnail.set_size_request(320, 180);
        thumbnail.add_css_class("episode-thumbnail");
        thumbnail.set_content_fit(gtk4::ContentFit::Cover);

        // Episode info
        let info_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        info_box.set_margin_top(8);
        info_box.set_margin_bottom(8);
        info_box.set_margin_start(12);
        info_box.set_margin_end(12);

        let title_label = gtk4::Label::new(None);
        title_label.set_xalign(0.0);
        title_label.add_css_class("episode-title");
        title_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        title_label.set_lines(1);

        let show_label = gtk4::Label::new(None);
        show_label.set_xalign(0.0);
        show_label.add_css_class("episode-show");

        let metadata_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);

        let episode_label = gtk4::Label::new(None);
        episode_label.add_css_class("episode-number");

        let duration_label = gtk4::Label::new(None);
        duration_label.add_css_class("episode-duration");

        metadata_box.append(&episode_label);
        metadata_box.append(&gtk4::Label::new(Some("•")));
        metadata_box.append(&duration_label);

        let summary_label = gtk4::Label::new(None);
        summary_label.set_xalign(0.0);
        summary_label.add_css_class("episode-summary");
        summary_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        summary_label.set_lines(2);
        summary_label.set_wrap(true);

        let progress_bar = gtk4::ProgressBar::new();
        progress_bar.add_css_class("episode-progress");
        progress_bar.set_visible(false); // Only show if episode has progress

        info_box.append(&title_label);
        info_box.append(&show_label);
        info_box.append(&metadata_box);
        info_box.append(&summary_label);
        info_box.append(&progress_bar);

        container.append(&thumbnail);
        container.append(&info_box);

        MiniPlayer {
            container,
            thumbnail,
            title_label,
            show_label,
            episode_label,
            duration_label,
            summary_label,
            progress_bar,
        }
    }

    fn create_countdown_timer() -> CountdownTimer {
        let container = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        container.set_size_request(120, -1);
        container.add_css_class("countdown-timer");
        container.set_valign(gtk4::Align::Center);

        // Circular progress indicator
        let circular_progress = gtk4::DrawingArea::new();
        circular_progress.set_size_request(80, 80);
        circular_progress.add_css_class("circular-progress");

        let time_label = gtk4::Label::new(None);
        time_label.add_css_class("countdown-time");

        let message_label = gtk4::Label::new(Some("Playing next"));
        message_label.add_css_class("countdown-message");

        container.append(&circular_progress);
        container.append(&time_label);
        container.append(&message_label);

        CountdownTimer {
            container,
            circular_progress,
            time_label,
            message_label,
        }
    }

    fn create_action_buttons() -> ActionButtons {
        let container = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        container.add_css_class("action-buttons");
        container.set_valign(gtk4::Align::Center);

        // Primary actions
        let button_box = gtk4::Box::new(gtk4::Orientation::Vertical, 8);

        let play_now_button = gtk4::Button::with_label("Play Now");
        play_now_button.add_css_class("suggested-action");

        let cancel_button = gtk4::Button::with_label("Cancel");
        cancel_button.add_css_class("destructive-action");

        button_box.append(&play_now_button);
        button_box.append(&cancel_button);

        // Auto-play toggle
        let auto_play_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        auto_play_box.set_margin_top(8);

        let auto_play_label = gtk4::Label::new(Some("Auto-play"));
        auto_play_label.add_css_class("dim-label");

        let auto_play_switch = gtk4::Switch::new();

        auto_play_box.append(&auto_play_label);
        auto_play_box.append(&auto_play_switch);

        container.append(&button_box);
        container.append(&auto_play_box);

        ActionButtons {
            container,
            play_now_button,
            cancel_button,
            auto_play_switch,
            auto_play_label,
        }
    }

    pub fn bind_to_view_model(&self, view_model: Arc<PlayerViewModel>) {
        let mut handles = self._binding_handles.borrow_mut();

        // Clear any existing bindings
        handles.clear();

        // Bind overlay visibility to computed property
        let overlay_box = self.overlay_box.clone();
        let vm = view_model.clone();
        glib::spawn_future_local(async move {
            let computed = vm.should_show_next_episode_overlay();
            let mut subscriber = computed.subscribe();

            // Set initial state
            overlay_box.set_visible(computed.get().await);

            // Listen for changes
            while subscriber.wait_for_change().await {
                overlay_box.set_visible(computed.get().await);
            }
        });

        // Bind episode info
        self.bind_episode_info(&view_model);

        // Bind countdown timer
        self.bind_countdown_timer(&view_model);

        // Bind action buttons
        self.bind_action_buttons(&view_model, &mut handles);
    }

    fn bind_episode_info(&self, view_model: &Arc<PlayerViewModel>) {
        let title_label = self.mini_player.title_label.clone();
        let show_label = self.mini_player.show_label.clone();
        let episode_label = self.mini_player.episode_label.clone();
        let duration_label = self.mini_player.duration_label.clone();
        let summary_label = self.mini_player.summary_label.clone();
        let vm = view_model.clone();

        glib::spawn_future_local(async move {
            let info_computed = vm.next_episode_info();
            let mut subscriber = info_computed.subscribe();

            // Helper to update labels
            let update_labels = |info: &NextEpisodeInfo| {
                title_label.set_text(&info.title);
                show_label.set_text(&info.show_title);
                episode_label.set_text(&info.season_episode);
                duration_label.set_text(&info.duration);
                summary_label.set_text(&info.summary);
            };

            // Set initial state
            update_labels(&info_computed.get().await);

            // Listen for changes
            while subscriber.wait_for_change().await {
                update_labels(&info_computed.get().await);
            }
        });

        // Bind thumbnail
        let thumbnail = self.mini_player.thumbnail.clone();
        let vm = view_model.clone();
        glib::spawn_future_local(async move {
            let thumbnail_prop = vm.next_episode_thumbnail();
            let mut subscriber = thumbnail_prop.subscribe();

            // Helper to update thumbnail
            let update_thumbnail = |data: &Option<Vec<u8>>| {
                if let Some(bytes) = data {
                    if let Ok(texture) = gdk4::Texture::from_bytes(&glib::Bytes::from(bytes)) {
                        thumbnail.set_paintable(Some(&texture));
                    }
                }
            };

            // Set initial state
            update_thumbnail(&thumbnail_prop.get().await);

            // Listen for changes
            while subscriber.wait_for_change().await {
                update_thumbnail(&thumbnail_prop.get().await);
            }
        });
    }

    fn bind_countdown_timer(&self, view_model: &Arc<PlayerViewModel>) {
        // Bind time label
        let time_label = self.countdown_timer.time_label.clone();
        let vm = view_model.clone();
        glib::spawn_future_local(async move {
            let state_prop = vm.auto_play_state();
            let mut subscriber = state_prop.subscribe();

            // Helper to update time
            let update_time = |state: &AutoPlayState| match state {
                AutoPlayState::Counting(seconds) => {
                    time_label.set_text(&format!("{}s", seconds));
                }
                _ => time_label.set_text(""),
            };

            // Set initial state
            update_time(&state_prop.get().await);

            // Listen for changes
            while subscriber.wait_for_change().await {
                update_time(&state_prop.get().await);
            }
        });

        // Bind circular progress
        let drawing_area = self.countdown_timer.circular_progress.clone();
        let vm = view_model.clone();

        drawing_area.set_draw_func(move |_, cr, width, height| {
            // We'll get the progress synchronously during draw
            let progress_computed = vm.countdown_progress();
            let progress = progress_computed.get_sync();
            Self::draw_circular_progress(cr, width, height, progress);
        });

        // Trigger redraws when countdown changes
        let drawing_area_clone = self.countdown_timer.circular_progress.clone();
        let vm = view_model.clone();
        glib::spawn_future_local(async move {
            let state_prop = vm.auto_play_state();
            let mut subscriber = state_prop.subscribe();

            // Initial draw
            drawing_area_clone.queue_draw();

            // Redraw on changes
            while subscriber.wait_for_change().await {
                drawing_area_clone.queue_draw();
            }
        });
    }

    fn bind_action_buttons(
        &self,
        view_model: &Arc<PlayerViewModel>,
        handles: &mut Vec<glib::SignalHandlerId>,
    ) {
        // Play now button
        let vm = view_model.clone();
        let handle = self
            .action_buttons
            .play_now_button
            .connect_clicked(move |_| {
                let vm = vm.clone();
                glib::spawn_future_local(async move {
                    vm.play_next_episode_now().await;
                });
            });
        handles.push(handle);

        // Cancel button
        let vm = view_model.clone();
        let handle = self.action_buttons.cancel_button.connect_clicked(move |_| {
            let vm = vm.clone();
            glib::spawn_future_local(async move {
                vm.cancel_auto_play().await;
            });
        });
        handles.push(handle);

        // Auto-play switch - bind state
        let switch = self.action_buttons.auto_play_switch.clone();
        let vm = view_model.clone();
        glib::spawn_future_local(async move {
            let enabled_prop = vm.auto_play_enabled();
            let mut subscriber = enabled_prop.subscribe();

            // Set initial state
            switch.set_active(enabled_prop.get().await);

            // Listen for changes
            while subscriber.wait_for_change().await {
                switch.set_active(enabled_prop.get().await);
            }
        });

        // Auto-play switch - handle toggling
        let vm = view_model.clone();
        let handle = self
            .action_buttons
            .auto_play_switch
            .connect_state_set(move |_, _| {
                let vm = vm.clone();
                glib::spawn_future_local(async move {
                    vm.toggle_auto_play().await;
                });
                glib::Propagation::Proceed
            });
        handles.push(handle);
    }

    fn draw_circular_progress(cr: &gtk4::cairo::Context, width: i32, height: i32, progress: f64) {
        let center_x = width as f64 / 2.0;
        let center_y = height as f64 / 2.0;
        let radius = (width.min(height) as f64 / 2.0) - 4.0;

        // Background circle
        cr.set_source_rgba(0.5, 0.5, 0.5, 0.3);
        let _ = cr.arc(center_x, center_y, radius, 0.0, 2.0 * std::f64::consts::PI);
        cr.set_line_width(4.0);
        let _ = cr.stroke();

        // Progress arc
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        let start_angle = -std::f64::consts::PI / 2.0;
        let end_angle = start_angle + (2.0 * std::f64::consts::PI * progress);
        let _ = cr.arc(center_x, center_y, radius, start_angle, end_angle);
        let _ = cr.stroke();
    }
}
