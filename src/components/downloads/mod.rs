//! The Downloads view: offline downloads as standalone movie rows and grouped
//! show rows (aggregate "6/10 episodes" + per-episode state), with per-item and
//! per-group actions. Renders entirely from persisted repo state, so it works
//! with no source reachable (R16). Row derivation is pure (see [`row`]); this
//! component only turns rows into widgets and relays user actions.

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

/// Displayed poster size in a download row (2:3 movie poster).
const POSTER_W: i32 = 56;
const POSTER_H: i32 = 84;
/// Requested transcode size (2× the display size for crisp HiDPI rendering).
const POSTER_REQ_W: u32 = 112;
const POSTER_REQ_H: u32 = 168;

/// GTK widget handles for a rendered row, kept so progress ticks and arriving
/// artwork can update the row in place without rebuilding the whole listbox.
struct RowWidgets {
    picture: gtk::Picture,
    placeholder: gtk::Image,
    /// The artwork URL this row's poster was requested at, used to match an
    /// arriving [`DownloadsViewMsg::ArtworkReady`].
    poster_url: Option<String>,
    /// Present for item rows (movies/episodes); `None` for group headers.
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
    listbox: gtk::ListBox,
    banner: adw::Banner,
    /// Active source, used to resolve a poster path into an artwork URL.
    source: Option<Arc<dyn MediaSource>>,
    /// Shared artwork cache for downloading/caching posters off the main loop.
    artwork_cache: Option<Arc<ArtworkCache>>,
    /// In-memory texture cache keyed by artwork URL, so a rebuild reuses already
    /// loaded posters instead of re-fetching (mirrors the library grid).
    texture_cache: HashMap<String, gtk::gdk::Texture>,
    /// Per-row widget handles for in-place progress/poster updates. Keyed by
    /// `media_item_id` for item rows and by `group_id` for group headers.
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

