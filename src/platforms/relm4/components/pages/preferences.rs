use adw::prelude::*;
use gtk::prelude::*;
use libadwaita as adw;
use relm4::gtk;
use relm4::prelude::*;

use crate::db::connection::DatabaseConnection;

#[derive(Debug)]
pub struct PreferencesPage {
    db: DatabaseConnection,
    // Player preferences
    default_player: String,
    hardware_acceleration: bool,
    // Display preferences
    default_view_mode: String,
    items_per_page: i32,
    // Cache preferences
    cache_size_mb: i32,
    auto_clean_cache: bool,
}

#[derive(Debug)]
pub enum PreferencesInput {
    SetDefaultPlayer(String),
    SetHardwareAcceleration(bool),
    SetDefaultViewMode(String),
    SetItemsPerPage(i32),
    SetCacheSize(i32),
    SetAutoCleanCache(bool),
    SavePreferences,
    RestoreDefaults,
}

#[derive(Debug)]
pub enum PreferencesOutput {
    PreferencesSaved,
    Error(String),
}

#[relm4::component(pub async)]
impl AsyncComponent for PreferencesPage {
    type Init = DatabaseConnection;
    type Input = PreferencesInput;
    type Output = PreferencesOutput;
    type CommandOutput = ();

    view! {
        #[root]
        gtk::ScrolledWindow {
            set_hscrollbar_policy: gtk::PolicyType::Never,
            set_vscrollbar_policy: gtk::PolicyType::Automatic,

            adw::Clamp {
                set_maximum_size: 600,
                set_margin_all: 24,

                #[wrap(Some)]
                set_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 24,

                    // Player Settings Group
                    adw::PreferencesGroup {
                        set_title: "Player",
                        set_description: Some("Configure media playback settings"),

                        // Default Player Backend
                        add = &adw::ActionRow {
                            set_title: "Default Player Backend",
                            set_subtitle: "Choose your preferred video player",

                            add_suffix = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 6,
                                set_valign: gtk::Align::Center,

                                gtk::DropDown {
                                    set_model: Some(&gtk::StringList::new(&[
                                        "MPV (Recommended)",
                                        "GStreamer",
                                    ])),
                                    set_selected: if model.default_player == "mpv" { 0 } else { 1 },
                                    connect_selected_notify[sender] => move |dropdown| {
                                        let selected = dropdown.selected();
                                        let player = if selected == 0 { "mpv" } else { "gstreamer" };
                                        sender.input(PreferencesInput::SetDefaultPlayer(player.to_string()));
                                    }
                                }
                            }
                        },

                        // Hardware Acceleration
                        add = &adw::ActionRow {
                            set_title: "Hardware Acceleration",
                            set_subtitle: "Enable GPU acceleration for video playback",

                            add_suffix = &gtk::Switch {
                                set_active: model.hardware_acceleration,
                                set_valign: gtk::Align::Center,
                                connect_active_notify[sender] => move |switch| {
                                    sender.input(PreferencesInput::SetHardwareAcceleration(switch.is_active()));
                                }
                            }
                        },
                    },


                    // Library Settings Group
                    adw::PreferencesGroup {
                        set_title: "Library",
                        set_description: Some("Configure library display settings"),

                        // Default View Mode
                        add = &adw::ActionRow {
                            set_title: "Default View Mode",
                            set_subtitle: "Choose how media items are displayed",

                            add_suffix = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 6,
                                set_valign: gtk::Align::Center,

                                gtk::DropDown {
                                    set_model: Some(&gtk::StringList::new(&[
                                        "Grid View",
                                        "List View",
                                    ])),
                                    set_selected: if model.default_view_mode == "grid" { 0 } else { 1 },
                                    connect_selected_notify[sender] => move |dropdown| {
                                        let selected = dropdown.selected();
                                        let mode = if selected == 0 { "grid" } else { "list" };
                                        sender.input(PreferencesInput::SetDefaultViewMode(mode.to_string()));
                                    }
                                }
                            }
                        },

                        // Items Per Page
                        add = &adw::ActionRow {
                            set_title: "Items Per Page",
                            set_subtitle: "Number of items to load at once",

                            add_suffix = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 6,
                                set_valign: gtk::Align::Center,

                                gtk::SpinButton {
                                    set_adjustment: &gtk::Adjustment::new(
                                        model.items_per_page as f64,
                                        12.0,
                                        100.0,
                                        12.0,
                                        12.0,
                                        0.0,
                                    ),
                                    set_value: model.items_per_page as f64,
                                    connect_value_changed[sender] => move |spin| {
                                        sender.input(PreferencesInput::SetItemsPerPage(spin.value() as i32));
                                    }
                                }
                            }
                        },
                    },

