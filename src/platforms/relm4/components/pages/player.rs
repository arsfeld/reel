use crate::config::Config;
use crate::models::MediaItemId;
use crate::player::factory::{Player, PlayerState};
use adw::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use libadwaita as adw;
use relm4::gtk;
use relm4::prelude::*;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::{debug, error, info};

pub struct PlayerPage {
    media_item_id: Option<MediaItemId>,
    player: Option<Arc<RwLock<Player>>>,
    player_state: PlayerState,
    position: Duration,
    duration: Duration,
    volume: f64,
    db: Arc<crate::db::connection::DatabaseConnection>,
    video_container: gtk::Box,
}

impl std::fmt::Debug for PlayerPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayerPage")
            .field("media_item_id", &self.media_item_id)
            .field("player_state", &self.player_state)
            .field("position", &self.position)
            .field("duration", &self.duration)
            .field("volume", &self.volume)
            .finish()
    }
}

#[derive(Debug)]
pub enum PlayerInput {
    LoadMedia(MediaItemId),
    PlayPause,
    Stop,
    Seek(Duration),
    SetVolume(f64),
    UpdatePosition,
}

#[derive(Debug, Clone)]
pub enum PlayerOutput {
    NavigateBack,
    MediaLoaded,
    Error(String),
}

pub enum PlayerCommandOutput {
    PlayerInitialized(Option<Arc<RwLock<Player>>>),
    StateChanged(PlayerState),
    PositionUpdate {
        position: Option<Duration>,
        duration: Option<Duration>,
        state: PlayerState,
    },
}

impl std::fmt::Debug for PlayerCommandOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlayerInitialized(player) => {
                write!(f, "PlayerInitialized({})", player.is_some())
            }
            Self::StateChanged(state) => write!(f, "StateChanged({:?})", state),
            Self::PositionUpdate {
                position,
                duration,
                state,
            } => {
                write!(
                    f,
                    "PositionUpdate {{ position: {:?}, duration: {:?}, state: {:?} }}",
                    position, duration, state
                )
            }
        }
    }
}

#[relm4::component(pub async)]
impl AsyncComponent for PlayerPage {
    type Init = (
        Option<MediaItemId>,
        Arc<crate::db::connection::DatabaseConnection>,
    );
    type Input = PlayerInput;
    type Output = PlayerOutput;
    type CommandOutput = PlayerCommandOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_vexpand: true,
            set_hexpand: true,
            add_css_class: "player-container",

            // Header
            gtk::HeaderBar {
                set_title_widget = Some(&gtk::Label::new(Some("Player"))) {},


                pack_start = &gtk::Button {
                    set_icon_name: "go-previous-symbolic",
                    connect_clicked[sender] => move |_| {
                        sender.output(PlayerOutput::NavigateBack).unwrap();
                    },
                },
            },

            // Video area with GLArea from player backend
            model.video_container.clone() {
                set_vexpand: true,
                set_hexpand: true,
                set_valign: gtk::Align::Fill,
                set_halign: gtk::Align::Fill,
                add_css_class: "video-area",
            },

            // Simple controls
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_halign: gtk::Align::Center,
                set_spacing: 12,
                set_margin_all: 24,

                gtk::Button {
                    #[watch]
                    set_icon_name: if matches!(model.player_state, PlayerState::Playing) {
                        "media-playback-pause-symbolic"
                    } else {
                        "media-playback-start-symbolic"
                    },
                    add_css_class: "circular",
                    add_css_class: "suggested-action",
                    set_size_request: (48, 48),
                    connect_clicked => PlayerInput::PlayPause,
                },

