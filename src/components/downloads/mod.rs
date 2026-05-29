//! The Downloads view: offline downloads as a grid of poster cards — one card
//! per standalone movie and one aggregate card per show (with "6/10 episodes"
//! status), each with status + progress overlaid and per-item / per-group
//! actions revealed on hover. Renders entirely from persisted repo state, so it
//! works with no source reachable (R16). Row derivation is pure (see [`row`]);
//! this component only turns rows into cards and relays user actions.

pub mod row;

use std::collections::HashMap;
use std::sync::Arc;

use adw::prelude::*;
use relm4::prelude::*;

use crate::models::download::{Download, DownloadGroup, DownloadState, FailReason};
use crate::services::artwork::ArtworkCache;
use crate::services::download::GroupStatus;
use crate::services::media_source::MediaSource;

use row::{DownloadItemView, DownloadRow, derive_rows, group_retry_targets};

/// Displayed poster-card size in the downloads grid (2:3 movie poster),
/// matching the library grid's proportions at a slightly smaller scale.
const CARD_W: i32 = 160;
const CARD_H: i32 = 240;
/// Requested transcode size (2× the display size for crisp HiDPI rendering).
const POSTER_REQ_W: u32 = 320;
const POSTER_REQ_H: u32 = 480;

/// GTK widget handles for a rendered card, kept so progress ticks and arriving
/// artwork can update the card in place without rebuilding the whole grid.
struct RowWidgets {
    picture: gtk::Picture,
    placeholder: gtk::Image,
    /// The artwork URL this card's poster was requested at, used to match an
    /// arriving [`DownloadsViewMsg::ArtworkReady`].
    poster_url: Option<String>,
    /// Present for item cards (movies/episodes); `None` for show cards.
    progress_bar: Option<gtk::ProgressBar>,
    status_label: Option<gtk::Label>,
}

/// A user action on a single download. Defined here (the UI layer) and consumed
/// by the app's download handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum DownloadItemAction {
    Pause,
    Resume,
    /// Cancel an in-flight/queued download and discard its partial file.
    Cancel,
    /// Delete a completed (or any) download, removing its files.
    Delete,
    /// Retry a failed download.
    Retry,
}

pub struct DownloadsView {
    downloads: Vec<Download>,
    groups: Vec<DownloadGroup>,
    /// Whether the over-budget warn-only banner should show.
    over_budget_warning: bool,
    /// Whether the disk-full banner should show.
    disk_full: bool,
    grid: gtk::FlowBox,
    banner: adw::Banner,
    /// Active source, used to resolve a poster path into an artwork URL.
    source: Option<Arc<dyn MediaSource>>,
    /// Shared artwork cache for downloading/caching posters off the main loop.
    artwork_cache: Option<Arc<ArtworkCache>>,
    /// In-memory texture cache keyed by artwork URL, so a rebuild reuses already
    /// loaded posters instead of re-fetching (mirrors the library grid).
    texture_cache: HashMap<String, gtk::gdk::Texture>,
    /// Per-card widget handles for in-place progress/poster updates. Keyed by
    /// `media_item_id` for item cards and by `group_id` for show cards.
    row_widgets: HashMap<String, RowWidgets>,
}

#[allow(dead_code)]
pub enum DownloadsViewMsg {
    /// Replace the rendered state with the latest repo snapshot + banner flags.
    SetDownloads {
        downloads: Vec<Download>,
        groups: Vec<DownloadGroup>,
        over_budget_warning: bool,
        disk_full: bool,
    },
    /// Provide the active source + artwork cache so rows can show posters.
    SetSource(Arc<dyn MediaSource>, Arc<ArtworkCache>),
    /// A transfer advanced: update just this row's progress bar + status line.
    Progress {
        media_item_id: String,
        downloaded: u64,
        total: Option<u64>,
    },
    /// A poster finished downloading; apply it to every row using this URL.
    ArtworkReady {
        url: String,
        texture: gtk::gdk::Texture,
    },
    /// A per-item action button was clicked.
    Action {
        media_item_id: String,
        action: DownloadItemAction,
    },
    /// Apply an action to every member of a group (retry-failed / delete / etc).
    GroupAction {
        media_item_ids: Vec<String>,
        action: DownloadItemAction,
    },
}

