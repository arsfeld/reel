mod media_card;

use std::sync::Arc;

use gtk4::prelude::*;
use libadwaita as adw;
use relm4::prelude::*;
use relm4::typed_view::grid::TypedGridView;
use tracing::info;

use crate::models::library::LibraryType;
use crate::models::media::MediaItem;
use crate::services::artwork::ArtworkCache;
use crate::services::media_source::MediaSource;

use media_card::MediaCardData;

pub struct LibraryView {
    grid: TypedGridView<MediaCardData, gtk4::SingleSelection>,
    library_type: LibraryType,
    source: Option<Arc<dyn MediaSource>>,
    artwork_cache: Option<Arc<ArtworkCache>>,
    stack: gtk4::Stack,
    loading_page: adw::StatusPage,
    empty_page: adw::StatusPage,
    error_page: adw::StatusPage,
    grid_page: gtk4::ScrolledWindow,
}

pub enum LibraryViewMsg {
    LoadLibrary(LibraryType),
    SetSource(Arc<dyn MediaSource>, Arc<ArtworkCache>),
    ItemActivated(u32),
    LibraryLoaded(Vec<MediaItem>),
    LoadError(String),
}

impl std::fmt::Debug for LibraryViewMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadLibrary(lt) => write!(f, "LoadLibrary({lt:?})"),
            Self::SetSource(..) => write!(f, "SetSource(..)"),
            Self::ItemActivated(pos) => write!(f, "ItemActivated({pos})"),
            Self::LibraryLoaded(items) => write!(f, "LibraryLoaded({} items)", items.len()),
            Self::LoadError(msg) => write!(f, "LoadError({msg})"),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code, clippy::large_enum_variant)]
pub enum LibraryViewOutput {
    ShowDetail(MediaItem),
    Error(String),
}

pub enum LibraryViewCmd {
    Loaded(Vec<MediaItem>),
    Error(String),
    ArtworkReady(usize, gtk4::gdk::Texture),
}

impl std::fmt::Debug for LibraryViewCmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LibraryViewCmd")
    }
}

#[relm4::component(pub)]
impl Component for LibraryView {
    type Init = ();
    type Input = LibraryViewMsg;
    type Output = LibraryViewOutput;
    type CommandOutput = LibraryViewCmd;

    view! {
        #[root]
        gtk4::Box {
            set_orientation: gtk4::Orientation::Vertical,
            set_hexpand: true,
            set_vexpand: true,
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = view_output!();

        let stack = gtk4::Stack::new();
        stack.set_hexpand(true);
        stack.set_vexpand(true);

        let loading_page = adw::StatusPage::builder()
            .title("Loading...")
            .icon_name("content-loading-symbolic")
            .build();

        let empty_page = adw::StatusPage::builder()
            .title("No Media")
            .description("Connect a Plex server to browse your library")
            .icon_name("folder-videos-symbolic")
            .build();

        let error_page = adw::StatusPage::builder()
            .title("Error")
            .icon_name("dialog-error-symbolic")
            .build();

        let grid: TypedGridView<MediaCardData, gtk4::SingleSelection> = TypedGridView::new();
        let grid_view = grid.view.clone();
        grid_view.set_min_columns(3);
        grid_view.set_max_columns(10);
        grid_view.set_margin_start(12);
        grid_view.set_margin_end(12);
        grid_view.set_margin_top(12);
        grid_view.set_margin_bottom(12);

        let sender_activate = sender.input_sender().clone();
        grid_view.connect_activate(move |_view, position| {
            let _ = sender_activate.send(LibraryViewMsg::ItemActivated(position));
        });

        let grid_page = gtk4::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&grid_view)
            .build();

        stack.add_child(&loading_page);
        stack.add_child(&empty_page);
        stack.add_child(&error_page);
        stack.add_child(&grid_page);
        stack.set_visible_child(&empty_page);

        root.append(&stack);

        let model = Self {
            grid,
            library_type: LibraryType::Movie,
            source: None,
            artwork_cache: None,
            stack,
            loading_page,
            empty_page,
            error_page,
            grid_page,
        };

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>, _root: &Self::Root) {
        match msg {
            LibraryViewMsg::SetSource(source, artwork_cache) => {
                self.source = Some(source);
                self.artwork_cache = Some(artwork_cache);
            }
            LibraryViewMsg::LoadLibrary(library_type) => {
                self.library_type = library_type;
                self.stack.set_visible_child(&self.loading_page);

                let Some(source) = self.source.clone() else {
                    self.stack.set_visible_child(&self.empty_page);
                    return;
                };

                let lt = library_type;
                sender.oneshot_command(async move {
                    match source.libraries().await {
                        Ok(libs) => {
                            let target_type = match lt {
                                LibraryType::Movie => "movie",
                                LibraryType::Show => "show",
                            };
                            let mut all_items = Vec::new();
                            for lib in libs
                                .iter()
                                .filter(|l| l.library_type.as_str() == target_type)
                            {
                                match source.library_items(&lib.key).await {
                                    Ok(items) => all_items.extend(items),
                                    Err(e) => return LibraryViewCmd::Error(e.to_string()),
                                }
                            }
                            LibraryViewCmd::Loaded(all_items)
                        }
                        Err(e) => LibraryViewCmd::Error(e.to_string()),
                    }
                });
            }
            LibraryViewMsg::ItemActivated(position) => {
                if let Some(item) = self.grid.get(position) {
                    let borrow = item.borrow();
                    if let Some(ref media_item) = borrow.media_item {
                        let _ = sender.output(LibraryViewOutput::ShowDetail(media_item.clone()));
                    }
                }
            }
            LibraryViewMsg::LibraryLoaded(items) => {
                self.grid.clear();

                if items.is_empty() {
                    self.stack.set_visible_child(&self.empty_page);
                    return;
                }

                let artwork_cache = self.artwork_cache.clone();
                let source = self.source.clone();

                for (idx, item) in items.iter().enumerate() {
                    let card = MediaCardData::from_media_item(item);
                    self.grid.append(card);

                    if let (Some(poster_path), Some(source), Some(cache)) =
                        (&item.poster_path, &source, &artwork_cache)
                    {
                        let url = source.artwork_url(poster_path, 300, 450);
                        let cache = Arc::clone(cache);
                        sender.oneshot_command(async move {
                            match cache.get_or_download(&url).await {
                                Ok(path) => match gtk4::gdk::Texture::from_filename(&path) {
                                    Ok(texture) => LibraryViewCmd::ArtworkReady(idx, texture),
                                    Err(_) => LibraryViewCmd::Error(String::new()),
                                },
                                Err(_) => LibraryViewCmd::Error(String::new()),
                            }
                        });
                    }
                }

                self.stack.set_visible_child(&self.grid_page);
                info!("Library loaded: {} items", items.len());
            }
            LibraryViewMsg::LoadError(msg) => {
                self.error_page.set_description(Some(&msg));
                self.stack.set_visible_child(&self.error_page);
            }
        }
    }

    fn update_cmd(
        &mut self,
        cmd: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match cmd {
            LibraryViewCmd::Loaded(items) => {
                sender.input(LibraryViewMsg::LibraryLoaded(items));
            }
            LibraryViewCmd::Error(msg) => {
                if !msg.is_empty() {
                    sender.input(LibraryViewMsg::LoadError(msg));
                }
            }
            LibraryViewCmd::ArtworkReady(idx, texture) => {
                if let Some(item) = self.grid.get(idx as u32) {
                    item.borrow_mut().poster_texture = Some(texture);
                }
            }
        }
    }
}