                gtk::Button {
                    set_icon_name: "media-playback-stop-symbolic",
                    add_css_class: "circular",
                    set_size_request: (48, 48),
                    connect_clicked => PlayerInput::Stop,
                },
            },

            // Status bar
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_halign: gtk::Align::Center,
                set_margin_bottom: 12,

                gtk::Label {
                    #[watch]
                    set_label: &format!("Status: {:?}", model.player_state),
                    add_css_class: "dim-label",
                },
            },
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let (media_item_id, db) = init;

        // Create a container for the video widget that will be populated when player initializes
        let video_container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();

        // Add a placeholder initially
        let placeholder = gtk::Label::new(Some("Initializing player..."));
        placeholder.add_css_class("title-1");
        placeholder.set_valign(gtk::Align::Center);
        placeholder.set_halign(gtk::Align::Center);
        video_container.append(&placeholder);

        let model = Self {
            media_item_id,
            player: None,
            player_state: PlayerState::Idle,
            position: Duration::from_secs(0),
            duration: Duration::from_secs(0),
            volume: 1.0,
            db,
            video_container: video_container.clone(),
        };

        // Initialize the player
        sender.oneshot_command(async move {
            // Create a default config - in real app this would come from settings
            let config = Config::default();
            match Player::new(&config) {
                Ok(player) => {
                    info!("Player initialized successfully");
                    PlayerCommandOutput::PlayerInitialized(Some(Arc::new(RwLock::new(player))))
                }
                Err(e) => {
                    error!("Failed to initialize player: {}", e);
                    PlayerCommandOutput::PlayerInitialized(None)
                }
            }
        });

        // Load media if provided
        if let Some(id) = &model.media_item_id {
            sender.input(PlayerInput::LoadMedia(id.clone()));
        }

        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        match msg {
            PlayerInput::LoadMedia(id) => {
                self.media_item_id = Some(id.clone());
                self.player_state = PlayerState::Loading;

                // Get actual media URL from backend using GetStreamUrlCommand
                let db_clone = self.db.clone();
                let media_id = id.clone();

                if let Some(player) = &self.player {
                    let player_clone = Arc::clone(player);
                    sender.oneshot_command(async move {
                        use crate::services::commands::Command;
                        use crate::services::commands::media_commands::GetStreamUrlCommand;

                        // Get the stream info from the backend using stateless command
                        let stream_info = match (GetStreamUrlCommand {
                            db: db_clone.as_ref().clone(),
                            media_item_id: media_id,
                        })
                        .execute()
                        .await
                        {
                            Ok(info) => info,
                            Err(e) => {
                                error!("Failed to get stream URL: {}", e);
                                return PlayerCommandOutput::StateChanged(PlayerState::Error);
                            }
                        };

                        info!("Got stream URL: {}", stream_info.url);

                        // Load the media into the player
                        let player = player_clone.read().unwrap();
                        match player.load_media(&stream_info.url).await {
                            Ok(_) => {
                                info!("Media loaded successfully");
                                PlayerCommandOutput::StateChanged(PlayerState::Idle)
                            }
                            Err(e) => {
                                error!("Failed to load media: {}", e);
                                PlayerCommandOutput::StateChanged(PlayerState::Error)
                            }
                        }
                    });
                }
            }
            PlayerInput::PlayPause => {
                if let Some(player) = &self.player {
                    let player_clone = Arc::clone(player);
                    let current_state = self.player_state.clone();

                    sender.oneshot_command(async move {
                        let player = player_clone.read().unwrap();
                        match current_state {
                            PlayerState::Playing => {
                                player.pause().await.ok();
                                PlayerCommandOutput::StateChanged(PlayerState::Paused)
                            }
                            _ => {
                                player.play().await.ok();
                                PlayerCommandOutput::StateChanged(PlayerState::Playing)
                            }
                        }
                    });
                }
            }
            PlayerInput::Stop => {
                if let Some(player) = &self.player {
                    let player_clone = Arc::clone(player);
                    sender.oneshot_command(async move {
                        let player = player_clone.read().unwrap();
                        player.stop().await.ok();
                        PlayerCommandOutput::StateChanged(PlayerState::Stopped)
                    });
                }
            }
            PlayerInput::Seek(position) => {
                if let Some(player) = &self.player {
                    let player_clone = Arc::clone(player);
                    sender.oneshot_command(async move {
                        let player = player_clone.read().unwrap();
                        player.seek(position).await.ok();
                        // Return current state after seek
                        let state = player.get_state().await;
                        PlayerCommandOutput::StateChanged(state)
                    });
                }
            }
            PlayerInput::SetVolume(volume) => {
                self.volume = volume;
                if let Some(player) = &self.player {
                    let player_clone = Arc::clone(player);
                    sender.oneshot_command(async move {
                        let player = player_clone.read().unwrap();
                        player.set_volume(volume).await.ok();
                        // Return current state after volume change
                        let state = player.get_state().await;
                        PlayerCommandOutput::StateChanged(state)
                    });
                }
            }
            PlayerInput::UpdatePosition => {
                if let Some(player) = &self.player {
                    let player_clone = Arc::clone(player);
                    sender.oneshot_command(async move {
                        let player = player_clone.read().unwrap();
                        let position = player.get_position().await;
                        let duration = player.get_duration().await;
                        let state = player.get_state().await;
                        PlayerCommandOutput::PositionUpdate {
                            position,
                            duration,
                            state,
                        }
                    });
                }
            }
        }
    }

    async fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            PlayerCommandOutput::PlayerInitialized(player_opt) => {
                self.player = player_opt.clone();
                if let Some(ref player) = player_opt {
                    self.player_state = PlayerState::Idle;
                    info!("Player backend initialized and ready");

                    // Clear the container and add the actual video widget
                    while let Some(child) = self.video_container.first_child() {
                        self.video_container.remove(&child);
                    }

                    // Get the video widget from the player backend (GLArea for MPV)
                    let video_widget = {
                        let player_guard = player.read().unwrap();
                        player_guard.create_video_widget()
                    };

                    self.video_container.append(&video_widget);
                    info!("Video widget (GLArea) added to player");
                } else {
                    self.player_state = PlayerState::Error;
                    error!("Failed to initialize player backend");
                }
            }
            PlayerCommandOutput::StateChanged(state) => {
                self.player_state = state;
            }
            PlayerCommandOutput::PositionUpdate {
                position,
                duration,
                state,
            } => {
                if let Some(pos) = position {
                    self.position = pos;
                }
                if let Some(dur) = duration {
                    self.duration = dur;
                }
                self.player_state = state;
            }
        }
    }
}
