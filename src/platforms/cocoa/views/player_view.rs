use crate::platforms::cocoa::utils::{CGFloat, CGRect};
use dispatch::Queue;
use objc2::{ClassType, msg_send, msg_send_id, rc::Retained, runtime::NSObject, sel};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSButtonType, NSControlStateValue, NSSlider, NSStackView,
    NSStackViewDistribution, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_av_foundation::{AVPlayer, AVPlayerItem, AVPlayerLayer, AVPlayerTimeControlStatus};
use objc2_core_media::CMTime;
use objc2_foundation::{NSKeyValueObservingOptions, NSString, NSTimeInterval, NSURL};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::core::viewmodels::{PlayerViewModel, Property};
use crate::platforms::cocoa::error::{CocoaError, CocoaResult};
use crate::platforms::cocoa::utils::{AutoLayout, NSEdgeInsets};

const CONTROL_HEIGHT: CGFloat = 60.0;
const BUTTON_WIDTH: CGFloat = 40.0;

#[derive(Debug)]
pub struct PlayerView {
    container: Retained<NSView>,
    player_layer: Option<Retained<AVPlayerLayer>>,
    player: Option<Retained<AVPlayer>>,
    controls_container: Retained<NSStackView>,
    play_pause_button: Retained<NSButton>,
    seek_slider: Retained<NSSlider>,
    time_label: Retained<NSTextField>,
    volume_slider: Retained<NSSlider>,
    fullscreen_button: Retained<NSButton>,
    view_model: Arc<PlayerViewModel>,
    is_playing: Arc<RwLock<bool>>,
    duration: Arc<RwLock<Duration>>,
}

impl PlayerView {
    pub fn new(view_model: Arc<PlayerViewModel>) -> CocoaResult<Self> {
        debug!("Creating PlayerView with AVPlayer");

        // Create main container
        let container = unsafe {
            let view = NSView::new(crate::platforms::cocoa::utils::main_thread_marker());
            view.setWantsLayer(true);
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
            view
        };

        // Create controls container
        let controls_container = unsafe {
            let stack = NSStackView::new(crate::platforms::cocoa::utils::main_thread_marker());
            // TODO: Fix NSUserInterfaceLayoutOrientation constant name
            // stack.setOrientation(NSUserInterfaceLayoutOrientation::NSUserInterfaceLayoutOrientationHorizontal);
            stack.setDistribution(NSStackViewDistribution::Fill);
            stack.setSpacing(10.0);
            stack.setTranslatesAutoresizingMaskIntoConstraints(false);
            // TODO: Fix NSEdgeInsets usage - may need different approach
            // stack.setEdgeInsets(NSEdgeInsets::uniform(10.0));
            stack
        };

        // Create play/pause button
        let play_pause_button = unsafe {
            let button = NSButton::new(crate::platforms::cocoa::utils::main_thread_marker());
            button.setTitle(&NSString::from_str("▶️"));
            button.setBezelStyle(NSBezelStyle::Rounded);
            button.setButtonType(
                NSButtonType::MomentaryPushIn, /* TODO: Fix enum constant name */
            );
            button
        };

        // Create seek slider
        let seek_slider = unsafe {
            let mtm = crate::platforms::cocoa::utils::main_thread_marker();
            let slider = NSSlider::sliderWithValue_minValue_maxValue_target_action(
                0.0, 0.0, 100.0, None, None, mtm,
            );
            slider.setControlSize(objc2_app_kit::NSControlSize::Regular);
            slider.setContinuous(true);
            slider
        };

        // Create time label
        let time_label = unsafe {
            let label = NSTextField::new(crate::platforms::cocoa::utils::main_thread_marker());
            label.setStringValue(&NSString::from_str("0:00 / 0:00"));
            label.setEditable(false);
            label.setBordered(false);
            label.setBackgroundColor(None);
            label.setAlignment(objc2_app_kit::NSTextAlignment::Center);
            label
        };

        // Create volume slider
        let volume_slider = unsafe {
            let mtm = crate::platforms::cocoa::utils::main_thread_marker();
            let slider = NSSlider::sliderWithValue_minValue_maxValue_target_action(
                1.0, 0.0, 1.0, None, None, mtm,
            );
            slider.setControlSize(objc2_app_kit::NSControlSize::Small);
            slider
        };

        // Create fullscreen button
        let fullscreen_button = unsafe {
            let button = NSButton::new(crate::platforms::cocoa::utils::main_thread_marker());
            button.setTitle(&NSString::from_str("⛶"));
            button.setBezelStyle(NSBezelStyle::Rounded);
            button.setButtonType(
                NSButtonType::MomentaryPushIn, /* TODO: Fix enum constant name */
            );
            button
        };

        // Assemble controls
        unsafe {
            controls_container.addArrangedSubview(&play_pause_button);
            controls_container.addArrangedSubview(&seek_slider);
            controls_container.addArrangedSubview(&time_label);
            controls_container.addArrangedSubview(&volume_slider);
            controls_container.addArrangedSubview(&fullscreen_button);

            container.addSubview(&controls_container);
        }

        // Setup constraints
        // TODO: Constraints simplified due to AutoLayout API structure
        let controls_constraints = vec![AutoLayout::height(&controls_container, CONTROL_HEIGHT)];
        AutoLayout::activate(&controls_constraints);

        let play_button_constraints = vec![AutoLayout::width(&play_pause_button, BUTTON_WIDTH)];
        AutoLayout::activate(&play_button_constraints);

        let time_label_constraints = vec![AutoLayout::width(&time_label, 100.0)];
        AutoLayout::activate(&time_label_constraints);

        let volume_slider_constraints = vec![AutoLayout::width(&volume_slider, 80.0)];
        AutoLayout::activate(&volume_slider_constraints);

        let fullscreen_button_constraints =
            vec![AutoLayout::width(&fullscreen_button, BUTTON_WIDTH)];
        AutoLayout::activate(&fullscreen_button_constraints);

        let mut player_view = Self {
            container,
            player_layer: None,
            player: None,
            controls_container,
            play_pause_button,
            seek_slider,
            time_label,
            volume_slider,
            fullscreen_button,
            view_model,
            is_playing: Arc::new(RwLock::new(false)),
            duration: Arc::new(RwLock::new(Duration::ZERO)),
        };

        player_view.setup_bindings()?;
        player_view.setup_actions()?;

        Ok(player_view)
    }