// Manual `Debug` (relm4 requires it) because `Arc<dyn MediaSource>` and
// `gtk::gdk::Texture` payloads aren't worth printing in full.
impl std::fmt::Debug for DownloadsViewMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SetDownloads { .. } => write!(f, "SetDownloads(..)"),
            Self::SetSource(..) => write!(f, "SetSource(..)"),
            Self::Progress {
                media_item_id,
                downloaded,
                total,
            } => write!(f, "Progress({media_item_id}, {downloaded}, {total:?})"),
            Self::ArtworkReady { url, .. } => write!(f, "ArtworkReady({url})"),
            Self::Action {
                media_item_id,
                action,
            } => write!(f, "Action({media_item_id}, {action:?})"),
            Self::GroupAction { action, .. } => write!(f, "GroupAction({action:?})"),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum DownloadsViewOutput {
    /// Forward a per-item action to the app's download handlers.
    ItemAction {
        media_item_id: String,
        action: DownloadItemAction,
    },
    /// Open the detail page for the clicked card. Carries a `media_item_id`:
    /// a movie's own id, or (for a show card) any member episode's id — the app
    /// resolves an episode up to its parent show.
    OpenDetail(String),
}

#[relm4::component(pub)]
impl SimpleComponent for DownloadsView {
    type Init = ();
    type Input = DownloadsViewMsg;
    type Output = DownloadsViewOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            #[name = "banner"]
            adw::Banner {
                set_revealed: false,
            },

            gtk::ScrolledWindow {
                set_vexpand: true,
                set_hscrollbar_policy: gtk::PolicyType::Never,

                #[name = "grid"]
                gtk::FlowBox {
                    add_css_class: "downloads-grid",
                    set_selection_mode: gtk::SelectionMode::None,
                    set_homogeneous: true,
                    set_min_children_per_line: 1,
                    set_max_children_per_line: 12,
                    set_column_spacing: 4,
                    set_row_spacing: 4,
                    set_margin_top: 16,
                    set_margin_bottom: 16,
                    set_margin_start: 16,
                    set_margin_end: 16,
                    set_valign: gtk::Align::Start,
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let widgets = view_output!();
        let mut model = Self {
            downloads: Vec::new(),
            groups: Vec::new(),
            over_budget_warning: false,
            disk_full: false,
            grid: widgets.grid.clone(),
            banner: widgets.banner.clone(),
            source: None,
            artwork_cache: None,
            texture_cache: HashMap::new(),
            row_widgets: HashMap::new(),
        };
        model.rebuild(&sender);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            DownloadsViewMsg::SetDownloads {
                downloads,
                groups,
                over_budget_warning,
                disk_full,
            } => {
                self.downloads = downloads;
                self.groups = groups;
                self.over_budget_warning = over_budget_warning;
                self.disk_full = disk_full;
                self.rebuild(&sender);
            }
            DownloadsViewMsg::SetSource(source, artwork_cache) => {
                self.source = Some(source);
                self.artwork_cache = Some(artwork_cache);
                // Posters need the source's artwork URL; re-render now that it's
                // available (the first snapshot rendered before the source did).
                self.rebuild(&sender);
            }
            DownloadsViewMsg::Progress {
                media_item_id,
                downloaded,
                total,
            } => {
                self.apply_progress(&media_item_id, downloaded, total);
            }
            DownloadsViewMsg::ArtworkReady { url, texture } => {
                self.texture_cache.insert(url.clone(), texture.clone());
                for rw in self.row_widgets.values() {
                    if rw.poster_url.as_deref() == Some(url.as_str()) {
                        rw.picture.set_paintable(Some(&texture));
                        rw.placeholder.set_visible(false);
                    }
                }
            }
            DownloadsViewMsg::Action {
                media_item_id,
                action,
            } => {
                let _ = sender.output(DownloadsViewOutput::ItemAction {
                    media_item_id,
                    action,
                });
            }
            DownloadsViewMsg::GroupAction {
                media_item_ids,
                action,
            } => {
                for media_item_id in media_item_ids {
                    let _ = sender.output(DownloadsViewOutput::ItemAction {
                        media_item_id,
                        action,
                    });
                }
            }
        }
    }
}

