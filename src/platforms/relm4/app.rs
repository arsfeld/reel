use super::components::shared::{AppCommand, AppInput, AppOutput, CommandResult, NavigationTarget};
// MessageBrokers are now static and accessed directly from broker modules
use crate::db::{Database, DatabaseConnection};
use libadwaita as adw;
use libadwaita::prelude::*;
use relm4::gtk;
use relm4::prelude::*;
use std::cell::RefCell;
use std::sync::Arc;
use tokio::runtime::Runtime;

thread_local! {
    static RUNTIME: RefCell<Option<Arc<Runtime>>> = RefCell::new(None);
}

pub struct ReelApp {
    runtime: Arc<Runtime>,
}

impl ReelApp {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self { runtime }
    }

    pub fn run(&self) -> anyhow::Result<()> {
        // Store runtime in a thread-local for AppModel to access
        RUNTIME.with(|r| {
            *r.borrow_mut() = Some(self.runtime.clone());
        });

        let app = adw::Application::builder()
            .application_id("one.reel.Reel")
            .build();

        // Initialize AdwStyleManager for proper theme support
        let style_manager = adw::StyleManager::default();
        style_manager.set_color_scheme(adw::ColorScheme::PreferDark);

        let app = RelmApp::from_app(app);
        relm4::set_global_css(
            "
            /* Navigation and Layout */
            .navigation-sidebar {
                background: transparent;
            }

            /* Adwaita Typography Classes */
            .title-1 {
                font-size: 28pt;
                font-weight: 800;
                line-height: 1.1;
            }
            .title-2 {
                font-size: 22pt;
                font-weight: 700;
                line-height: 1.2;
            }
            .title-3 {
                font-size: 18pt;
                font-weight: 600;
                line-height: 1.2;
            }
            .heading {
                font-size: 15pt;
                font-weight: 600;
                line-height: 1.3;
            }
            .body {
                font-size: 11pt;
                line-height: 1.4;
            }
            .caption {
                font-size: 9pt;
                line-height: 1.3;
            }
            .dim-label {
                opacity: 0.55;
            }

            /* Media Cards */
            .media-card {
                background: var(--card-bg-color);
                border-radius: 8px;
                box-shadow: 0 1px 3px rgba(0,0,0,0.12);
                padding: 12px;
                margin: 6px;
                transition: all 0.2s ease;
            }
            .media-card:hover {
                transform: translateY(-2px);
                box-shadow: 0 4px 12px rgba(0,0,0,0.15);
            }

            /* Progress indicators */
            .progress-bar {
                background: var(--accent-color);
                border-radius: 2px;
                min-height: 4px;
            }

            /* Player styles */
            .video-area {
                background-color: black;
            }
            .fullscreen .video-area {
                background-color: black;
            }

            /* OSD controls with proper dark theme support */
            .overlay-controls {
                background: linear-gradient(to top, rgba(0, 0, 0, 0.8), transparent);
                border-radius: 12px;
                padding: 24px;
                transition: opacity 0.3s ease;
            }
            .overlay-controls button {
                background: rgba(255, 255, 255, 0.1);
                border: 1px solid rgba(255, 255, 255, 0.2);
                color: white;
                border-radius: 50%;
                min-width: 48px;
                min-height: 48px;
                margin: 0 6px;
            }
            .overlay-controls button:hover {
                background: rgba(255, 255, 255, 0.2);
            }
            .seek-bar {
                min-height: 8px;
                border-radius: 4px;
            }

            /* Button styling */
            .suggested-action {
                background: var(--accent-bg-color);
                color: var(--accent-fg-color);
            }
            .destructive-action {
                background: var(--destructive-bg-color);
                color: var(--destructive-fg-color);
            }
            .flat {
                background: transparent;
                box-shadow: none;
                border: none;
            }
            .circular {
                border-radius: 50%;
            }
            .pill {
                border-radius: 999px;
                padding: 6px 12px;
            }

            /* Responsive design helpers */
            @media (max-width: 640px) {
                .media-grid {
                    grid-template-columns: repeat(2, 1fr);
                }
                .title-1 {
                    font-size: 22pt;
                }
            }
            @media (min-width: 641px) and (max-width: 1024px) {
                .media-grid {
                    grid-template-columns: repeat(4, 1fr);
                }
            }
            @media (min-width: 1025px) {
                .media-grid {
                    grid-template-columns: repeat(6, 1fr);
                }
            }
            ",
        );
        app.run_async::<AppModel>(());

        Ok(())
    }
}