    pub fn view(&self) -> &NSView {
        &self.container
    }

    fn setup_bindings(&mut self) -> CocoaResult<()> {
        debug!("Setting up PlayerView bindings");

        // TODO: UI navigation - implement play_item navigation from UI layer
        // let current_url = self.view_model.current_url();
        let container = self.container.clone();
        let is_playing = self.is_playing.clone();
        let duration = self.duration.clone();

        // TODO: Fix subscription API
        // current_url.subscribe(Box::new(move |url_opt| { ... }));

        // Subscribe to playback state
        let is_playing_prop = self.view_model.is_playing();
        let play_pause_button = self.play_pause_button.clone();
        let player = self.player.clone();

        // is_playing_prop.subscribe(Box::new( // TODO: Fix subscription API
        /*move |playing| {
            let play_pause_button = play_pause_button.clone();
            let player = player.clone();
            let playing = *playing;

            Queue::main().exec_async(move || {
                unsafe {
                    // Update button label
                    if playing {
                        play_pause_button.setTitle(&NSString::from_str("⏸"));
                    } else {
                        play_pause_button.setTitle(&NSString::from_str("▶️"));
                    }

                    // Control playback
                    if let Some(player) = &player {
                        if playing {
                            player.play();
                        } else {
                            player.pause();
                        }
                    }
                }
            });
        }));
        */

        // Subscribe to position updates
        let position = self.view_model.position();
        let seek_slider = self.seek_slider.clone();
        let time_label = self.time_label.clone();
        let duration = self.duration.clone();

        // position.subscribe(Box::new( // TODO: Fix subscription API
        /*move |pos| {
            let seek_slider = seek_slider.clone();
            let time_label = time_label.clone();
            let duration = duration.clone();
            let pos = *pos;

            tokio::spawn(async move {
                let total_duration = *duration.read().await;

                Queue::main().exec_async(move || {
                    unsafe {
                        // Update slider position
                        if total_duration.as_secs() > 0 {
                            let progress = (pos.as_secs_f64() / total_duration.as_secs_f64()) * 100.0;
                            seek_slider.setDoubleValue(progress);
                        }

                        // Update time label
                        let current = Self::format_duration(pos);
                        let total = Self::format_duration(total_duration);
                        let time_str = format!("{} / {}", current, total);
                        time_label.setStringValue(&NSString::from_str(&time_str));
                    }
                });
            });
        }));
        */

        // Subscribe to volume changes
        let volume = self.view_model.volume();
        let volume_slider = self.volume_slider.clone();
        let player = self.player.clone();

        // volume.subscribe(Box::new( // TODO: Fix subscription API
        /*move |vol| {
            let volume_slider = volume_slider.clone();
            let player = player.clone();
            let vol = *vol;

            Queue::main().exec_async(move || {
                unsafe {
                    volume_slider.setFloatValue(vol);

                    if let Some(player) = &player {
                        player.setVolume(vol);
                    }
                }
            });
        }));
        */

        Ok(())
    }

