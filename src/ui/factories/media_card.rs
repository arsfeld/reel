use crate::db::entities::MediaItemModel;
use crate::models::MediaItemId;
use gtk::prelude::*;
use relm4::factory::FactoryComponent;
use relm4::prelude::*;

#[derive(Debug, Clone)]
pub struct MediaCardInit {
    pub item: MediaItemModel,
    pub show_progress: bool,
    pub watched: bool,
    pub progress_percent: f64,
    pub show_media_type_icon: bool, // For mixed libraries
}

#[tracker::track]
#[derive(Debug)]
pub struct MediaCard {
    item: MediaItemModel,
    #[do_not_track]
    item_id: MediaItemId,
    #[do_not_track]
    parent_show_id: Option<MediaItemId>, // For episodes, ID of parent show
    show_progress: bool,
    show_media_type_icon: bool,
    hover: bool,
    selected: bool,
    progress_percent: f64,
    image_loaded: bool,
    load_failed: bool,
    watched: bool,
    #[do_not_track]
    texture: Option<gtk::gdk::Texture>,
    #[do_not_track]
    popover: Option<gtk::PopoverMenu>,
}

#[derive(Debug, Clone)]
pub enum MediaCardInput {
    SetHover(bool),
    SetSelected(bool),
    UpdateProgress(f64),
    ImageLoaded(gtk::gdk::Texture),
    ImageLoadFailed,
    Play,
}

#[derive(Debug, Clone)]
pub enum MediaCardOutput {
    Clicked(MediaItemId),
    Play(MediaItemId),
    GoToShow(MediaItemId), // Navigate to parent show (for episodes)
    MarkWatched(MediaItemId),
    MarkUnwatched(MediaItemId),
}

#[allow(unused_assignments)]
#[relm4::factory(pub)]
impl FactoryComponent for MediaCard {
    type Init = MediaCardInit;
    type Input = MediaCardInput;
    type Output = MediaCardOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::FlowBox;

    view! {
        // Root button matching GTK MediaCard button style
        root = gtk::Button {
            add_css_class: "flat",
            add_css_class: "media-card",
            add_css_class: "poster-card",
            set_width_request: 180,
            set_height_request: 270,

            // Main overlay container
            gtk::Overlay {
                add_css_class: "poster-overlay",

                // Poster image with proper aspect ratio
                #[name(poster)]
                gtk::Picture {
                    set_content_fit: gtk::ContentFit::Cover,
                    set_can_shrink: true,
                    set_width_request: 180,
                    set_height_request: 270,
                    add_css_class: "rounded-poster",
                    #[track(self.changed(MediaCard::image_loaded()))]
                    add_css_class: if self.image_loaded { "poster-fade-in" } else { "poster-skeleton" },
                },


                // Info gradient at bottom
                add_overlay = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_valign: gtk::Align::End,
                    add_css_class: "poster-info-gradient",

                    // Inner box with text
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 2,
                        set_margin_bottom: 4,
                        set_margin_start: 4,
                        set_margin_end: 4,
                        set_margin_top: 4,
                        add_css_class: "media-card-info",

                        gtk::Label {
                            set_label: &self.item.title,
                            set_xalign: 0.0,
                            set_single_line_mode: true,
                            set_ellipsize: gtk::pango::EllipsizeMode::End,
                            add_css_class: "title-4",
                        },

                        gtk::Label {
                            set_label: &self.format_subtitle(),
                            set_xalign: 0.0,
                            set_single_line_mode: true,
                            set_ellipsize: gtk::pango::EllipsizeMode::End,
                            add_css_class: "subtitle",
                            #[track(self.changed(MediaCard::item()))]
                            set_visible: !self.format_subtitle().is_empty(),
                        }
                    }
                },

                // Media type icon (top-left for mixed libraries)
                add_overlay = &gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_halign: gtk::Align::Start,
                    set_valign: gtk::Align::Start,
                    set_margin_top: 8,
                    set_margin_start: 8,
                    #[track(self.changed(MediaCard::show_media_type_icon()))]
                    set_visible: self.show_media_type_icon,

                    gtk::Box {
                        set_width_request: 32,
                        set_height_request: 32,
                        add_css_class: "osd",
                        add_css_class: "circular",
                        set_halign: gtk::Align::Center,
                        set_valign: gtk::Align::Center,

                        gtk::Image {
                            set_icon_name: Some(&self.get_media_type_icon()),
                            set_pixel_size: 16,
                        }
                    }
                },

                // Unwatched indicator (top-right glowing dot)
                add_overlay = &gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_halign: gtk::Align::End,
                    set_valign: gtk::Align::Start,
                    set_margin_top: 4,
                    set_margin_end: 4,
                    #[track(self.changed(MediaCard::item()))]
                    set_visible: !self.is_watched(),
                    add_css_class: "unwatched-indicator",