pub struct AppModel {
    db: DatabaseConnection,
    runtime: Arc<Runtime>,
    loading: bool,
    current_page: NavigationTarget,
    style_manager: adw::StyleManager,
}

#[relm4::component(pub async)]
impl AsyncComponent for AppModel {
    type Input = AppInput;
    type Output = AppOutput;
    type Init = ();
    type CommandOutput = CommandResult;

    view! {
        #[root]
        adw::ApplicationWindow {
            set_title: Some("Reel"),
            set_default_width: 1280,
            set_default_height: 720,

            #[wrap(Some)]
            set_content = &gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                adw::HeaderBar {
                    #[wrap(Some)]
                    set_title_widget = &adw::WindowTitle {
                        set_title: "Reel",
                        set_subtitle: "Media Player",
                    },

                    pack_end = &gtk::Button {
                        set_icon_name: "preferences-system-symbolic",
                        set_tooltip_text: Some("Preferences"),
                        connect_clicked => AppInput::Navigate(NavigationTarget::Preferences),
                    }
                },

                if model.loading {
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 24,
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,
                        set_vexpand: true,
                        set_hexpand: true,

                        gtk::Image {
                            set_icon_name: Some("content-loading-symbolic"),
                            set_pixel_size: 64,
                        },

                        gtk::Label {
                            set_label: "Loading Reel",
                            add_css_class: "title-2",
                        },

                        gtk::Label {
                            set_label: "Initializing your media libraries...",
                            add_css_class: "dim-label",
                        },

                        gtk::Spinner {
                            set_spinning: true,
                            set_width_request: 48,
                            set_height_request: 48,
                        }
                    }
                } else {
                    adw::NavigationSplitView {
                        set_collapsed: false,
                        set_show_content: true,

                        #[wrap(Some)]
                        set_sidebar = &adw::NavigationPage {
                            set_title: "Navigation",

                            #[wrap(Some)]
                            set_child = &gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                                set_width_request: 280,

                                gtk::SearchEntry {
                                    set_placeholder_text: Some("Search media..."),
                                    set_margin_all: 12,
                                    add_css_class: "flat",
                                },

                                gtk::ScrolledWindow {
                                    set_vexpand: true,
                                    set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),

                                    gtk::ListBox {
                                        set_selection_mode: gtk::SelectionMode::Single,
                                        add_css_class: "navigation-sidebar",

                                        adw::ActionRow {
                                            set_title: "Home",
                                            set_icon_name: Some("user-home-symbolic"),
                                            set_activatable: true,
                                        },

                                        adw::ActionRow {
                                            set_title: "Movies",
                                            set_icon_name: Some("video-x-generic-symbolic"),
                                            set_activatable: true,
                                        },

                                        adw::ActionRow {
                                            set_title: "TV Shows",
                                            set_icon_name: Some("media-playlist-symbolic"),
                                            set_activatable: true,
                                        },

                                        adw::ActionRow {
                                            set_title: "Continue Watching",
                                            set_icon_name: Some("media-playback-start-symbolic"),
                                            set_activatable: true,
                                        },

                                        adw::ActionRow {
                                            set_title: "Watchlist",
                                            set_icon_name: Some("starred-symbolic"),
                                            set_activatable: true,
                                        }
                                    }
                                }
                            }
                        },

                        #[wrap(Some)]
                        set_content = &adw::NavigationPage {
                            set_title: "Home",

                            #[wrap(Some)]
                            set_child = &gtk::Stack {
                                set_transition_type: gtk::StackTransitionType::SlideLeftRight,
                                set_transition_duration: 250,

                                add_named[Some("home")] = &gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_spacing: 24,
                                    set_halign: gtk::Align::Center,
                                    set_valign: gtk::Align::Center,
                                    set_vexpand: true,
                                    set_hexpand: true,

                                    gtk::Image {
                                        set_icon_name: Some("applications-multimedia-symbolic"),
                                        set_pixel_size: 128,
                                        add_css_class: "dim-label",
                                    },

                                    gtk::Label {
                                        set_label: "Welcome to Reel",
                                        add_css_class: "title-1",
                                        set_halign: gtk::Align::Center,
                                    },

                                    gtk::Label {
                                        set_label: "Your premium media experience starts here.\nConnect your Plex or Jellyfin server to get started.",
                                        add_css_class: "body",
                                        set_halign: gtk::Align::Center,
                                        set_justify: gtk::Justification::Center,
                                    },

                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Horizontal,
                                        set_spacing: 12,
                                        set_halign: gtk::Align::Center,
                                        set_margin_top: 24,

                                        gtk::Button {
                                            set_label: "Add Server",
                                            add_css_class: "suggested-action",
                                            add_css_class: "pill",
                                        },

                                        gtk::Button {
                                            set_label: "Browse Local Files",
                                            add_css_class: "flat",
                                        }
                                    }
                                },

                                add_named[Some("preferences")] = &gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_margin_all: 24,
                                    set_spacing: 18,

                                    gtk::Label {
                                        set_label: "Preferences",
                                        add_css_class: "title-1",
                                        set_halign: gtk::Align::Start,
                                        set_margin_bottom: 12,
                                    },

                                    gtk::Box {
                                        set_orientation: gtk::Orientation::Vertical,
                                        set_spacing: 12,
                                        add_css_class: "card",

                                        gtk::Label {
                                            set_label: "Appearance",
                                            add_css_class: "heading",
                                            set_halign: gtk::Align::Start,
                                        },

                                        gtk::ListBox {
                                            set_selection_mode: gtk::SelectionMode::None,
                                            add_css_class: "boxed-list",

                                            adw::ComboRow {
                                                set_title: "Theme",
                                                set_subtitle: "Choose your preferred theme",
                                            },

                                            adw::SwitchRow {
                                                set_title: "Use System Theme",
                                                set_subtitle: "Follow system dark/light theme preference",
                                                set_active: true,
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    async fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        use crate::config::Config;

        // Get runtime from thread-local
        let runtime = RUNTIME.with(|r| {
            r.borrow()
                .as_ref()
                .expect("Runtime not initialized")
                .clone()
        });

        let config = Config::default();

        // Initialize database directly
        let database = Database::new()
            .await
            .expect("Failed to initialize database");
        let db = database.get_connection();

        let model = AppModel {
            db: db.clone(),
            runtime,
            loading: true,
            current_page: NavigationTarget::Home,
            style_manager: adw::StyleManager::default(),
        };

        let widgets = view_output!();

        // Make sure window is visible
        root.set_visible(true);
        root.present();

        // Simple initialization - no backend management needed
        sender.oneshot_command(async move {
            CommandResult::InitialDataLoaded {
                sources: vec![],
                libraries: vec![],
            }
        });

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        match msg {
            AppInput::Initialize => {
                self.loading = true;
                let db = self.db.clone();
                sender.oneshot_command(async move {
                    super::components::shared::execute_command(AppCommand::LoadInitialData, &db)
                        .await
                });
            }
            AppInput::Navigate(target) => {
                self.current_page = target.clone();
            }
            AppInput::ShowError(error) => {
                eprintln!("Error: {}", error);
            }
            AppInput::ShowLoading(loading) => {
                self.loading = loading;
            }
            AppInput::Quit => {
                relm4::main_application().quit();
            }
        }
    }

    async fn update_cmd(
        &mut self,
        msg: Self::CommandOutput,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            CommandResult::InitialDataLoaded { .. } => {
                self.loading = false;
            }
            CommandResult::Error(error) => {
                self.loading = false;
                eprintln!("Command error: {}", error);
            }
            _ => {}
        }
    }
}