/// Borrowed fields needed to render one show as a single poster card.
struct GroupCardInfo<'a> {
    group_id: &'a str,
    title: &'a str,
    status: GroupStatus,
    done: usize,
    total: usize,
    episodes: &'a [DownloadItemView],
}

impl DownloadsView {
    /// Update the banner and rebuild the listbox from the current state.
    fn rebuild(&mut self, sender: &ComponentSender<Self>) {
        // Banner: disk-full takes precedence over the over-budget warning.
        if self.disk_full {
            self.banner
                .set_title("Downloads paused — the disk is full. Free space to resume.");
            self.banner.set_revealed(true);
        } else if self.over_budget_warning {
            self.banner.set_title(
                "Over the download budget. Only unwatched downloads remain — none were deleted.",
            );
            self.banner.set_revealed(true);
        } else {
            self.banner.set_revealed(false);
        }

        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }
        // Widget handles point at the cards we just dropped; the texture cache
        // stays so reloaded posters appear instantly.
        self.row_widgets.clear();

        let rows = derive_rows(&self.downloads, &self.groups);
        if rows.is_empty() {
            self.grid.append(&empty_card());
            return;
        }
        for row in rows {
            let card = match row {
                DownloadRow::Standalone(item) => self.item_card(&item, sender),
                DownloadRow::Group {
                    group_id,
                    title,
                    status,
                    done,
                    total,
                    episodes,
                } => self.group_card(
                    GroupCardInfo {
                        group_id: &group_id,
                        title: &title,
                        status,
                        done,
                        total,
                        episodes: &episodes,
                    },
                    sender,
                ),
            };
            self.grid.append(&card);
        }
    }

    /// Update one row's progress bar + status line in place, avoiding a full
    /// listbox rebuild on every transfer tick.
    fn apply_progress(&self, media_item_id: &str, downloaded: u64, total: Option<u64>) {
        let Some(rw) = self.row_widgets.get(media_item_id) else {
            return;
        };
        if let Some(bar) = rw.progress_bar.as_ref() {
            match total.filter(|t| *t > 0) {
                Some(t) => {
                    bar.set_fraction((downloaded as f64 / t as f64).clamp(0.0, 1.0));
                    bar.set_visible(true);
                }
                // Total unknown: the byte count in the status line carries the
                // signal; a fraction-less bar would be misleading.
                None => bar.set_visible(false),
            }
        }
        if let Some(lbl) = rw.status_label.as_ref() {
            lbl.set_label(&downloading_status_text(
                downloaded as i64,
                total.map(|t| t as i64),
            ));
        }
    }

    /// Resolve `poster_path` to an artwork URL and apply a cached texture or
    /// kick off a background fetch. Returns the URL the poster was requested at
    /// (for later [`DownloadsViewMsg::ArtworkReady`] matching).
    fn load_poster(
        &self,
        picture: &gtk::Picture,
        placeholder: &gtk::Image,
        poster_path: Option<&str>,
        sender: &ComponentSender<Self>,
    ) -> Option<String> {
        let (Some(path), Some(source)) = (poster_path, self.source.as_ref()) else {
            placeholder.set_visible(true);
            return None;
        };
        let url = source.artwork_url(path, POSTER_REQ_W, POSTER_REQ_H);
        if let Some(texture) = self.texture_cache.get(&url) {
            picture.set_paintable(Some(texture));
            placeholder.set_visible(false);
            return Some(url);
        }
        placeholder.set_visible(true);
        if let Some(cache) = self.artwork_cache.as_ref() {
            let cache = Arc::clone(cache);
            let fetch_url = url.clone();
            let input = sender.input_sender().clone();
            gtk::glib::spawn_future_local(async move {
                if let Ok(path) = cache.get_or_download(&fetch_url).await
                    && let Ok(texture) = gtk::gdk::Texture::from_filename(&path)
                {
                    let _ = input.send(DownloadsViewMsg::ArtworkReady {
                        url: fetch_url,
                        texture,
                    });
                }
            });
        }
        Some(url)
    }

    /// A single download as a poster card: poster artwork with the status and
    /// progress overlaid at the bottom and the per-item action buttons revealed
    /// over the poster on hover. The title sits beneath the poster.
    fn item_card(&mut self, item: &DownloadItemView, sender: &ComponentSender<Self>) -> gtk::Box {
        let card = build_poster_card(&item_label(item));
        let poster_url = self.load_poster(
            &card.picture,
            &card.placeholder,
            item.poster_path.as_deref(),
            sender,
        );

        card.status.set_label(&status_text(item));
        if let Some(frac) = item.progress {
            card.progress.set_fraction(frac);
        }
        card.progress.set_visible(
            item.progress.is_some()
                && matches!(
                    item.state,
                    DownloadState::Downloading | DownloadState::Paused
                ),
        );

        for (icon, tooltip, action) in actions_for_state(item.state) {
            card.actions.append(&action_button(
                &item.media_item_id,
                icon,
                tooltip,
                action,
                sender,
            ));
        }

        // Click the poster (anywhere but the action buttons) to open its detail.
        attach_open_gesture(&card.container, &item.media_item_id, sender);

        self.row_widgets.insert(
            item.media_item_id.clone(),
            RowWidgets {
                picture: card.picture,
                placeholder: card.placeholder,
                poster_url,
                progress_bar: Some(card.progress),
                status_label: Some(card.status),
            },
        );

        card.container
    }

    /// A whole show as a single poster card: series artwork with the aggregate
    /// "6/10 episodes" status overlaid and retry-failed / delete-all actions.
    fn group_card(&mut self, info: GroupCardInfo<'_>, sender: &ComponentSender<Self>) -> gtk::Box {
        let GroupCardInfo {
            group_id,
            title,
            status,
            done,
            total,
            episodes,
        } = info;
        let card = build_poster_card(title);

        // Poster comes from any member (episodes fall back to the series poster).
        let poster_path = episodes.iter().find_map(|e| e.poster_path.clone());
        let poster_url = self.load_poster(
            &card.picture,
            &card.placeholder,
            poster_path.as_deref(),
            sender,
        );

        card.status
            .set_label(&group_status_text(status, done, total));
        // While episodes are still arriving, show how far the show has gotten.
        if status == GroupStatus::Downloading && total > 0 {
            card.progress.set_fraction(done as f64 / total as f64);
            card.progress.set_visible(true);
        }

        // Retry-all when any member failed.
        let retry_targets = group_retry_targets(episodes);
        if !retry_targets.is_empty() {
            let btn = gtk::Button::builder()
                .icon_name("view-refresh-symbolic")
                .tooltip_text("Retry failed episodes")
                .css_classes(["flat", "circular"])
                .build();
            let s = sender.input_sender().clone();
            btn.connect_clicked(move |_| {
                let _ = s.send(DownloadsViewMsg::GroupAction {
                    media_item_ids: retry_targets.clone(),
                    action: DownloadItemAction::Retry,
                });
            });
            card.actions.append(&btn);
        }

        // Delete the whole show's members.
        let all_ids: Vec<String> = episodes.iter().map(|e| e.media_item_id.clone()).collect();
        let del = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Delete all")
            .css_classes(["flat", "circular"])
            .build();
        let s = sender.input_sender().clone();
        del.connect_clicked(move |_| {
            let _ = s.send(DownloadsViewMsg::GroupAction {
                media_item_ids: all_ids.clone(),
                action: DownloadItemAction::Delete,
            });
        });
        card.actions.append(&del);

        // Click the show poster to open its detail. Episodes are non-empty by
        // construction; the app resolves the episode id up to its parent show.
        if let Some(ep) = episodes.first() {
            attach_open_gesture(&card.container, &ep.media_item_id, sender);
        }

        self.row_widgets.insert(
            group_id.to_string(),
            RowWidgets {
                picture: card.picture,
                placeholder: card.placeholder,
                poster_url,
                progress_bar: None,
                status_label: Some(card.status),
            },
        );

        card.container
    }
}