                    // Data & Storage Settings Group
                    adw::PreferencesGroup {
                        set_title: "Data & Storage",
                        set_description: Some("Manage cache and offline content"),

                        // Cache Size
                        add = &adw::ActionRow {
                            set_title: "Cache Size Limit",
                            set_subtitle: &format!("Currently using {} MB", model.cache_size_mb),

                            add_suffix = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 6,
                                set_valign: gtk::Align::Center,

                                gtk::SpinButton {
                                    set_adjustment: &gtk::Adjustment::new(
                                        model.cache_size_mb as f64,
                                        100.0,
                                        10000.0,
                                        100.0,
                                        500.0,
                                        0.0,
                                    ),
                                    set_value: model.cache_size_mb as f64,
                                    connect_value_changed[sender] => move |spin| {
                                        sender.input(PreferencesInput::SetCacheSize(spin.value() as i32));
                                    }
                                },

                                gtk::Label {
                                    set_label: "MB",
                                    add_css_class: "dim-label",
                                }
                            }
                        },

                        // Auto Clean Cache
                        add = &adw::ActionRow {
                            set_title: "Auto-clean Cache",
                            set_subtitle: "Automatically remove old cached data",

                            add_suffix = &gtk::Switch {
                                set_active: model.auto_clean_cache,
                                set_valign: gtk::Align::Center,
                                connect_active_notify[sender] => move |switch| {
                                    sender.input(PreferencesInput::SetAutoCleanCache(switch.is_active()));
                                }
                            }
                        },

                        // Clear Cache Button
                        add = &adw::ActionRow {
                            set_title: "Clear Cache",
                            set_subtitle: "Remove all cached images and data",

                            add_suffix = &gtk::Button {
                                set_label: "Clear Now",
                                add_css_class: "destructive-action",
                                set_valign: gtk::Align::Center,
                                connect_clicked[sender] => move |_| {
                                    // TODO: Implement cache clearing
                                    tracing::info!("Clear cache requested");
                                }
                            }
                        },
                    },

                    // Actions
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 12,
                        set_halign: gtk::Align::End,
                        set_margin_top: 24,

                        gtk::Button {
                            set_label: "Restore Defaults",
                            connect_clicked => PreferencesInput::RestoreDefaults,
                        },

                        gtk::Button {
                            set_label: "Save",
                            add_css_class: "suggested-action",
                            connect_clicked => PreferencesInput::SavePreferences,
                        },
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
        // Load preferences from config file
        let config = match crate::config::Config::load() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to load config, using defaults: {}", e);
                crate::config::Config::default()
            }
        };

        let model = Self {
            db,
            default_player: config.playback.player_backend,
            hardware_acceleration: config.playback.hardware_acceleration,
            default_view_mode: "grid".to_string(), // These would need to be added to config
            items_per_page: 48,
            cache_size_mb: config.playback.mpv_cache_size_mb as i32,
            auto_clean_cache: true,
        };

        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            PreferencesInput::SetDefaultPlayer(player) => {
                self.default_player = player;
                tracing::info!("Default player set to: {}", self.default_player);
            }
            PreferencesInput::SetHardwareAcceleration(enabled) => {
                self.hardware_acceleration = enabled;
                tracing::info!("Hardware acceleration: {}", enabled);
            }
            PreferencesInput::SetDefaultViewMode(mode) => {
                self.default_view_mode = mode;
                tracing::info!("Default view mode set to: {}", self.default_view_mode);
            }
            PreferencesInput::SetItemsPerPage(count) => {
                self.items_per_page = count;
                tracing::info!("Items per page set to: {}", count);
            }
            PreferencesInput::SetCacheSize(size) => {
                self.cache_size_mb = size;
                tracing::info!("Cache size set to: {} MB", size);
            }
            PreferencesInput::SetAutoCleanCache(enabled) => {
                self.auto_clean_cache = enabled;
                tracing::info!("Auto-clean cache: {}", enabled);
            }
            PreferencesInput::SavePreferences => {
                tracing::info!("Saving preferences...");

                // Load current config or create new one
                let mut config = match crate::config::Config::load() {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("Failed to load config: {}", e);
                        crate::config::Config::default()
                    }
                };

                // Update config with current preferences
                config.playback.player_backend = self.default_player.clone();
                config.playback.hardware_acceleration = self.hardware_acceleration;
                // Note: view_mode and items_per_page would need to be added to config struct
                // For now we'll just save what we can

                // Save config to file
                match config.save() {
                    Ok(_) => {
                        tracing::info!("Preferences saved successfully");
                        sender.output(PreferencesOutput::PreferencesSaved).unwrap();
                    }
                    Err(e) => {
                        tracing::error!("Failed to save preferences: {}", e);
                        sender
                            .output(PreferencesOutput::Error(format!("Failed to save: {}", e)))
                            .unwrap();
                    }
                }
            }
            PreferencesInput::RestoreDefaults => {
                // Reset to default values
                self.default_player = "mpv".to_string();
                self.hardware_acceleration = true;
                self.default_view_mode = "grid".to_string();
                self.items_per_page = 48;
                self.cache_size_mb = 1000;
                self.auto_clean_cache = true;

                tracing::info!("Preferences restored to defaults");
            }
        }
    }
}