                    gtk::Box {
                        set_width_request: 8,
                        set_height_request: 8,
                        add_css_class: "unwatched-glow-dot",
                    }
                },

                // Progress bar overlay (bottom)
                add_overlay = &gtk::ProgressBar {
                    set_valign: gtk::Align::End,
                    #[track(self.changed(MediaCard::progress_percent()))]
                    set_visible: self.is_partially_watched(),
                    #[track(self.changed(MediaCard::progress_percent()))]
                    set_fraction: self.progress_percent,
                    add_css_class: "media-progress",
                }
            },

            connect_clicked[sender, item_id = self.item_id.clone()] => move |_| {
                sender.output(MediaCardOutput::Clicked(item_id.clone())).unwrap();
            }
        }
    }

    fn init_widgets(
        &mut self,
        _index: &DynamicIndex,
        root: Self::Root,
        _returned_widget: &<Self::ParentWidget as relm4::factory::FactoryView>::ReturnedWidget,
        sender: FactorySender<Self>,
    ) -> Self::Widgets {
        // Set up context menu BEFORE calling view_output! (which consumes root)

        // Create context menu
        let menu = gtk::gio::Menu::new();

        // Add "Play" action for all media types
        menu.append(Some("Play"), Some("card.play"));

        // Add "Go to Show" for episodes
        if self.item.media_type == "episode" && self.parent_show_id.is_some() {
            menu.append(Some("Go to Show"), Some("card.go_to_show"));
        }

        // Add watch status toggle
        if self.watched {
            menu.append(Some("Mark as Unwatched"), Some("card.mark_unwatched"));
        } else {
            menu.append(Some("Mark as Watched"), Some("card.mark_watched"));
        }

        // Create popover menu
        let popover = gtk::PopoverMenu::from_model(Some(&menu));
        popover.set_parent(&root);
        popover.set_has_arrow(false);

        // Store popover reference for cleanup in shutdown
        self.popover = Some(popover.clone());

        // Create action group for menu actions
        let action_group = gtk::gio::SimpleActionGroup::new();

        // Play action
        let play_action = gtk::gio::SimpleAction::new("play", None);
        let sender_clone = sender.clone();
        let item_id = self.item_id.clone();
        play_action.connect_activate(move |_, _| {
            sender_clone
                .output(MediaCardOutput::Play(item_id.clone()))
                .unwrap();
        });
        action_group.add_action(&play_action);

        // Go to Show action (if episode)
        if let Some(parent_id) = &self.parent_show_id {
            let go_to_show_action = gtk::gio::SimpleAction::new("go_to_show", None);
            let sender_clone = sender.clone();
            let parent_id_clone = parent_id.clone();
            go_to_show_action.connect_activate(move |_, _| {
                sender_clone
                    .output(MediaCardOutput::GoToShow(parent_id_clone.clone()))
                    .unwrap();
            });
            action_group.add_action(&go_to_show_action);
        }

        // Mark Watched action
        let mark_watched_action = gtk::gio::SimpleAction::new("mark_watched", None);
        let sender_clone = sender.clone();
        let item_id_clone = self.item_id.clone();
        mark_watched_action.connect_activate(move |_, _| {
            sender_clone
                .output(MediaCardOutput::MarkWatched(item_id_clone.clone()))
                .unwrap();
        });
        action_group.add_action(&mark_watched_action);

        // Mark Unwatched action
        let mark_unwatched_action = gtk::gio::SimpleAction::new("mark_unwatched", None);
        let sender_clone = sender.clone();
        let item_id_clone = self.item_id.clone();
        mark_unwatched_action.connect_activate(move |_, _| {
            sender_clone
                .output(MediaCardOutput::MarkUnwatched(item_id_clone.clone()))
                .unwrap();
        });
        action_group.add_action(&mark_unwatched_action);

        // Insert action group
        root.insert_action_group("card", Some(&action_group));

        // Add right-click gesture
        let gesture = gtk::GestureClick::new();
        gesture.set_button(3); // Right mouse button
        let popover_clone = popover.clone();
        gesture.connect_released(move |gesture, _, x, y| {
            // Get the widget that was clicked
            if let Some(_widget) = gesture.widget() {
                // Calculate the position relative to the widget
                let rect = gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1);
                popover_clone.set_pointing_to(Some(&rect));
                popover_clone.popup();
            }
        });
        root.add_controller(gesture);

        // Now call view_output! which will consume root
        let widgets = view_output!();
        widgets
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        let item_id = MediaItemId::new(init.item.id.clone());

        // For episodes, get parent show ID for "Go to Show" context menu action
        let parent_show_id = init
            .item
            .parent_id
            .as_ref()
            .map(|id| MediaItemId::new(id.clone()));

        // Debug: Creating card for item with appropriate initial state

        // Check if we already have poster metadata - if no poster URL, mark as loaded with placeholder
        let has_poster =
            init.item.poster_url.is_some() && !init.item.poster_url.as_ref().unwrap().is_empty();
        let image_loaded = !has_poster; // If no poster URL, consider it "loaded" to hide spinner

        Self {
            item: init.item,
            item_id,
            parent_show_id,
            show_progress: init.show_progress,
            show_media_type_icon: init.show_media_type_icon,
            hover: false,
            selected: false,
            progress_percent: init.progress_percent,
            image_loaded,
            load_failed: false,
            watched: init.watched,
            texture: None,
            popover: None,
            tracker: 0,
        }
    }

    fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        msg: Self::Input,
        sender: FactorySender<Self>,
    ) {
        self.reset();

        match msg {
            MediaCardInput::SetHover(hover) => {
                self.set_hover(hover);
            }
            MediaCardInput::SetSelected(selected) => {
                self.set_selected(selected);
            }
            MediaCardInput::UpdateProgress(progress) => {
                self.set_progress_percent(progress);
            }
            MediaCardInput::ImageLoaded(texture) => {
                // Set the texture on the picture widget
                widgets.poster.set_paintable(Some(&texture));
                self.texture = Some(texture);
                self.set_image_loaded(true);
                // Image successfully loaded
            }
            MediaCardInput::ImageLoadFailed => {
                self.set_load_failed(true);
                // Image failed to load - spinner will be hidden
            }
            MediaCardInput::Play => {
                sender
                    .output(MediaCardOutput::Play(self.item_id.clone()))
                    .unwrap();
            }
        }
    }

    fn shutdown(&mut self, _widgets: &mut Self::Widgets, _output: relm4::Sender<Self::Output>) {
        // Properly unparent the popover before the button is finalized
        // This prevents GTK warnings about finalizing buttons with children
        if let Some(popover) = self.popover.take() {
            popover.unparent();
        }
    }
}

