use adw::prelude::*;
use libadwaita as adw;
use relm4::gtk;
use relm4::prelude::*;

use crate::db::connection::DatabaseConnection;
use crate::services::config_service::CONFIG_SERVICE;

#[tracker::track]
#[derive(Debug)]
pub struct PreferencesDialog {
    #[do_not_track]
    db: DatabaseConnection,
    // Player preferences
    default_player: String,
    hardware_acceleration: bool,
    // Display preferences
    items_per_page: i32,
    // Cache preferences
    cache_size_mb: i32,
    auto_clean_cache: bool,
}

#[derive(Debug)]
pub enum PreferencesDialogInput {
    SetDefaultPlayer(String),
    ReloadConfig,
    Close,
}

#[derive(Debug)]
pub enum PreferencesDialogOutput {
    Closed,
}

#[relm4::component(pub async)]
impl AsyncComponent for PreferencesDialog {
    type Init = DatabaseConnection;
    type Input = PreferencesDialogInput;
    type Output = PreferencesDialogOutput;
    type CommandOutput = ();

    view! {
        #[root]
        adw::PreferencesDialog {
            set_title: "Preferences",
            set_content_width: 500,
            set_content_height: 400,

            // Close button is automatically provided by PreferencesDialog
            connect_closed => PreferencesDialogInput::Close,

            add = &adw::PreferencesPage {
                set_title: "Settings",
                set_icon_name: Some("preferences-system-symbolic"),

                add = &adw::PreferencesGroup {
                    set_title: "Player",
                    set_description: Some("Configure media playback settings"),
                    set_margin_top: 24,
                    set_margin_bottom: 24,
                    set_margin_start: 24,
                    set_margin_end: 24,

                    // Default Player Backend - The only visible setting
                    add = &adw::ActionRow {
                        set_title: "Default Player Backend",
                        set_subtitle: "Choose your preferred video player",

                        add_suffix = &gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 6,
                            set_valign: gtk::Align::Center,

                            gtk::DropDown {
                                set_model: Some(&gtk::StringList::new(
                                    if cfg!(target_os = "macos") {
                                        // On macOS, only show GStreamer
                                        &["GStreamer"]
                                    } else {
                                        // On other platforms, show both options
                                        &["MPV (Recommended)", "GStreamer"]
                                    }
                                )),
                                #[track(model.changed(PreferencesDialog::default_player()))]
                                set_selected: if cfg!(target_os = "macos") {
                                    0 // Always select GStreamer on macOS
                                } else {
                                    if model.default_player == "mpv" { 0 } else { 1 }
                                },
                                connect_selected_notify[sender] => move |dropdown| {
                                    let selected = dropdown.selected();
                                    let player = if cfg!(target_os = "macos") {
                                        "gstreamer" // Always GStreamer on macOS
                                    } else {
                                        if selected == 0 { "mpv" } else { "gstreamer" }
                                    };
                                    sender.input(PreferencesDialogInput::SetDefaultPlayer(player.to_string()));
                                }
                            }
                        }
                    },
                },
            },
        }
    }

    async fn init(
        db: Self::Init,
        _root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        // Load preferences from ConfigService
        let config = CONFIG_SERVICE.get_config().await;

        let model = Self {
            db,
            default_player: config.playback.player_backend,
            hardware_acceleration: config.playback.hardware_acceleration,
            items_per_page: 48,
            cache_size_mb: config.playback.mpv_cache_size_mb as i32,
            auto_clean_cache: true,
            tracker: 0,
        };

        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        self.reset(); // Reset tracker to track changes

        match msg {
            PreferencesDialogInput::SetDefaultPlayer(player) => {
                self.set_default_player(player.clone());
                tracing::info!("Default player set to: {}", self.default_player);

                // Update via ConfigService which will handle saving and broadcasting
                relm4::spawn_local(async move {
                    if let Err(e) = CONFIG_SERVICE.set_player_backend(player).await {
                        tracing::error!("Failed to save preference: {}", e);
                    } else {
                        tracing::info!("Player preference saved successfully");
                    }
                });
            }
            PreferencesDialogInput::ReloadConfig => {
                // Reload config from ConfigService
                let _config = relm4::spawn_local(async move { CONFIG_SERVICE.get_config().await });

                // This will be handled asynchronously - for now just log
                tracing::info!("Reloading config from service");
            }
            PreferencesDialogInput::Close => {
                root.close();
                sender.output(PreferencesDialogOutput::Closed).unwrap();
            }
        }
    }
}
