//! The Downloads view: offline downloads as standalone movie rows and grouped
//! show rows (aggregate "6/10 episodes" + per-episode state), with per-item and
//! per-group actions. Renders entirely from persisted repo state, so it works
//! with no source reachable (R16). Row derivation is pure (see [`row`]); this
//! component only turns rows into widgets and relays user actions.

pub mod row;

use adw::prelude::*;
use relm4::prelude::*;

use crate::models::download::{Download, DownloadGroup, DownloadState, FailReason};
use crate::services::download::GroupStatus;

use row::{DownloadItemView, DownloadRow, derive_rows, group_retry_targets};

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
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum DownloadsViewMsg {
    /// Replace the rendered state with the latest repo snapshot + banner flags.
    SetDownloads {
        downloads: Vec<Download>,
        groups: Vec<DownloadGroup>,
        over_budget_warning: bool,
        disk_full: bool,
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
        let model = Self {
            downloads: Vec::new(),
            groups: Vec::new(),
            over_budget_warning: false,
            disk_full: false,
            listbox: widgets.listbox.clone(),
            banner: widgets.banner.clone(),
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
    fn rebuild(&self, sender: &ComponentSender<Self>) {
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

        let rows = derive_rows(&self.downloads, &self.groups);
        if rows.is_empty() {
            self.listbox.append(&empty_row());
            return;
        }
        for row in rows {
            match row {
                DownloadRow::Standalone(item) => {
                    self.listbox.append(&self.item_row(&item, false, sender));
                }
                DownloadRow::Group {
                    title,
                    status,
                    done,
                    total,
                    episodes,
                    ..
                } => {
                    self.listbox.append(
                        &self.group_header_row(&title, status, done, total, &episodes, sender),
                    );
                    for ep in &episodes {
                        self.listbox.append(&self.item_row(ep, true, sender));
                    }
                }
            }
        }
    }

    /// A single download row: poster-less title + status + progress + actions.
    fn item_row(
        &self,
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
            .margin_start(if nested { 32 } else { 12 })
            .margin_end(12)
            .build();

        let vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
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

        if item.state == DownloadState::Downloading
            && let Some(frac) = item.progress
        {
            let bar = gtk::ProgressBar::builder().fraction(frac).build();
            vbox.append(&bar);
        }
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

        row.set_child(Some(&hbox));
        row
    }

    /// A group header row: title, aggregate status, and group actions.
    fn group_header_row(
        &self,
        title: &str,
        status: GroupStatus,
        done: usize,
        total: usize,
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

        let vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(2)
            .hexpand(true)
            .build();
        let title_lbl = gtk::Label::builder()
            .label(title)
            .halign(gtk::Align::Start)
            .build();
        title_lbl.add_css_class("heading");
        vbox.append(&title_lbl);

        let agg = gtk::Label::builder()
            .label(group_status_text(status, done, total))
            .halign(gtk::Align::Start)
            .build();
        agg.add_css_class("caption");
        agg.add_css_class("dim-label");
        vbox.append(&agg);
        hbox.append(&vbox);

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
        DownloadState::Downloading => match item.progress {
            Some(f) => format!("Downloading — {}%", (f * 100.0).round() as u32),
            None => "Downloading".to_string(),
        },
        DownloadState::Paused => "Paused".to_string(),
        DownloadState::Completed => "Downloaded".to_string(),
        DownloadState::Failed => fail_text(item.fail_reason),
        DownloadState::Removed => "Removed".to_string(),
        DownloadState::Pruned => "Pruned".to_string(),
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
            poster_path: None,
        };
        assert_eq!(item_label(&v), "S2E5 · Pilot");
    }
}
