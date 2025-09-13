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

#[derive(Debug)]
pub enum PlayerCommandOutput {
    PlayerInitialized(Option<Arc<RwLock<Player>>>),
    StateChanged(PlayerState),
    PositionUpdate {
        position: Option<Duration>,
        duration: Option<Duration>,
        state: PlayerState,
    },
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

            // Video area - placeholder for now, will add GLArea integration later
            gtk::Box {
                set_vexpand: true,
                set_hexpand: true,
                set_valign: gtk::Align::Fill,
                set_halign: gtk::Align::Fill,
                add_css_class: "video-area",

                gtk::Label {
                    #[watch]
                    set_label: &format!("🎬 Player Backend: {:?}", model.player_state),
                    add_css_class: "title-1",
                    set_valign: gtk::Align::Center,
                    set_halign: gtk::Align::Center,
                },
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

        let model = Self {
            media_item_id,
            player: None,
            player_state: PlayerState::Idle,
            position: Duration::from_secs(0),
            duration: Duration::from_secs(0),
            volume: 1.0,
            db,
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

                // TODO: Get actual media URL from backend
                // For now, use a placeholder
                let media_url = format!("https://example.com/media/{}", id.0);

                if let Some(player) = &self.player {
                    let player_clone = Arc::clone(player);
                    sender.oneshot_command(async move {
                        let player = player_clone.read().unwrap();
                        match player.load_media(&media_url).await {
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
                self.player = player_opt;
                if self.player.is_some() {
                    self.player_state = PlayerState::Idle;
                    info!("Player backend initialized and ready");
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
