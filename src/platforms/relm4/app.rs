use super::components::shared::{AppCommand, AppInput, AppOutput, CommandResult, NavigationTarget};
use crate::core::state::AppState;
use crate::services::DataService;
use libadwaita as adw;
use libadwaita::prelude::*;
use relm4::gtk;
use relm4::gtk::prelude::*;
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
        let app = RelmApp::from_app(app);
        relm4::set_global_css(
            "
            .navigation-sidebar {
                background: transparent;
            }
            .title-1 {
                font-size: 24pt;
                font-weight: bold;
            }
            ",
        );
        app.run_async::<AppModel>(());

        Ok(())
    }
}

pub struct AppModel {
    app_state: Arc<AppState>,
    runtime: Arc<Runtime>,
    data_service: Arc<DataService>,
    loading: bool,
    current_page: NavigationTarget,
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
                    gtk::Spinner {
                        set_spinning: true,
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,
                        set_vexpand: true,
                        set_hexpand: true,
                        set_width_request: 32,
                        set_height_request: 32,
                    }
                } else {
                    adw::NavigationSplitView {
                        set_collapsed: false,
                        set_show_content: true,

                        #[wrap(Some)]
                        set_sidebar = &adw::NavigationPage {
                            #[wrap(Some)]
                            set_child = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_width_request: 280,

                            gtk::ScrolledWindow {
                                set_vexpand: true,

                                gtk::ListBox {
                                    set_selection_mode: gtk::SelectionMode::Single,
                                    add_css_class: "navigation-sidebar",

                                    gtk::ListBoxRow {
                                        gtk::Box {
                                            set_orientation: gtk::Orientation::Horizontal,
                                            set_spacing: 12,
                                            set_margin_all: 12,

                                            gtk::Image {
                                                set_icon_name: Some("user-home-symbolic"),
                                            },

                                            gtk::Label {
                                                set_label: "Home",
                                                set_xalign: 0.0,
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        },

                        #[wrap(Some)]
                        set_content = &adw::NavigationPage {
                            #[wrap(Some)]
                            set_child = &gtk::Stack {
                            set_transition_type: gtk::StackTransitionType::Crossfade,

                            add_named[Some("home")] = &gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,

                                gtk::Label {
                                    set_label: "Welcome to Reel",
                                    set_vexpand: true,
                                    set_hexpand: true,
                                    add_css_class: "title-1",
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
        use tokio::sync::RwLock;

        // Get runtime from thread-local
        let runtime = RUNTIME.with(|r| {
            r.borrow()
                .as_ref()
                .expect("Runtime not initialized")
                .clone()
        });

        let config = Arc::new(RwLock::new(Config::default()));
        let app_state = AppState::new_async(config.clone())
            .await
            .expect("Failed to initialize AppState");

        let data_service = app_state.data_service.clone();

        let model = AppModel {
            app_state: Arc::new(app_state),
            runtime,
            data_service: data_service.clone(),
            loading: true,
            current_page: NavigationTarget::Home,
        };

        let widgets = view_output!();

        // Make sure window is visible
        root.set_visible(true);
        root.present();

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
        _root: &Self::Root,
    ) {
        match msg {
            AppInput::Initialize => {
                self.loading = true;
                let data_service = self.data_service.clone();
                sender.oneshot_command(async move {
                    super::components::shared::execute_command(
                        AppCommand::LoadInitialData,
                        data_service,
                    )
                    .await
                });
            }
            AppInput::Navigate(target) => {
                self.current_page = target.clone();
                sender.output(AppOutput::NavigationChanged(target)).unwrap();
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