/// Attach a click gesture that opens the card's detail page. Lives on the card
/// container, so a click anywhere on the poster or title navigates; the action
/// buttons sit in front and claim their own clicks, so they never navigate.
fn attach_open_gesture(
    widget: &impl IsA<gtk::Widget>,
    media_item_id: &str,
    sender: &ComponentSender<DownloadsView>,
) {
    let click = gtk::GestureClick::new();
    let out = sender.output_sender().clone();
    let id = media_item_id.to_string();
    click.connect_released(move |gesture, _n_press, _x, _y| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        let _ = out.send(DownloadsViewOutput::OpenDetail(id.clone()));
    });
    widget.add_controller(click);
}

/// The action buttons appropriate for a download state.
fn actions_for_state(
    state: DownloadState,
) -> Vec<(&'static str, &'static str, DownloadItemAction)> {
    match state {
        DownloadState::Queued | DownloadState::Downloading => vec![
            (
                "media-playback-pause-symbolic",
                "Pause",
                DownloadItemAction::Pause,
            ),
            (
                "process-stop-symbolic",
                "Cancel",
                DownloadItemAction::Cancel,
            ),
        ],
        DownloadState::Paused => vec![
            (
                "media-playback-start-symbolic",
                "Resume",
                DownloadItemAction::Resume,
            ),
            (
                "process-stop-symbolic",
                "Cancel",
                DownloadItemAction::Cancel,
            ),
        ],
        DownloadState::Failed => vec![
            ("view-refresh-symbolic", "Retry", DownloadItemAction::Retry),
            ("user-trash-symbolic", "Remove", DownloadItemAction::Delete),
        ],
        DownloadState::Completed => {
            vec![("user-trash-symbolic", "Delete", DownloadItemAction::Delete)]
        }
        DownloadState::Removed | DownloadState::Pruned => Vec::new(),
    }
}