impl MediaCard {
    fn format_subtitle(&self) -> String {
        match self.item.media_type.as_str() {
            "movie" => {
                if let Some(year) = self.item.year {
                    format!("{}", year)
                } else {
                    String::new()
                }
            }
            "show" => {
                // For shows, we'll extract episode count from metadata if available
                // Otherwise default to "TV Series"
                if let Some(metadata) = &self.item.metadata {
                    // Try to extract episode count from metadata JSON
                    if let Some(episode_count) =
                        metadata.get("total_episode_count").and_then(|v| v.as_u64())
                    {
                        if episode_count == 1 {
                            "1 episode".to_string()
                        } else {
                            format!("{} episodes", episode_count)
                        }
                    } else if let Some(season_count) =
                        metadata.get("season_count").and_then(|v| v.as_u64())
                    {
                        if season_count == 1 {
                            "1 season".to_string()
                        } else {
                            format!("{} seasons", season_count)
                        }
                    } else {
                        "TV Series".to_string()
                    }
                } else {
                    "TV Series".to_string()
                }
            }
            "episode" => {
                // For episodes, check if we have a custom episode subtitle in metadata
                // (set when we're displaying the show poster instead of episode thumbnail)
                if let Some(metadata) = &self.item.metadata
                    && let Some(episode_subtitle) =
                        metadata.get("episode_subtitle").and_then(|v| v.as_str())
                {
                    return episode_subtitle.to_string();
                }

                // Otherwise format like normal
                if let (Some(season), Some(episode)) =
                    (self.item.season_number, self.item.episode_number)
                {
                    format!("S{}E{}", season, episode)
                } else {
                    "Episode".to_string()
                }
            }
            _ => String::new(),
        }
    }

    fn is_watched(&self) -> bool {
        // This will be determined by playback progress from the database
        // For now, use the watched field we track
        self.watched
    }

    fn is_partially_watched(&self) -> bool {
        // Check if progress is between 0 and 90%
        self.progress_percent > 0.0 && self.progress_percent < 0.9
    }

    fn get_media_type_icon(&self) -> String {
        match self.item.media_type.as_str() {
            "movie" => "video-x-generic-symbolic",
            "show" => "video-display-symbolic",
            "episode" => "video-display-symbolic",
            "album" => "media-optical-cd-audio-symbolic",
            "track" => "audio-x-generic-symbolic",
            "photo" => "image-x-generic-symbolic",
            _ => "folder-symbolic",
        }
        .to_string()
    }
}
