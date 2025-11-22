use crate::config::Config;
use crate::player::{PlayerController, PlayerHandle, PlayerState};
use crate::services::config_service::CONFIG_SERVICE;
use gtk::glib;
use gtk::prelude::*;
use relm4::gtk;
use relm4::prelude::*;
use tracing::{error, info, warn};

use super::{PlayerInput, PlayerOutput, PlayerPage};

/// Backend management and lifecycle methods
impl PlayerPage {
    #[cfg(all(feature = "mpv", not(target_os = "macos")))]
    pub(super) fn backend_prefers_mpv(selected_backend: &str) -> bool {
        selected_backend.is_empty() || selected_backend.eq_ignore_ascii_case("mpv")
    }

    #[cfg(not(all(feature = "mpv", not(target_os = "macos"))))]
    pub(super) fn backend_prefers_mpv(_selected_backend: &str) -> bool {
        false
    }

    pub(super) fn mpv_upscaling_mode_from_config(config: &Config) -> crate::player::UpscalingMode {
        match config.playback.mpv_upscaling_mode.as_str() {
            "High Quality" | "high_quality" => crate::player::UpscalingMode::HighQuality,
            "FSR" | "fsr" => crate::player::UpscalingMode::FSR,
            "Anime" | "anime" => crate::player::UpscalingMode::Anime,
            "custom" => crate::player::UpscalingMode::Custom,
            _ => crate::player::UpscalingMode::None,
        }
    }

    pub(super) fn attach_player_controller(
        &mut self,
        handle: PlayerHandle,
        controller: PlayerController,
        sender: &AsyncComponentSender<Self>,
    ) {
        info!("Player controller initialized successfully");

        if let Some(mut error_receiver) = handle.take_error_receiver() {
            let sender_clone = sender.clone();
            glib::spawn_future_local(async move {
                while let Some(error_msg) = error_receiver.recv().await {
                    error!("Player error received: {}", error_msg);
                    sender_clone
                        .output(PlayerOutput::ShowToast(error_msg.clone()))
                        .unwrap();
                    sender_clone.input(PlayerInput::ShowError(error_msg));
                }
            });
        }

        glib::spawn_future_local(async move {
            controller.run().await;
        });

        let placeholder = self.video_placeholder.take();
        if placeholder.is_none() {
            while let Some(child) = self.video_container.first_child() {
                self.video_container.remove(&child);
            }
        }

        let video_container = self.video_container.clone();
        let handle_for_widget = handle.clone();
        glib::spawn_future_local(async move {
            if let Ok(video_widget) = handle_for_widget.create_video_widget().await {
                if let Some(placeholder) = placeholder {
                    video_container.remove(&placeholder);
                }

                video_widget.set_vexpand(true);
                video_widget.set_hexpand(true);
                video_widget.set_valign(gtk::Align::Fill);
                video_widget.set_halign(gtk::Align::Fill);

                video_container.append(&video_widget);
                info!("Video widget successfully attached to container");
            }
        });

        if self.is_mpv_backend {
            let saved_mode = self.current_upscaling_mode;
            let handle_for_upscaling = handle.clone();
            glib::spawn_future_local(async move {
                if let Err(err) = handle_for_upscaling.set_upscaling_mode(saved_mode).await {
                    warn!("Failed to apply MPV upscaling mode: {}", err);
                }
            });
        }

        self.player = Some(handle);
    }

    pub(super) async fn rebuild_player_backend(
        &mut self,
        config: &Config,
        sender: &AsyncComponentSender<Self>,
        reason: &str,
    ) {
        let backend_label = &config.playback.player_backend;
        info!(
            "Rebuilding player backend due to {} (requested={})",
            reason, backend_label
        );

        self.is_mpv_backend = Self::backend_prefers_mpv(backend_label);
        self.current_upscaling_mode = Self::mpv_upscaling_mode_from_config(config);
        self.error_message = None;

        if let Some(existing_player) = self.player.take() {
            let handle = existing_player.clone();
            glib::spawn_future_local(async move {
                if let Err(err) = handle.stop().await {
                    warn!(
                        "Failed to stop previous player during backend switch: {}",
                        err
                    );
                }
            });
        }

        let active_media = self.media_item_id.clone();
        let active_context = self.playlist_context.clone();

        match PlayerController::new(config) {
            Ok((handle, controller)) => {
                self.attach_player_controller(handle, controller, sender);

                let backend_display = if self.is_mpv_backend {
                    "MPV"
                } else {
                    "GStreamer"
                };

                sender
                    .output(PlayerOutput::ShowToast(format!(
                        "Switched playback backend to {}",
                        backend_display
                    )))
                    .ok();

                if let Some(media_id) = active_media {
                    if let Some(context) = active_context {
                        sender.input(PlayerInput::LoadMediaWithContext { media_id, context });
                    } else {
                        sender.input(PlayerInput::LoadMedia(media_id));
                    }
                }
            }
            Err(e) => {
                error!("Failed to rebuild player backend: {}", e);
                self.error_message = Some(format!("Failed to initialize player: {}", e));
                self.player_state = PlayerState::Error;
            }
        }
    }

    pub(super) async fn handle_config_update(
        &mut self,
        config: &Config,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.config_auto_resume = config.playback.auto_resume;
        self.config_resume_threshold_seconds = config.playback.resume_threshold_seconds as u64;
        self.config_progress_update_interval_seconds =
            config.playback.progress_update_interval_seconds as u64;

        // Update skip marker manager config
        self.skip_marker_manager.update_config(
            config.playback.skip_intro_enabled,
            config.playback.skip_credits_enabled,
            config.playback.auto_skip_intro,
            config.playback.auto_skip_credits,
            config.playback.minimum_marker_duration_seconds as u64,
        );

        let prefer_mpv = Self::backend_prefers_mpv(&config.playback.player_backend);
        if prefer_mpv != self.is_mpv_backend {
            self.rebuild_player_backend(config, sender, "config update")
                .await;
            return;
        }

        if self.is_mpv_backend {
            let new_mode = Self::mpv_upscaling_mode_from_config(config);
            if new_mode != self.current_upscaling_mode {
                self.current_upscaling_mode = new_mode;
                if let Some(ref player) = self.player {
                    let player_handle = player.clone();
                    glib::spawn_future_local(async move {
                        if let Err(err) = player_handle.set_upscaling_mode(new_mode).await {
                            warn!("Failed to update MPV upscaling mode: {}", err);
                        }
                    });
                }
            }
        }
    }

    pub(super) async fn ensure_backend_alignment(
        &mut self,
        backend: &str,
        sender: &AsyncComponentSender<Self>,
    ) {
        let prefer_mpv = Self::backend_prefers_mpv(backend);
        if prefer_mpv != self.is_mpv_backend {
            let config = CONFIG_SERVICE.get_config().await;
            self.rebuild_player_backend(&config, sender, "backend change event")
                .await;
        } else {
            info!(
                "Player backend already using requested engine: {}",
                if prefer_mpv { "mpv" } else { "gstreamer" }
            );
        }
    }
}