fn action_button(
    media_item_id: &str,
    icon: &str,
    tooltip: &str,
    action: DownloadItemAction,
    sender: &ComponentSender<DownloadsView>,
) -> gtk::Button {
    let btn = gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .css_classes(["flat", "circular"])
        .valign(gtk::Align::Center)
        .build();
    let s = sender.input_sender().clone();
    let id = media_item_id.to_string();
    btn.connect_clicked(move |_| {
        let _ = s.send(DownloadsViewMsg::Action {
            media_item_id: id.clone(),
            action,
        });
    });
    btn
}

fn empty_card() -> gtk::Label {
    let label = gtk::Label::builder()
        .label("No downloads yet. Download a movie or show to watch offline.")
        .margin_top(24)
        .margin_bottom(24)
        .build();
    label.add_css_class("dim-label");
    label
}

/// Widget handles for a freshly built poster card that the caller fills in.
struct PosterCard {
    /// The full card (poster frame + title) appended to the grid.
    container: gtk::Box,
    picture: gtk::Picture,
    placeholder: gtk::Image,
    /// Bottom-overlaid status line (e.g. "Downloading — 300 MB · 30%").
    status: gtk::Label,
    /// Bottom-edge progress bar; hidden until there's a fraction to show.
    progress: gtk::ProgressBar,
    /// Top-right action button tray, revealed over the poster on hover.
    actions: gtk::Box,
}

