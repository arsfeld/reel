use adw::prelude::*;
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
    items_per_page: i32,
    // Cache preferences
    cache_size_mb: i32,
    auto_clean_cache: bool,
    // Skip intro/credits preferences
    skip_intro_enabled: bool,
    skip_credits_enabled: bool,
    auto_skip_intro: bool,
    auto_skip_credits: bool,
    minimum_marker_duration_seconds: i32,
    // Update preferences
    update_behavior: String,
    check_updates_on_startup: bool,
    auto_download_updates: bool,
    check_prerelease: bool,
}

#[derive(Debug)]
pub enum PreferencesInput {
    SetDefaultPlayer(String),
    SetHardwareAcceleration(bool),
    SetItemsPerPage(i32),
    SetCacheSize(i32),
    SetAutoCleanCache(bool),
    SetSkipIntroEnabled(bool),
    SetSkipCreditsEnabled(bool),
    SetAutoSkipIntro(bool),
    SetAutoSkipCredits(bool),
    SetMinimumMarkerDuration(i32),
    SetUpdateBehavior(String),
    SetCheckUpdatesOnStartup(bool),
    SetAutoDownloadUpdates(bool),
    SetCheckPrerelease(bool),
    CheckForUpdates,
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

                        // Hardware Acceleration - HIDDEN
                        // add = &adw::ActionRow {
                        //     set_title: "Hardware Acceleration",
                        //     set_subtitle: "Enable GPU acceleration for video playback",