    fn setup_actions(&mut self) -> CocoaResult<()> {
        debug!("Setting up player control actions");

        // Play/pause button action
        let view_model = self.view_model.clone();
        let play_action = block2::StackBlock::new(move || {
            view_model.toggle_playback();
        });

        unsafe {
            let _: () = msg_send![&self.play_pause_button, setTarget: &*play_action];
            let _: () = msg_send![&self.play_pause_button, setAction: sel!(invoke)];
        }

        // Seek slider action
        let view_model = self.view_model.clone();
        let duration = self.duration.clone();
        let seek_action = block2::StackBlock::new(move |sender: &NSSlider| {
            let value = unsafe { sender.doubleValue() };

            let view_model = view_model.clone();
            let duration = duration.clone();

            tokio::spawn(async move {
                let total_duration = *duration.read().await;
                let seek_position =
                    Duration::from_secs_f64((value / 100.0) * total_duration.as_secs_f64());
                view_model.seek(seek_position);
            });
        });

        unsafe {
            let _: () = msg_send![&self.seek_slider, setTarget: &*seek_action];
            let _: () = msg_send![&self.seek_slider, setAction: sel!(invoke:)];
        }

        // Volume slider action
        let view_model = self.view_model.clone();
        let volume_action = block2::StackBlock::new(move |sender: &NSSlider| {
            let value = unsafe { sender.floatValue() };
            view_model.set_volume(value as f64);
        });

        unsafe {
            let _: () = msg_send![&self.volume_slider, setTarget: &*volume_action];
            let _: () = msg_send![&self.volume_slider, setAction: sel!(invoke:)];
        }

        // Fullscreen button action
        let fullscreen_action = block2::StackBlock::new(|| {
            info!("Fullscreen button clicked");
            // Toggle fullscreen - would need window reference
        });

        unsafe {
            let _: () = msg_send![&self.fullscreen_button, setTarget: &*fullscreen_action];
            let _: () = msg_send![&self.fullscreen_button, setAction: sel!(invoke)];
        }

        Ok(())
    }

    fn load_video(
        container: &NSView,
        url: &str,
        is_playing: Arc<RwLock<bool>>,
        duration: Arc<RwLock<Duration>>,
    ) {
        unsafe {
            // Create NSURL
            let ns_url = NSURL::URLWithString(&NSString::from_str(url));
            if ns_url.is_none() {
                error!("Failed to create NSURL from: {}", url);
                return;
            }
            let ns_url = ns_url.unwrap();

            // Create AVPlayerItem
            let mtm = crate::platforms::cocoa::utils::main_thread_marker();
            let player_item = AVPlayerItem::playerItemWithURL(&ns_url, mtm);

            // Create AVPlayer
            let player = AVPlayer::playerWithPlayerItem(Some(&player_item), mtm);

            // Create AVPlayerLayer
            let player_layer = AVPlayerLayer::playerLayerWithPlayer(Some(&player));
            // Use Core Animation to set the frame
            unsafe {
                let _: () = msg_send![&player_layer, setFrame: container.bounds()];
            }
            // Set video gravity using the correct constant
            let gravity = objc2_foundation::NSString::from_str("AVLayerVideoGravityResizeAspect");
            player_layer.setVideoGravity(&gravity);

            // Add player layer to container
            if let Some(layer) = container.layer() {
                layer.insertSublayer_atIndex(&player_layer, 0);
            }

            // Start playback
            player.play();

            // Update state
            tokio::spawn(async move {
                *is_playing.write().await = true;
            });

            // Get duration immediately instead of delaying
            // This avoids thread safety issues with AVPlayerItem
            let asset = unsafe { player_item.asset() };
            let duration_clone = duration.clone();

            // Get duration using msg_send
            let duration_cmtime: CMTime = unsafe { msg_send![&asset, duration] };
            let seconds = duration_cmtime.seconds();
            if seconds.is_finite() && seconds > 0.0 {
                let dur = Duration::from_secs_f64(seconds);
                tokio::spawn(async move {
                    *duration_clone.write().await = dur;
                });
            } else {
                // If duration not ready, try again after a delay
                let duration_clone = duration.clone();
                dispatch::Queue::main().exec_after(Duration::from_millis(500), move || {
                    // Since we're on the main thread already, we can safely access the player
                    // But we need to get it from the app state or another source
                    // For now, just set a default duration
                    let dur = Duration::from_secs(0);
                    tokio::spawn(async move {
                        *duration_clone.write().await = dur;
                    });
                });
            }
        }
    }

    fn format_duration(duration: Duration) -> String {
        let total_seconds = duration.as_secs();
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        if hours > 0 {
            format!("{}:{:02}:{:02}", hours, minutes, seconds)
        } else {
            format!("{}:{:02}", minutes, seconds)
        }
    }

    pub fn load_url(&self, url: String) {
        self.view_model.load_url(url);
    }

    pub fn stop(&self) {
        self.view_model.stop();

        // Clean up player
        if let Some(player) = &self.player {
            unsafe {
                player.pause();
                player.replaceCurrentItemWithPlayerItem(None);
            }
        }

        // Remove player layer
        if let Some(player_layer) = &self.player_layer {
            unsafe {
                player_layer.removeFromSuperlayer();
            }
        }
    }
}