/// Build a library-style poster card: a rounded `media-card-frame` wrapping the
/// poster, a bottom scrim with the status line + progress bar overlaid, a
/// hover-revealed action tray, and the title beneath. The caller fills in the
/// poster, status text, progress, and action buttons.
fn build_poster_card(title: &str) -> PosterCard {
    let picture = gtk::Picture::builder()
        .content_fit(gtk::ContentFit::Cover)
        .width_request(CARD_W)
        .height_request(CARD_H)
        .css_classes(["media-card-poster"])
        .build();

    let placeholder = gtk::Image::builder()
        .icon_name("video-x-generic-symbolic")
        .pixel_size(48)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .css_classes(["media-card-placeholder-icon"])
        .build();

    // Bottom gradient scrim so the overlaid status text stays readable.
    let scrim = gtk::Box::builder()
        .halign(gtk::Align::Fill)
        .valign(gtk::Align::End)
        .height_request(64)
        .css_classes(["media-card-scrim"])
        .build();

    let status = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .valign(gtk::Align::End)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .margin_start(10)
        .margin_end(10)
        .margin_bottom(12)
        .css_classes(["download-status"])
        .build();

    let progress = gtk::ProgressBar::builder()
        .halign(gtk::Align::Fill)
        .valign(gtk::Align::End)
        .css_classes(["watch-progress"])
        .visible(false)
        .build();

    let actions = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(4)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Start)
        .margin_top(6)
        .margin_end(6)
        .css_classes(["download-actions"])
        .build();

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&picture));
    overlay.add_overlay(&scrim);
    overlay.add_overlay(&placeholder);
    overlay.add_overlay(&status);
    overlay.add_overlay(&progress);
    overlay.add_overlay(&actions);

    let frame = gtk::Frame::builder()
        .css_classes(["media-card-frame"])
        .child(&overlay)
        .build();

    let title_label = gtk::Label::builder()
        .label(title)
        .halign(gtk::Align::Start)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .max_width_chars(18)
        .lines(2)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .css_classes(["media-card-title"])
        .build();

    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .width_request(CARD_W)
        // Keep the card at its natural width, centered in its grid cell —
        // without this it fills (and stretches) the homogeneous FlowBox cell.
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Start)
        .css_classes(["media-card"])
        .build();
    container.append(&frame);
    container.append(&title_label);

    PosterCard {
        container,
        picture,
        placeholder,
        status,
        progress,
        actions,
    }
}

/// "Movie" or "S1E2 · Episode title" label.
fn item_label(item: &DownloadItemView) -> String {
    match (item.season_number, item.episode_number) {
        (Some(s), Some(e)) => format!("S{s}E{e} · {}", item.title),
        _ => item.title.clone(),
    }
}

fn status_text(item: &DownloadItemView) -> String {
    match item.state {
        DownloadState::Queued => "Queued".to_string(),
        DownloadState::Downloading => downloading_status_text(item.byte_count, item.total_size),
        DownloadState::Paused => match item.progress {
            Some(f) => format!("Paused — {}%", (f * 100.0).round() as u32),
            None => "Paused".to_string(),
        },
        DownloadState::Completed => match item.total_size {
            Some(t) if t > 0 => format!("Downloaded — {}", format_size(t)),
            _ => "Downloaded".to_string(),
        },
        DownloadState::Failed => fail_text(item.fail_reason),
        DownloadState::Removed => "Removed".to_string(),
        DownloadState::Pruned => "Pruned".to_string(),
    }
}

/// "Downloading — 12 MB of 1.2 GB · 30%", or just the byte count when the total
/// is not yet known (no `Content-Length`).
fn downloading_status_text(downloaded: i64, total: Option<i64>) -> String {
    match total.filter(|t| *t > 0) {
        Some(t) => {
            let pct = ((downloaded as f64 / t as f64).clamp(0.0, 1.0) * 100.0).round() as u32;
            format!(
                "Downloading — {} of {} · {pct}%",
                format_size(downloaded),
                format_size(t)
            )
        }
        None => format!("Downloading — {}", format_size(downloaded)),
    }
}

