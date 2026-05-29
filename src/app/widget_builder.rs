use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;

use super::App;
use super::AppMsg;
use crate::components::downloads::DownloadsView;
use crate::components::home::HomeView;
use crate::components::library::LibraryView;
use crate::components::player::video_player::VideoPlayer;
use crate::components::sidebar::Sidebar;
use crate::services::window_state::{self, WindowState};

use super::utils::parse_uri_list;

/// All widgets built during init() that the App model needs to hold.
pub struct BuiltWidgets {
    pub toast_overlay: adw::ToastOverlay,
    pub stack: gtk::Stack,
    pub nav_view: adw::NavigationView,
    pub split_view: adw::NavigationSplitView,
    pub library_title: gtk::Label,
    pub player_chrome_revealer: gtk::Revealer,
    pub player_window_title: adw::WindowTitle,
}

/// Build the full widget hierarchy and attach controllers to the root window.
#[allow(clippy::too_many_lines)]
pub fn build_widgets(
    root: &adw::ApplicationWindow,
    sender: &ComponentSender<App>,
    sidebar: &Controller<Sidebar>,
    home_view: &Controller<HomeView>,
    library_view: &Controller<LibraryView>,
    video_player: &Controller<VideoPlayer>,
    downloads_view: &Controller<DownloadsView>,
) -> BuiltWidgets {
    let toast_overlay = adw::ToastOverlay::new();
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .build();

    // Shell page: sidebar + navigation content
    let split_view = adw::NavigationSplitView::new();

    let sidebar_nav_page = adw::NavigationPage::builder()
        .title("Library")
        .child(sidebar.widget())
        .build();
    split_view.set_sidebar(Some(&sidebar_nav_page));

    let nav_view = adw::NavigationView::new();

    // --- Library title label (updated when sidebar nav changes) ---
    let library_title = gtk::Label::builder()
        .label("Movies")
        .css_classes(["title"])
        .build();

    // --- Hamburger menu (shared between all views) ---
    let menu = gtk::gio::Menu::new();
    menu.append(Some("Preferences"), Some("app.preferences"));
    menu.append(Some("Add Source"), Some("app.add-source"));
    menu.append(Some("About Reel"), Some("app.about"));

    // Register menu actions once on the root window
    let action_group = gtk::gio::SimpleActionGroup::new();

    let prefs_action = gtk::gio::SimpleAction::new("preferences", None);
    let sender_prefs = sender.input_sender().clone();
    prefs_action.connect_activate(move |_, _| {
        let _ = sender_prefs.send(AppMsg::OpenPreferences);
    });
    action_group.add_action(&prefs_action);

    let conn_action = gtk::gio::SimpleAction::new("add-source", None);
    let sender_conn = sender.input_sender().clone();
    conn_action.connect_activate(move |_, _| {
        let _ = sender_conn.send(AppMsg::ShowConnectionDialog);
    });
    action_group.add_action(&conn_action);

    let about_action = gtk::gio::SimpleAction::new("about", None);
    let sender_about = sender.input_sender().clone();
    about_action.connect_activate(move |_, _| {
        let _ = sender_about.send(AppMsg::OpenAbout);
    });
    action_group.add_action(&about_action);

    root.insert_action_group("app", Some(&action_group));

    // --- Library root page (header bar + switcher bar + content) ---
    let library_toolbar = adw::ToolbarView::new();
    let library_header = adw::HeaderBar::new();

    library_header.set_title_widget(Some(&library_title));

    let lib_menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("Menu")
        .menu_model(&menu)
        .primary(true)
        .build();
    library_header.pack_end(&lib_menu_button);

    library_toolbar.add_top_bar(&library_header);
    library_toolbar.set_content(Some(library_view.widget()));

    let library_nav_page = adw::NavigationPage::builder()
        .title("Library")
        .tag("library")
        .child(&library_toolbar)
        .build();
    nav_view.add(&library_nav_page);

    // Home page — lives inside the split view so the sidebar is always visible
    let home_toolbar = adw::ToolbarView::new();
    let home_header = adw::HeaderBar::new();

    let home_menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("Menu")
        .menu_model(&menu)
        .primary(true)
        .build();
    home_header.pack_end(&home_menu_button);

    home_toolbar.add_top_bar(&home_header);
    home_toolbar.set_content(Some(home_view.widget()));

    let home_nav_page = adw::NavigationPage::builder()
        .title("Home")
        .tag("home")
        .child(&home_toolbar)
        .build();
    nav_view.add(&home_nav_page);

    // Downloads page — its own top-level destination (source-independent).
    let downloads_toolbar = adw::ToolbarView::new();
    let downloads_header = adw::HeaderBar::new();
    downloads_header.set_title_widget(Some(
        &gtk::Label::builder()
            .label("Downloads")
            .css_classes(["title"])
            .build(),
    ));
    let downloads_menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("Menu")
        .menu_model(&menu)
        .primary(true)
        .build();
    downloads_header.pack_end(&downloads_menu_button);
    downloads_toolbar.add_top_bar(&downloads_header);
    downloads_toolbar.set_content(Some(downloads_view.widget()));

    let downloads_nav_page = adw::NavigationPage::builder()
        .title("Downloads")
        .tag("downloads")
        .child(&downloads_toolbar)
        .build();
    nav_view.add(&downloads_nav_page);

    let content_nav_page = adw::NavigationPage::builder()
        .title("Content")
        .child(&nav_view)
        .build();
    split_view.set_content(Some(&content_nav_page));

    // --- Player chrome (windowed overlay, synced to OSD) ---
    let player_window_title = adw::WindowTitle::new("Reel", "");
    let player_back = gtk::Button::builder()
        .icon_name("go-back-symbolic")
        .tooltip_text("Back to library (Esc)")
        .css_classes(["flat"])
        .build();
    let sender_back = sender.input_sender().clone();
    player_back.connect_clicked(move |_| {
        let _ = sender_back.send(AppMsg::GoBack);
    });

    let player_header = adw::HeaderBar::builder()
        .css_classes(["flat", "video-chrome"])
        .build();
    player_header.pack_start(&player_back);
    player_header.set_title_widget(Some(&player_window_title));

    let player_chrome_revealer = gtk::Revealer::builder()
        .child(&player_header)
        .reveal_child(false)
        .transition_type(gtk::RevealerTransitionType::Crossfade)
        .transition_duration(180)
        .valign(gtk::Align::Start)
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .build();

    // --- Responsive sidebar ---
    // AdwNavigationSplitView handles its own responsive collapse natively —
    // when collapsed, it moves sidebar + content into an internal NavigationView.

    // Player page: video with windowed chrome overlaid at the top.
    // AdwApplicationWindow does not support gtk_window_set_titlebar().
    let player_overlay = gtk::Overlay::new();
    player_overlay.set_child(Some(video_player.widget()));
    player_overlay.add_overlay(&player_chrome_revealer);
    video_player.widget().set_vexpand(true);

    let player_page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    player_page.append(&player_overlay);
    player_overlay.set_vexpand(true);

    stack.add_named(&split_view, Some("shell"));
    stack.add_named(&player_page, Some("player"));

    toast_overlay.set_child(Some(&stack));
    root.set_content(Some(&toast_overlay));

    // Keyboard shortcuts (library-level only; player handles its own)
    let key_controller = gtk::EventControllerKey::new();
    let sender_key = sender.input_sender().clone();
    let root_key = root.clone();
    key_controller.connect_key_pressed(move |_controller, key, _code, mods| {
        // Detect if a text input widget has focus
        let is_text_focused = gtk::prelude::GtkWindowExt::focus(&root_key).is_some_and(|w| {
            w.is::<gtk::SearchEntry>()
                || w.is::<gtk::Entry>()
                || w.is::<gtk::Text>()
                || w.is::<gtk::TextView>()
        });

        if !is_text_focused {
            let ctrl = mods.contains(gtk::gdk::ModifierType::CONTROL_MASK);
            match key {
                gtk::gdk::Key::f if ctrl => {
                    let _ = sender_key.send(AppMsg::FocusSearch);
                    glib::Propagation::Stop
                }
                gtk::gdk::Key::slash if mods.is_empty() => {
                    let _ = sender_key.send(AppMsg::FocusSearch);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        } else {
            glib::Propagation::Proceed
        }
    });
    root.add_controller(key_controller);

    // Drag-and-drop
    let drop_target = gtk::DropTarget::builder()
        .actions(gtk::gdk::DragAction::COPY)
        .build();
    drop_target.set_types(&[glib::types::Type::STRING]);
    let sender_drop = sender.input_sender().clone();
    drop_target.connect_drop(move |_target, value, _x, _y| {
        if let Ok(text) = value.get::<String>() {
            for uri in parse_uri_list(&text) {
                let _ = sender_drop.send(AppMsg::FilesDropped(uri));
            }
            return true;
        }
        false
    });
    root.add_controller(drop_target);

    // Load window state
    let ws = window_state::load();
    root.set_default_size(ws.width, ws.height);
    if ws.maximized {
        root.maximize();
    }

    // Save window state on close
    let root_close = root.clone();
    root.connect_close_request(move |_window| {
        let (width, height) = root_close.default_size();
        let state = WindowState {
            width,
            height,
            maximized: root_close.is_maximized(),
            volume: 100.0,
            ..window_state::load()
        };
        if let Err(e) = window_state::save(&state) {
            tracing::warn!("Failed to save window state: {}", e);
        }
        glib::Propagation::Proceed
    });

    BuiltWidgets {
        toast_overlay,
        stack,
        nav_view,
        split_view,
        library_title,
        player_chrome_revealer,
        player_window_title,
    }
}