                #[name = "listbox"]
                gtk::ListBox {
                    add_css_class: "boxed-list",
                    set_selection_mode: gtk::SelectionMode::None,
                    set_margin_top: 12,
                    set_margin_bottom: 12,
                    set_margin_start: 12,
                    set_margin_end: 12,
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
            listbox: widgets.listbox.clone(),
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

        while let Some(child) = self.listbox.first_child() {
            self.listbox.remove(&child);
        }
        // Widget handles point at the rows we just dropped; the texture cache
        // stays so reloaded posters appear instantly.
        self.row_widgets.clear();

        let rows = derive_rows(&self.downloads, &self.groups);
        if rows.is_empty() {
            self.listbox.append(&empty_row());
            return;
        }
        for row in rows {
            match row {
                DownloadRow::Standalone(item) => {
                    let widget = self.item_row(&item, false, sender);
                    self.listbox.append(&widget);
                }
                DownloadRow::Group {
                    group_id,
                    title,
                    status,
                    done,
                    total,
                    episodes,
                } => {
                    let agg = group_status_text(status, done, total);
                    let header = self.group_header_row(&group_id, &title, &agg, &episodes, sender);
                    self.listbox.append(&header);
                    for ep in &episodes {
                        let widget = self.item_row(ep, true, sender);
                        self.listbox.append(&widget);
                    }
                }
            }
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

    /// A single download row: poster + title + status + progress + actions.
    fn item_row(
        &mut self,
        item: &DownloadItemView,
        nested: bool,
        sender: &ComponentSender<Self>,
    ) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::builder().selectable(false).build();
        let hbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(if nested { 24 } else { 12 })
            .margin_end(12)
            .build();

        let (frame, picture, placeholder) = build_row_poster();
        let poster_url =
            self.load_poster(&picture, &placeholder, item.poster_path.as_deref(), sender);
        hbox.append(&frame);

        let vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .valign(gtk::Align::Center)
            .hexpand(true)
            .build();
        let title = gtk::Label::builder()
            .label(item_label(item))
            .halign(gtk::Align::Start)
            .build();
        title.add_css_class("body");
        vbox.append(&title);

        let status = gtk::Label::builder()
            .label(status_text(item))
            .halign(gtk::Align::Start)
            .build();
        status.add_css_class("caption");
        status.add_css_class("dim-label");
        vbox.append(&status);

        // Always present (so progress ticks can reveal it); shown only while a
        // transfer is in flight or paused with a known fraction.
        let bar = gtk::ProgressBar::builder().build();
        if let Some(frac) = item.progress {
            bar.set_fraction(frac);
        }
        bar.set_visible(
            item.progress.is_some()
                && matches!(
                    item.state,
                    DownloadState::Downloading | DownloadState::Paused
                ),
        );
        vbox.append(&bar);
        hbox.append(&vbox);

        for (icon, tooltip, action) in actions_for_state(item.state) {
            hbox.append(&action_button(
                &item.media_item_id,
                icon,
                tooltip,
                action,
                sender,
            ));
        }

        self.row_widgets.insert(
            item.media_item_id.clone(),
            RowWidgets {
                picture,
                placeholder,
                poster_url,
                progress_bar: Some(bar),
                status_label: Some(status),
            },
        );

        row.set_child(Some(&hbox));
        row
    }

    /// A group header row: poster, title, aggregate status, and group actions.
    fn group_header_row(
        &mut self,
        group_id: &str,
        title: &str,
        agg_status: &str,
        episodes: &[DownloadItemView],
        sender: &ComponentSender<Self>,
    ) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::builder().selectable(false).build();
        let hbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .margin_top(10)
            .margin_bottom(6)
            .margin_start(12)
            .margin_end(12)
            .build();

        // Group poster comes from any member (episodes fall back to the series
        // poster), so the show artwork shows even before episodes are expanded.
        let (frame, picture, placeholder) = build_row_poster();
        let poster_path = episodes.iter().find_map(|e| e.poster_path.clone());
        let poster_url = self.load_poster(&picture, &placeholder, poster_path.as_deref(), sender);
        hbox.append(&frame);

        let vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .valign(gtk::Align::Center)
            .hexpand(true)
            .build();
        let title_lbl = gtk::Label::builder()
            .label(title)
            .halign(gtk::Align::Start)
            .build();
        title_lbl.add_css_class("heading");
        vbox.append(&title_lbl);

        let agg = gtk::Label::builder()
            .label(agg_status)
            .halign(gtk::Align::Start)
            .build();
        agg.add_css_class("caption");
        agg.add_css_class("dim-label");
        vbox.append(&agg);
        hbox.append(&vbox);

        self.row_widgets.insert(
            group_id.to_string(),
            RowWidgets {
                picture,
                placeholder,
                poster_url,
                progress_bar: None,
                status_label: None,
            },
        );

        // Retry-all when any member failed.
        let retry_targets = group_retry_targets(episodes);
        if !retry_targets.is_empty() {
            let btn = gtk::Button::builder()
                .icon_name("view-refresh-symbolic")
                .tooltip_text("Retry failed episodes")
                .css_classes(["flat"])
                .build();
            let s = sender.input_sender().clone();
            btn.connect_clicked(move |_| {
                let _ = s.send(DownloadsViewMsg::GroupAction {
                    media_item_ids: retry_targets.clone(),
                    action: DownloadItemAction::Retry,
                });
            });
            hbox.append(&btn);
        }

        // Delete the whole group's members.
        let all_ids: Vec<String> = episodes.iter().map(|e| e.media_item_id.clone()).collect();
        let del = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Delete all")
            .css_classes(["flat"])
            .build();
        let s = sender.input_sender().clone();
        del.connect_clicked(move |_| {
            let _ = s.send(DownloadsViewMsg::GroupAction {
                media_item_ids: all_ids.clone(),
                action: DownloadItemAction::Delete,
            });
        });
        hbox.append(&del);

        row.set_child(Some(&hbox));
        row
    }
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
        .css_classes(["flat"])
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

fn empty_row() -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::builder().selectable(false).build();
    let label = gtk::Label::builder()
        .label("No downloads yet. Download a movie or show to watch offline.")
        .margin_top(24)
        .margin_bottom(24)
        .build();
    label.add_css_class("dim-label");
    row.set_child(Some(&label));
    row
}

/// A small rounded poster (picture + centered placeholder icon) for a row.
fn build_row_poster() -> (gtk::Frame, gtk::Picture, gtk::Image) {
    let picture = gtk::Picture::builder()
        .content_fit(gtk::ContentFit::Cover)
        .width_request(POSTER_W)
        .height_request(POSTER_H)
        .build();
    let placeholder = gtk::Image::builder()
        .icon_name("video-x-generic-symbolic")
        .pixel_size(20)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .css_classes(["dim-label"])
        .build();
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&picture));
    overlay.add_overlay(&placeholder);
    let frame = gtk::Frame::builder()
        .css_classes(["download-poster-frame"])
        .valign(gtk::Align::Center)
        .child(&overlay)
        .build();
    (frame, picture, placeholder)
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