                        //     add_suffix = &gtk::Switch {
                        //         set_active: model.hardware_acceleration,
                        //         set_valign: gtk::Align::Center,
                        //         connect_active_notify[sender] => move |switch| {
                        //             sender.input(PreferencesInput::SetHardwareAcceleration(switch.is_active()));
                        //         }
                        //     }
                        // },
                    },

                    // Playback Behavior Settings Group
                    adw::PreferencesGroup {
                        set_title: "Playback Behavior",
                        set_description: Some("Configure skip intro and skip credits behavior"),

                        // Show Skip Intro Button
                        add = &adw::ActionRow {
                            set_title: "Show Skip Intro Button",
                            set_subtitle: "Display skip intro button when intro markers are detected",

                            add_suffix = &gtk::Switch {
                                set_active: model.skip_intro_enabled,
                                set_valign: gtk::Align::Center,
                                connect_active_notify[sender] => move |switch| {
                                    sender.input(PreferencesInput::SetSkipIntroEnabled(switch.is_active()));
                                }
                            }
                        },

                        // Auto-skip Intro
                        add = &adw::ActionRow {
                            set_title: "Auto-skip Intro",
                            set_subtitle: "Automatically skip intros without showing button",

                            add_suffix = &gtk::Switch {
                                set_active: model.auto_skip_intro,
                                set_valign: gtk::Align::Center,
                                connect_active_notify[sender] => move |switch| {
                                    sender.input(PreferencesInput::SetAutoSkipIntro(switch.is_active()));
                                }
                            }
                        },

                        // Show Skip Credits Button
                        add = &adw::ActionRow {
                            set_title: "Show Skip Credits Button",
                            set_subtitle: "Display skip credits button when credits markers are detected",

                            add_suffix = &gtk::Switch {
                                set_active: model.skip_credits_enabled,
                                set_valign: gtk::Align::Center,
                                connect_active_notify[sender] => move |switch| {
                                    sender.input(PreferencesInput::SetSkipCreditsEnabled(switch.is_active()));
                                }
                            }
                        },

                        // Auto-skip Credits
                        add = &adw::ActionRow {
                            set_title: "Auto-skip Credits",
                            set_subtitle: "Automatically skip credits without showing button",

                            add_suffix = &gtk::Switch {
                                set_active: model.auto_skip_credits,
                                set_valign: gtk::Align::Center,
                                connect_active_notify[sender] => move |switch| {
                                    sender.input(PreferencesInput::SetAutoSkipCredits(switch.is_active()));
                                }
                            }
                        },

                        // Minimum Marker Duration
                        add = &adw::ActionRow {
                            set_title: "Minimum Marker Duration",
                            set_subtitle: "Minimum duration (in seconds) for intro/credits markers",

                            add_suffix = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 6,
                                set_valign: gtk::Align::Center,

                                gtk::SpinButton {
                                    set_adjustment: &gtk::Adjustment::new(
                                        model.minimum_marker_duration_seconds as f64,
                                        1.0,
                                        60.0,
                                        1.0,
                                        5.0,
                                        0.0,
                                    ),
                                    set_value: model.minimum_marker_duration_seconds as f64,
                                    connect_value_changed[sender] => move |spin| {
                                        sender.input(PreferencesInput::SetMinimumMarkerDuration(spin.value() as i32));
                                    }
                                },

                                gtk::Label {
                                    set_label: "seconds",
                                    add_css_class: "dim-label",
                                }
                            }
                        },
                    },

                    // Update Settings Group
                    adw::PreferencesGroup {
                        set_title: "Updates",
                        set_description: Some("Configure automatic update behavior"),

                        // Update Behavior
                        add = &adw::ActionRow {
                            set_title: "Update Behavior",
                            set_subtitle: "Choose how updates are handled",

                            add_suffix = &gtk::Box {
                                set_orientation: gtk::Orientation::Horizontal,
                                set_spacing: 6,
                                set_valign: gtk::Align::Center,

                                gtk::DropDown {
                                    set_model: Some(&gtk::StringList::new(&[
                                        "Manual",
                                        "Auto-download",
                                        "Disabled",
                                    ])),
                                    set_selected: match model.update_behavior.as_str() {
                                        "manual" => 0,
                                        "auto" => 1,
                                        "disabled" => 2,
                                        _ => 0,
                                    },
                                    connect_selected_notify[sender] => move |dropdown| {
                                        let selected = dropdown.selected();
                                        let behavior = match selected {
                                            0 => "manual",
                                            1 => "auto",
                                            2 => "disabled",
                                            _ => "manual",
                                        };
                                        sender.input(PreferencesInput::SetUpdateBehavior(behavior.to_string()));
                                    }
                                }
                            }
                        },

                        // Check on Startup
                        add = &adw::ActionRow {
                            set_title: "Check for Updates on Startup",
                            set_subtitle: "Automatically check for new versions when app starts",

                            add_suffix = &gtk::Switch {
                                set_active: model.check_updates_on_startup,
                                set_valign: gtk::Align::Center,
                                connect_active_notify[sender] => move |switch| {
                                    sender.input(PreferencesInput::SetCheckUpdatesOnStartup(switch.is_active()));
                                }
                            }
                        },

                        // Auto-download Updates
                        add = &adw::ActionRow {
                            set_title: "Auto-download Updates",
                            set_subtitle: "Automatically download updates when available",

                            add_suffix = &gtk::Switch {
                                set_active: model.auto_download_updates,
                                set_valign: gtk::Align::Center,
                                connect_active_notify[sender] => move |switch| {
                                    sender.input(PreferencesInput::SetAutoDownloadUpdates(switch.is_active()));
                                }
                            }
                        },

                        // Check Pre-release
                        add = &adw::ActionRow {
                            set_title: "Include Pre-release Versions",
                            set_subtitle: "Check for alpha/beta versions in addition to stable releases",

                            add_suffix = &gtk::Switch {
                                set_active: model.check_prerelease,
                                set_valign: gtk::Align::Center,
                                connect_active_notify[sender] => move |switch| {
                                    sender.input(PreferencesInput::SetCheckPrerelease(switch.is_active()));
                                }
                            }
                        },

                        // Check Now Button
                        add = &adw::ActionRow {
                            set_title: "Check for Updates",
                            set_subtitle: "Manually check for available updates",

                            add_suffix = &gtk::Button {
                                set_label: "Check Now",
                                set_valign: gtk::Align::Center,
                                connect_clicked => PreferencesInput::CheckForUpdates,
                            }
                        },
                    },

                    // Library Settings Group - HIDDEN
                    // adw::PreferencesGroup {
                    //     set_title: "Library",
                    //     set_description: Some("Configure library display settings"),

                    //     // Items Per Page
                    //     add = &adw::ActionRow {
                    //         set_title: "Items Per Page",
                    //         set_subtitle: "Number of items to load at once",

                    //         add_suffix = &gtk::Box {
                    //             set_orientation: gtk::Orientation::Horizontal,
                    //             set_spacing: 6,
                    //             set_valign: gtk::Align::Center,

                    //             gtk::SpinButton {
                    //                 set_adjustment: &gtk::Adjustment::new(
                    //                     model.items_per_page as f64,
                    //                     12.0,
                    //                     100.0,
                    //                     12.0,
                    //                     12.0,
                    //                     0.0,
                    //                 ),
                    //                 set_value: model.items_per_page as f64,
                    //                 connect_value_changed[sender] => move |spin| {
                    //                     sender.input(PreferencesInput::SetItemsPerPage(spin.value() as i32));
                    //                 }
                    //             }
                    //         }
                    //     },
                    // },

                    // Data & Storage Settings Group - HIDDEN
                    // adw::PreferencesGroup {
                    //     set_title: "Data & Storage",
                    //     set_description: Some("Manage cache and offline content"),

                    //     // Cache Size
                    //     add = &adw::ActionRow {
                    //         set_title: "Cache Size Limit",
                    //         set_subtitle: &format!("Currently using {} MB", model.cache_size_mb),

                    //         add_suffix = &gtk::Box {
                    //             set_orientation: gtk::Orientation::Horizontal,
                    //             set_spacing: 6,
                    //             set_valign: gtk::Align::Center,

                    //             gtk::SpinButton {
                    //                 set_adjustment: &gtk::Adjustment::new(
                    //                     model.cache_size_mb as f64,
                    //                     100.0,
                    //                     10000.0,
                    //                     100.0,
                    //                     500.0,
                    //                     0.0,
                    //                 ),
                    //                 set_value: model.cache_size_mb as f64,
                    //                 connect_value_changed[sender] => move |spin| {
                    //                     sender.input(PreferencesInput::SetCacheSize(spin.value() as i32));
                    //                 }
                    //             },

                    //             gtk::Label {
                    //                 set_label: "MB",
                    //                 add_css_class: "dim-label",
                    //             }
                    //         }
                    //     },

                    //     // Auto Clean Cache
                    //     add = &adw::ActionRow {
                    //         set_title: "Auto-clean Cache",
                    //         set_subtitle: "Automatically remove old cached data",

                    //         add_suffix = &gtk::Switch {
                    //             set_active: model.auto_clean_cache,
                    //             set_valign: gtk::Align::Center,
                    //             connect_active_notify[sender] => move |switch| {
                    //                 sender.input(PreferencesInput::SetAutoCleanCache(switch.is_active()));
                    //             }
                    //         }
                    //     },

                    //     // Clear Cache Button
                    //     add = &adw::ActionRow {
                    //         set_title: "Clear Cache",
                    //         set_subtitle: "Remove all cached images and data",

                    //         add_suffix = &gtk::Button {
                    //             set_label: "Clear Now",
                    //             add_css_class: "destructive-action",
                    //             set_valign: gtk::Align::Center,
                    //             connect_clicked[sender] => move |_| {
                    //                 // TODO: Implement cache clearing
                    //                 tracing::info!("Clear cache requested");
                    //             }
                    //         }
                    //     },
                    // },

                    // Actions - Keep the save button for player backend preference
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 12,
                        set_halign: gtk::Align::End,
                        set_margin_top: 24,

                        // Restore Defaults button - HIDDEN
                        // gtk::Button {
                        //     set_label: "Restore Defaults",
                        //     connect_clicked => PreferencesInput::RestoreDefaults,
                        // },

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
            items_per_page: 48,
            cache_size_mb: config.playback.mpv_cache_size_mb as i32,
            auto_clean_cache: true,
            skip_intro_enabled: config.playback.skip_intro_enabled,
            skip_credits_enabled: config.playback.skip_credits_enabled,
            auto_skip_intro: config.playback.auto_skip_intro,
            auto_skip_credits: config.playback.auto_skip_credits,
            minimum_marker_duration_seconds: config.playback.minimum_marker_duration_seconds as i32,
            update_behavior: config.updates.behavior,
            check_updates_on_startup: config.updates.check_on_startup,
            auto_download_updates: config.updates.auto_download,
            check_prerelease: config.updates.check_prerelease,
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
            PreferencesInput::SetSkipIntroEnabled(enabled) => {
                self.skip_intro_enabled = enabled;
                tracing::info!("Skip intro button enabled: {}", enabled);
            }
            PreferencesInput::SetSkipCreditsEnabled(enabled) => {
                self.skip_credits_enabled = enabled;
                tracing::info!("Skip credits button enabled: {}", enabled);
            }
            PreferencesInput::SetAutoSkipIntro(enabled) => {
                self.auto_skip_intro = enabled;
                tracing::info!("Auto-skip intro: {}", enabled);
            }
            PreferencesInput::SetAutoSkipCredits(enabled) => {
                self.auto_skip_credits = enabled;
                tracing::info!("Auto-skip credits: {}", enabled);
            }
            PreferencesInput::SetMinimumMarkerDuration(seconds) => {
                self.minimum_marker_duration_seconds = seconds;
                tracing::info!("Minimum marker duration set to: {} seconds", seconds);
            }
            PreferencesInput::SetUpdateBehavior(behavior) => {
                self.update_behavior = behavior.clone();
                tracing::info!("Update behavior set to: {}", behavior);
            }
            PreferencesInput::SetCheckUpdatesOnStartup(enabled) => {
                self.check_updates_on_startup = enabled;
                tracing::info!("Check updates on startup: {}", enabled);
            }
            PreferencesInput::SetAutoDownloadUpdates(enabled) => {
                self.auto_download_updates = enabled;
                tracing::info!("Auto-download updates: {}", enabled);
            }
            PreferencesInput::SetCheckPrerelease(enabled) => {
                self.check_prerelease = enabled;
                tracing::info!("Check pre-release versions: {}", enabled);
            }
            PreferencesInput::CheckForUpdates => {
                tracing::info!("Checking for updates manually...");
                // TODO: Trigger update check via UpdateService/Worker
                // For now, just log the request
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
                config.playback.skip_intro_enabled = self.skip_intro_enabled;
                config.playback.skip_credits_enabled = self.skip_credits_enabled;
                config.playback.auto_skip_intro = self.auto_skip_intro;
                config.playback.auto_skip_credits = self.auto_skip_credits;
                config.playback.minimum_marker_duration_seconds =
                    self.minimum_marker_duration_seconds as u32;

                // Update config
                config.updates.behavior = self.update_behavior.clone();
                config.updates.check_on_startup = self.check_updates_on_startup;
                config.updates.auto_download = self.auto_download_updates;
                config.updates.check_prerelease = self.check_prerelease;
                // Note: items_per_page would need to be added to config struct
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
                self.items_per_page = 48;
                self.cache_size_mb = 1000;
                self.auto_clean_cache = true;
                self.skip_intro_enabled = true;
                self.skip_credits_enabled = true;
                self.auto_skip_intro = false;
                self.auto_skip_credits = false;
                self.minimum_marker_duration_seconds = 5;
                self.update_behavior = "manual".to_string();
                self.check_updates_on_startup = true;
                self.auto_download_updates = false;
                self.check_prerelease = false;

                tracing::info!("Preferences restored to defaults");
            }
        }
    }
}