/// Human-readable size using decimal units, matching the settings usage readout.
fn format_size(bytes: i64) -> String {
    const KB: f64 = 1_000.0;
    const MB: f64 = 1_000_000.0;
    const GB: f64 = 1_000_000_000.0;
    let b = bytes.max(0) as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{b:.0} B")
    }
}

fn fail_text(reason: Option<FailReason>) -> String {
    match reason {
        Some(FailReason::Network) => "Failed — network error".to_string(),
        Some(FailReason::DiskFull) => "Failed — disk full".to_string(),
        Some(FailReason::AuthExpired) => "Failed — sign-in expired".to_string(),
        Some(FailReason::SourceFileChanged) => "Failed — source file changed".to_string(),
        Some(FailReason::FileMissing) => "Failed — file missing".to_string(),
        None => "Failed".to_string(),
    }
}

fn group_status_text(status: GroupStatus, done: usize, total: usize) -> String {
    let base = format!("{done}/{total} episodes");
    match status {
        GroupStatus::Downloading => format!("{base} · downloading"),
        GroupStatus::PartiallyFailed => format!("{base} · some failed"),
        GroupStatus::Failed => format!("{base} · failed"),
        GroupStatus::Complete => format!("{base} · complete"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_status_text_reads_naturally() {
        assert_eq!(
            group_status_text(GroupStatus::Downloading, 6, 10),
            "6/10 episodes · downloading"
        );
        assert_eq!(
            group_status_text(GroupStatus::Complete, 10, 10),
            "10/10 episodes · complete"
        );
    }

    #[test]
    fn actions_match_state() {
        assert_eq!(actions_for_state(DownloadState::Completed).len(), 1);
        assert_eq!(actions_for_state(DownloadState::Downloading).len(), 2);
        assert!(actions_for_state(DownloadState::Removed).is_empty());
        // Paused offers Resume.
        assert!(
            actions_for_state(DownloadState::Paused)
                .iter()
                .any(|(_, _, a)| *a == DownloadItemAction::Resume)
        );
    }

    #[test]
    fn item_label_formats_episode() {
        let v = DownloadItemView {
            media_item_id: "e1".into(),
            title: "Pilot".into(),
            state: DownloadState::Queued,
            fail_reason: None,
            season_number: Some(2),
            episode_number: Some(5),
            progress: None,
            byte_count: 0,
            total_size: None,
            poster_path: None,
        };
        assert_eq!(item_label(&v), "S2E5 · Pilot");
    }

    fn downloading(byte_count: i64, total_size: Option<i64>) -> DownloadItemView {
        DownloadItemView {
            media_item_id: "m1".into(),
            title: "Dune".into(),
            state: DownloadState::Downloading,
            fail_reason: None,
            season_number: None,
            episode_number: None,
            progress: total_size
                .filter(|t| *t > 0)
                .map(|t| (byte_count as f64 / t as f64).clamp(0.0, 1.0)),
            byte_count,
            total_size,
            poster_path: None,
        }
    }

    #[test]
    fn format_size_scales_units() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(12_000), "12 KB");
        assert_eq!(format_size(12_000_000), "12 MB");
        assert_eq!(format_size(1_500_000_000), "1.5 GB");
        // Never negative.
        assert_eq!(format_size(-5), "0 B");
    }

    #[test]
    fn downloading_status_shows_size_and_percent() {
        assert_eq!(
            status_text(&downloading(300_000_000, Some(1_000_000_000))),
            "Downloading — 300 MB of 1.0 GB · 30%"
        );
    }

    #[test]
    fn downloading_status_without_total_shows_only_bytes() {
        assert_eq!(
            status_text(&downloading(45_000_000, None)),
            "Downloading — 45 MB"
        );
    }

    #[test]
    fn completed_status_shows_final_size() {
        let mut v = downloading(1_000_000_000, Some(1_000_000_000));
        v.state = DownloadState::Completed;
        assert_eq!(status_text(&v), "Downloaded — 1.0 GB");
    }
}
