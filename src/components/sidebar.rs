use std::collections::HashSet;

use gtk::prelude::*;
use relm4::prelude::*;

use crate::models::library::{LibrarySection, LibraryType};

/// Reserved section key for the per-source Collections entry's visibility.
/// A real Plex section key is numeric, so this never collides.
pub const COLLECTIONS_KEY: &str = "__collections__";

/// What the sidebar currently has selected, for highlight sync. `Library`
/// carries the composite `{source_id}:{key}` so two same-keyed libraries on
/// different servers never highlight together.
#[derive(Debug, Clone, PartialEq)]
pub enum SidebarSelection {
    Home,
    /// `(source_id, section_key)` — composite so per-source highlight is isolated.
    Library(String, String),
    /// `source_id` of the source whose Collections entry is selected.
    Collections(String),
    /// The top-level offline Downloads destination.
    Downloads,
}

/// One connected source and its (possibly not-yet-loaded) libraries. The
/// sidebar holds these in insertion order; `derive_rows` renders one group per
/// entry.
#[derive(Debug, Clone)]
pub struct SourceGroup {
    pub name: String,
    pub source_type: String,
    pub source_id: String,
    pub libraries: Vec<LibrarySection>,
    /// True once a libraries fetch completed (success or empty), so the group
    /// shows a terminal state instead of spinning on "Loading" forever.
    pub loaded: bool,
}

/// Pure description of one rendered sidebar row. Derived from state by
/// [`derive_rows`]; the GTK build consumes it. Free of widgets so the
/// information architecture is unit-testable without a display.
#[derive(Debug, Clone, PartialEq)]
pub enum SidebarRow {
    Home,
    /// Offline downloads — a top-level destination, independent of any source
    /// (renders even with zero sources connected).
    Downloads,
    /// The source node header. Carries the source identity for the edit/remove
    /// affordances.
    SourceHeader {
        name: String,
        source_type: String,
        source_id: String,
    },
    /// Placeholder shown under a source header before its libraries resolve.
    Loading,
    /// Shown when a source loaded but reported no (supported) libraries — a
    /// terminal state, distinct from `Loading`, so the sidebar never spins
    /// forever if a fetch comes back empty.
    NoLibraries,
    /// Shown in normal mode when a source has libraries but all are hidden,
    /// so the source node never collapses to nothing.
    AllHidden,
    Library {
        key: String,
        title: String,
        library_type: LibraryType,
        visible: bool,
        source_type: String,
        source_id: String,
    },
    Collections {
        visible: bool,
        source_type: String,
        source_id: String,
    },
}

/// Derive the ordered list of sidebar rows from current state. Emits a single
/// `Home` row at the top, then one group per source (in insertion order): a
/// `SourceHeader` followed by that source's rows. In normal mode only visible
/// libraries (and Collections) appear; in edit mode every library appears with
/// its visibility flag so a toggle can be rendered. Visibility is keyed on the
/// **composite** `{source_type}:{source_id}:{section_key}`, so two same-named
/// libraries on different servers hide independently.
pub fn derive_rows(
    sources: &[SourceGroup],
    hidden: &HashSet<String>,
    edit_mode: bool,
) -> Vec<SidebarRow> {
    let mut rows = vec![SidebarRow::Home, SidebarRow::Downloads];

    for group in sources {
        rows.push(SidebarRow::SourceHeader {
            name: group.name.clone(),
            source_type: group.source_type.clone(),
            source_id: group.source_id.clone(),
        });

        // Source connected but its libraries have not been fetched yet.
        if !group.loaded {
            rows.push(SidebarRow::Loading);
            continue;
        }

        // Loaded, but the source reported no supported libraries.
        if group.libraries.is_empty() {
            rows.push(SidebarRow::NoLibraries);
            continue;
        }

        let is_hidden = |key: &str| {
            hidden.contains(&LibrarySection::visibility_key_for(
                &group.source_type,
                &group.source_id,
                key,
            ))
        };

        let mut any_visible = false;
        for lib in &group.libraries {
            let visible = !is_hidden(&lib.key);
            any_visible |= visible;
            if edit_mode || visible {
                rows.push(SidebarRow::Library {
                    key: lib.key.clone(),
                    title: lib.title.clone(),
                    library_type: lib.library_type,
                    visible,
                    source_type: group.source_type.clone(),
                    source_id: group.source_id.clone(),
                });
            }
        }

        let collections_visible = !is_hidden(COLLECTIONS_KEY);
        any_visible |= collections_visible;
        if edit_mode || collections_visible {
            rows.push(SidebarRow::Collections {
                visible: collections_visible,
                source_type: group.source_type.clone(),
                source_id: group.source_id.clone(),
            });
        }

        if !edit_mode && !any_visible {
            rows.push(SidebarRow::AllHidden);
        }
    }

    rows
}

/// Navigation action a row triggers when activated. `None` for non-actionable
/// rows (header, loading, all-hidden). Indexed in parallel with the listbox.
#[derive(Debug, Clone)]
enum RowAction {
    None,
    Home,
    /// A library plus the identity of the source that owns it.
    Library {
        section: LibrarySection,
        source_type: String,
        source_id: String,
    },
    /// The Collections entry plus its owning source identity.
    Collections {
        source_type: String,
        source_id: String,
    },
    Downloads,
}

pub struct Sidebar {
    /// Connected sources in insertion order; each renders as its own group.
    sources: Vec<SourceGroup>,
    hidden: HashSet<String>,
    edit_mode: bool,
    selected: SidebarSelection,
    listbox: gtk::ListBox,
    /// Handler for `row-selected`, blocked around programmatic selection so a
    /// rebuild or external highlight never emits a spurious navigation.
    row_selected_handler: gtk::glib::SignalHandlerId,
    /// Navigation action per listbox row, rebuilt alongside the rows.
    row_actions: Vec<RowAction>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum SidebarMsg {
    /// Upsert a connected source's display name + identity, preserving
    /// insertion order. Updates the name if `(type, id)` already exists, else
    /// pushes a new not-loaded group.
    SetSource {
        name: String,
        source_type: String,
        source_id: String,
    },
    /// Provide a source's libraries (fetched async by the app), matched by
    /// `source_id`. Ignored if no such group exists.
    SetLibraries {
        source_id: String,
        libraries: Vec<LibrarySection>,
    },
    /// Update the hidden-entry set (composite keys).
    SetVisibility(HashSet<String>),
    /// A listbox row was selected (by index into `row_actions`).
    RowSelected(usize),
    /// Toggle a library/collections entry's visibility (from its edit switch).
    /// Carries the owning source so the composite key is built correctly.
    ToggleEntry {
        key: String,
        source_type: String,
        source_id: String,
        visible: bool,
    },
    /// The remove (trash) button on a source header was clicked. Emits a
    /// `RemoveSource` output for the app to confirm; the group is NOT dropped
    /// until the app sends `DropSource` after confirmation.
    RemoveSourceClicked {
        source_type: String,
        source_id: String,
    },
    /// Drop a source group from the sidebar — sent by the app once the user has
    /// confirmed removal (the destructive media/watch eviction is the app's).
    DropSource {
        source_type: String,
        source_id: String,
    },
    /// Enter/leave edit mode (global across all sources).
    ToggleEditMode,
    /// Select a row programmatically (e.g. on startup / back navigation)
    /// without emitting a navigation output.
    SelectExternal(SidebarSelection),
}

#[derive(Debug)]
pub enum SidebarOutput {
    NavigateHome,
    /// Navigate to a library, carrying the owning source's identity so the app
    /// can scope the LibraryView list to the right source.
    Navigate {
        section: LibrarySection,
        source_type: String,
        source_id: String,
    },
    /// Show the Collections of a specific source.
    ShowCollections {
        source_type: String,
        source_id: String,
    },
    /// Show the top-level offline Downloads destination.
    ShowDownloads,
    /// Persist a visibility change for a library (or Collections) entry. `key`
    /// is the **composite** `{source_type}:{source_id}:{section_key}`.
    SetLibraryVisible {
        key: String,
        visible: bool,
    },
    /// Edit mode was exited; the app re-evaluates the current view (a hidden
    /// current library redirects to Home).
    EditModeExited,
    /// The user removed a source (optimistic local removal already applied).
    RemoveSource {
        source_type: String,
        source_id: String,
    },
}

#[relm4::component(pub)]
impl SimpleComponent for Sidebar {
    type Init = ();
    type Input = SidebarMsg;
    type Output = SidebarOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_width_request: 240,

            adw::HeaderBar {
                set_show_end_title_buttons: false,
                #[wrap(Some)]
                set_title_widget = &gtk::Label {
                    set_label: "Reel",
                    add_css_class: "title",
                },
            },

            gtk::ScrolledWindow {
                set_vexpand: true,
                set_hscrollbar_policy: gtk::PolicyType::Never,

                #[name = "listbox"]
                gtk::ListBox {
                    add_css_class: "navigation-sidebar",
                    set_selection_mode: gtk::SelectionMode::Single,
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

        // Connect row-selected manually so we hold the handler id and can block
        // it around programmatic selection.
        let input = sender.input_sender().clone();
        let row_selected_handler = widgets.listbox.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                let _ = input.send(SidebarMsg::RowSelected(row.index() as usize));
            }
        });

        let mut model = Self {
            sources: Vec::new(),
            hidden: HashSet::new(),
            edit_mode: false,
            selected: SidebarSelection::Home,
            listbox: widgets.listbox.clone(),
            row_selected_handler,
            row_actions: Vec::new(),
        };

        model.rebuild(&sender);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            SidebarMsg::SetSource {
                name,
                source_type,
                source_id,
            } => {
                if let Some(existing) = self
                    .sources
                    .iter_mut()
                    .find(|g| g.source_type == source_type && g.source_id == source_id)
                {
                    existing.name = name;
                } else {
                    self.sources.push(SourceGroup {
                        name,
                        source_type,
                        source_id,
                        libraries: Vec::new(),
                        loaded: false,
                    });
                }
                self.rebuild(&sender);
            }
            SidebarMsg::SetLibraries {
                source_id,
                libraries,
            } => {
                if let Some(group) = self.sources.iter_mut().find(|g| g.source_id == source_id) {
                    group.libraries = libraries;
                    group.loaded = true;
                    self.rebuild(&sender);
                }
            }
            SidebarMsg::SetVisibility(hidden) => {
                self.hidden = hidden;
                self.rebuild(&sender);
            }
            SidebarMsg::ToggleEditMode => {
                self.edit_mode = !self.edit_mode;
                self.rebuild(&sender);
                if !self.edit_mode {
                    let _ = sender.output(SidebarOutput::EditModeExited);
                }
            }
            SidebarMsg::ToggleEntry {
                key,
                source_type,
                source_id,
                visible,
            } => {
                let vis_key = LibrarySection::visibility_key_for(&source_type, &source_id, &key);
                if visible {
                    self.hidden.remove(&vis_key);
                } else {
                    self.hidden.insert(vis_key.clone());
                }
                // The switch already reflects the new state; no rebuild while in
                // edit mode (rows stay put). Persistence + re-filter happen in
                // the app via the output below. Emit the COMPOSITE key so every
                // consumer (retain_visible_items, redirect check, derive_rows)
                // keys on the same string.
                let _ = sender.output(SidebarOutput::SetLibraryVisible {
                    key: vis_key,
                    visible,
                });
            }
            SidebarMsg::RemoveSourceClicked {
                source_type,
                source_id,
            } => {
                // Do NOT remove the group here — removal evicts the source's
                // media + watch history, so the app must confirm with the user
                // first. The group is dropped only via `DropSource` once the
                // app's confirmation dialog is accepted.
                let _ = sender.output(SidebarOutput::RemoveSource {
                    source_type,
                    source_id,
                });
            }
            SidebarMsg::DropSource {
                source_type,
                source_id,
            } => {
                self.sources
                    .retain(|g| !(g.source_type == source_type && g.source_id == source_id));
                self.rebuild(&sender);
            }
            SidebarMsg::RowSelected(index) => {
                self.handle_row_selected(index, &sender);
            }
            SidebarMsg::SelectExternal(selection) => {
                self.selected = selection;
                self.listbox.block_signal(&self.row_selected_handler);
                self.select_current_row();
                self.listbox.unblock_signal(&self.row_selected_handler);
            }
        }
    }
}

impl Sidebar {
    /// Dispatch a real user row click to a navigation output. Programmatic
    /// selection is blocked at the signal, so this only fires for actionable
    /// rows.
    fn handle_row_selected(&mut self, index: usize, sender: &ComponentSender<Self>) {
        match self.row_actions.get(index) {
            Some(RowAction::Home) => {
                self.selected = SidebarSelection::Home;
                let _ = sender.output(SidebarOutput::NavigateHome);
            }
            Some(RowAction::Library {
                section,
                source_type,
                source_id,
            }) => {
                self.selected = SidebarSelection::Library(source_id.clone(), section.key.clone());
                let _ = sender.output(SidebarOutput::Navigate {
                    section: section.clone(),
                    source_type: source_type.clone(),
                    source_id: source_id.clone(),
                });
            }
            Some(RowAction::Collections {
                source_type,
                source_id,
            }) => {
                self.selected = SidebarSelection::Collections(source_id.clone());
                let _ = sender.output(SidebarOutput::ShowCollections {
                    source_type: source_type.clone(),
                    source_id: source_id.clone(),
                });
            }
            Some(RowAction::Downloads) => {
                self.selected = SidebarSelection::Downloads;
                let _ = sender.output(SidebarOutput::ShowDownloads);
            }
            Some(RowAction::None) | None => {}
        }
    }

    /// Clear and rebuild the listbox from current state, refreshing the parallel
    /// `row_actions`. The row-selected handler is blocked throughout so clearing
    /// and re-selecting can't emit spurious navigation.
    fn rebuild(&mut self, sender: &ComponentSender<Self>) {
        self.listbox.block_signal(&self.row_selected_handler);

        while let Some(child) = self.listbox.first_child() {
            self.listbox.remove(&child);
        }

        let rows = derive_rows(&self.sources, &self.hidden, self.edit_mode);

        let mut actions = Vec::with_capacity(rows.len());
        for row in &rows {
            let (widget, action) = self.build_row(row, sender);
            self.listbox.append(&widget);
            actions.push(action);
        }
        self.row_actions = actions;

        self.select_current_row();
        self.listbox.unblock_signal(&self.row_selected_handler);
    }

    /// Build the `ListBoxRow` for a derived row plus its navigation action.
    fn build_row(
        &self,
        row: &SidebarRow,
        sender: &ComponentSender<Self>,
    ) -> (gtk::ListBoxRow, RowAction) {
        match row {
            SidebarRow::Home => (nav_row("go-home-symbolic", "Home", None), RowAction::Home),
            SidebarRow::Downloads => (
                nav_row("folder-download-symbolic", "Downloads", None),
                RowAction::Downloads,
            ),
            SidebarRow::SourceHeader {
                name,
                source_type,
                source_id,
            } => (
                self.source_header_row(name, source_type, source_id, sender),
                RowAction::None,
            ),
            SidebarRow::Loading => {
                let r = gtk::ListBoxRow::builder().selectable(false).build();
                let b = indented_box();
                let spinner = gtk::Spinner::builder().spinning(true).build();
                b.append(&spinner);
                b.append(&dim_label("Loading libraries…"));
                r.set_child(Some(&b));
                (r, RowAction::None)
            }
            SidebarRow::NoLibraries => {
                let r = gtk::ListBoxRow::builder().selectable(false).build();
                let b = indented_box();
                b.append(&dim_label("No libraries found"));
                r.set_child(Some(&b));
                (r, RowAction::None)
            }
            SidebarRow::AllHidden => {
                let r = gtk::ListBoxRow::builder().selectable(false).build();
                let b = indented_box();
                b.append(&dim_label("All libraries hidden"));
                r.set_child(Some(&b));
                (r, RowAction::None)
            }
            SidebarRow::Library {
                key,
                title,
                library_type,
                visible,
                source_type,
                source_id,
            } => {
                let icon = match library_type {
                    LibraryType::Movie => "video-display-symbolic",
                    LibraryType::Show => "view-list-symbolic",
                };
                let switch = self
                    .edit_mode
                    .then(|| self.entry_switch(key, source_type, source_id, *visible, sender));
                let r = nav_row_indented(icon, title, switch);
                (
                    r,
                    RowAction::Library {
                        section: LibrarySection {
                            key: key.clone(),
                            title: title.clone(),
                            library_type: *library_type,
                            count: None,
                        },
                        source_type: source_type.clone(),
                        source_id: source_id.clone(),
                    },
                )
            }
            SidebarRow::Collections {
                visible,
                source_type,
                source_id,
            } => {
                let switch = self.edit_mode.then(|| {
                    self.entry_switch(COLLECTIONS_KEY, source_type, source_id, *visible, sender)
                });
                let r = nav_row_indented("view-grid-symbolic", "Collections", switch);
                (
                    r,
                    RowAction::Collections {
                        source_type: source_type.clone(),
                        source_id: source_id.clone(),
                    },
                )
            }
        }
    }

    /// The source node header row: source name, edit/Done toggle, and a remove
    /// (trash) button.
    fn source_header_row(
        &self,
        name: &str,
        source_type: &str,
        source_id: &str,
        sender: &ComponentSender<Self>,
    ) -> gtk::ListBoxRow {
        let r = gtk::ListBoxRow::builder().selectable(false).build();
        let b = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .margin_top(8)
            .margin_bottom(4)
            .margin_start(8)
            .margin_end(8)
            .build();

        let label = gtk::Label::builder()
            .label(name)
            .hexpand(true)
            .halign(gtk::Align::Start)
            .build();
        label.add_css_class("heading");
        b.append(&label);

        let (icon, tooltip) = if self.edit_mode {
            ("object-select-symbolic", "Done")
        } else {
            ("document-edit-symbolic", "Edit libraries")
        };
        let edit_btn = gtk::Button::builder()
            .icon_name(icon)
            .tooltip_text(tooltip)
            .build();
        edit_btn.add_css_class("flat");
        let s = sender.input_sender().clone();
        edit_btn.connect_clicked(move |_| {
            let _ = s.send(SidebarMsg::ToggleEditMode);
        });
        b.append(&edit_btn);

        let remove_btn = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Remove server")
            .build();
        remove_btn.add_css_class("flat");
        let s = sender.input_sender().clone();
        let st = source_type.to_string();
        let sid = source_id.to_string();
        remove_btn.connect_clicked(move |_| {
            let _ = s.send(SidebarMsg::RemoveSourceClicked {
                source_type: st.clone(),
                source_id: sid.clone(),
            });
        });
        b.append(&remove_btn);

        r.set_child(Some(&b));
        r
    }

    /// A visibility switch for a library/collections entry in edit mode.
    fn entry_switch(
        &self,
        key: &str,
        source_type: &str,
        source_id: &str,
        visible: bool,
        sender: &ComponentSender<Self>,
    ) -> gtk::Switch {
        let switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        // Set state before connecting so the initial value doesn't fire.
        switch.set_active(visible);
        let s = sender.input_sender().clone();
        let key = key.to_string();
        let st = source_type.to_string();
        let sid = source_id.to_string();
        switch.connect_active_notify(move |sw| {
            let _ = s.send(SidebarMsg::ToggleEntry {
                key: key.clone(),
                source_type: st.clone(),
                source_id: sid.clone(),
                visible: sw.is_active(),
            });
        });
        switch
    }

    /// Select the listbox row matching `self.selected`. Callers must block the
    /// `row-selected` handler around this so it does not emit navigation.
    fn select_current_row(&self) {
        let target = self
            .row_actions
            .iter()
            .position(|a| match (&self.selected, a) {
                (SidebarSelection::Home, RowAction::Home) => true,
                (
                    SidebarSelection::Library(sid, k),
                    RowAction::Library {
                        section, source_id, ..
                    },
                ) => *sid == *source_id && *k == section.key,
                (SidebarSelection::Collections(sid), RowAction::Collections { source_id, .. }) => {
                    *sid == *source_id
                }
                (SidebarSelection::Downloads, RowAction::Downloads) => true,
                _ => false,
            });
        if let Some(idx) = target {
            if let Some(row) = self.listbox.row_at_index(idx as i32) {
                self.listbox.select_row(Some(&row));
            }
        } else {
            self.listbox.unselect_all();
        }
    }
}

/// A standard navigation row: icon + label, optionally trailing widget.
fn nav_row(icon: &str, label: &str, trailing: Option<gtk::Switch>) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::builder().selectable(true).build();
    let b = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
        .build();
    b.append(&gtk::Image::from_icon_name(icon));
    let lbl = gtk::Label::builder()
        .label(label)
        .hexpand(true)
        .halign(gtk::Align::Start)
        .build();
    b.append(&lbl);
    if let Some(sw) = trailing {
        b.append(&sw);
    }
    row.set_child(Some(&b));
    row
}

/// A navigation row nested under the source node (extra start indent).
fn nav_row_indented(icon: &str, label: &str, trailing: Option<gtk::Switch>) -> gtk::ListBoxRow {
    let row = nav_row(icon, label, trailing);
    if let Some(child) = row.child() {
        child.set_margin_start(24);
    }
    row
}

fn indented_box() -> gtk::Box {
    gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(24)
        .margin_end(8)
        .build()
}

fn dim_label(text: &str) -> gtk::Label {
    let l = gtk::Label::builder()
        .label(text)
        .halign(gtk::Align::Start)
        .build();
    l.add_css_class("dim-label");
    l.add_css_class("caption");
    l
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(key: &str, title: &str, lt: LibraryType) -> LibrarySection {
        LibrarySection {
            key: key.to_string(),
            title: title.to_string(),
            library_type: lt,
            count: None,
        }
    }

    fn hidden(keys: &[&str]) -> HashSet<String> {
        keys.iter().map(|s| s.to_string()).collect()
    }

    fn group(
        name: &str,
        source_type: &str,
        source_id: &str,
        libraries: Vec<LibrarySection>,
        loaded: bool,
    ) -> SourceGroup {
        SourceGroup {
            name: name.to_string(),
            source_type: source_type.to_string(),
            source_id: source_id.to_string(),
            libraries,
            loaded,
        }
    }

    #[test]
    fn no_sources_shows_home_and_downloads() {
        // Downloads is a top-level destination, present with zero sources (R1).
        let rows = derive_rows(&[], &HashSet::new(), false);
        assert_eq!(rows, vec![SidebarRow::Home, SidebarRow::Downloads]);
    }

    #[test]
    fn downloads_row_precedes_source_header() {
        let sources = vec![group(
            "S",
            "plex",
            "srv",
            vec![section("1", "Movies", LibraryType::Movie)],
            true,
        )];
        let rows = derive_rows(&sources, &HashSet::new(), false);
        let dl = rows
            .iter()
            .position(|r| matches!(r, SidebarRow::Downloads))
            .unwrap();
        let header = rows
            .iter()
            .position(|r| matches!(r, SidebarRow::SourceHeader { .. }))
            .unwrap();
        assert!(
            dl < header,
            "Downloads must render before the source header"
        );
    }

    #[test]
    fn not_loaded_shows_loading() {
        let sources = vec![group("My Plex", "plex", "srv", vec![], false)];
        let rows = derive_rows(&sources, &HashSet::new(), false);
        assert_eq!(
            rows,
            vec![
                SidebarRow::Home,
                SidebarRow::Downloads,
                SidebarRow::SourceHeader {
                    name: "My Plex".to_string(),
                    source_type: "plex".to_string(),
                    source_id: "srv".to_string(),
                },
                SidebarRow::Loading,
            ]
        );
    }

    #[test]
    fn derive_rows_empty_source_shows_header_only() {
        let sources = vec![group("My Plex", "plex", "srv", vec![], true)];
        let rows = derive_rows(&sources, &HashSet::new(), false);
        assert_eq!(
            rows,
            vec![
                SidebarRow::Home,
                SidebarRow::Downloads,
                SidebarRow::SourceHeader {
                    name: "My Plex".to_string(),
                    source_type: "plex".to_string(),
                    source_id: "srv".to_string(),
                },
                SidebarRow::NoLibraries,
            ]
        );
    }

    #[test]
    fn normal_mode_lists_visible_libraries_and_collections() {
        let libs = vec![
            section("1", "Movies", LibraryType::Movie),
            section("2", "4K Movies", LibraryType::Movie),
        ];
        let sources = vec![group("S", "plex", "srv", libs, true)];
        let rows = derive_rows(&sources, &HashSet::new(), false);
        // Home, Downloads, header, 2 libraries, collections.
        assert_eq!(rows.len(), 6);
        assert!(matches!(rows[3], SidebarRow::Library { ref title, .. } if title == "Movies"));
        assert!(matches!(
            rows[5],
            SidebarRow::Collections { visible: true, .. }
        ));
    }

    #[test]
    fn derive_rows_two_sources_emits_two_groups() {
        let plex = group(
            "Plex",
            "plex",
            "px",
            vec![section("1", "Movies", LibraryType::Movie)],
            true,
        );
        let jelly = group(
            "Jelly",
            "jellyfin",
            "jf",
            vec![section("a", "Shows", LibraryType::Show)],
            true,
        );
        let rows = derive_rows(&[plex, jelly], &HashSet::new(), false);

        // Home appears exactly once, at the top.
        assert_eq!(rows[0], SidebarRow::Home);
        assert_eq!(
            rows.iter()
                .filter(|r| matches!(r, SidebarRow::Home))
                .count(),
            1
        );

        // Two headers, in insertion order.
        let headers: Vec<&str> = rows
            .iter()
            .filter_map(|r| match r {
                SidebarRow::SourceHeader { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(headers, vec!["Plex", "Jelly"]);

        // Each source contributes its own library + collections row.
        let lib_titles: Vec<&str> = rows
            .iter()
            .filter_map(|r| match r {
                SidebarRow::Library { title, .. } => Some(title.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(lib_titles, vec!["Movies", "Shows"]);
        assert_eq!(
            rows.iter()
                .filter(|r| matches!(r, SidebarRow::Collections { .. }))
                .count(),
            2
        );
    }

    #[test]
    fn derive_rows_hidden_library_excluded_per_source() {
        // Same bare key "2" on two servers: hiding jellyfin:jf:2 must not hide
        // the same-keyed Plex library plex:px:2.
        let plex = group(
            "Plex",
            "plex",
            "px",
            vec![section("2", "Plex Lib", LibraryType::Movie)],
            true,
        );
        let jelly = group(
            "Jelly",
            "jellyfin",
            "jf",
            vec![section("2", "Jelly Lib", LibraryType::Movie)],
            true,
        );
        let h = hidden(&["jellyfin:jf:2"]);
        let rows = derive_rows(&[plex, jelly], &h, false);

        let lib_titles: Vec<&str> = rows
            .iter()
            .filter_map(|r| match r {
                SidebarRow::Library { title, .. } => Some(title.as_str()),
                _ => None,
            })
            .collect();
        // Plex lib survives, Jellyfin lib is hidden.
        assert_eq!(lib_titles, vec!["Plex Lib"]);
    }

    #[test]
    fn derive_rows_preserves_source_order() {
        let a = group("A", "plex", "a", vec![], true);
        let b = group("B", "jellyfin", "b", vec![], true);
        let c = group("C", "plex", "c", vec![], true);
        let rows = derive_rows(&[a, b, c], &HashSet::new(), false);
        let headers: Vec<&str> = rows
            .iter()
            .filter_map(|r| match r {
                SidebarRow::SourceHeader { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(headers, vec!["A", "B", "C"]);
    }

    #[test]
    fn derive_rows_loading_state_per_source() {
        // First source loaded with a library, second still loading.
        let loaded = group(
            "Loaded",
            "plex",
            "p",
            vec![section("1", "Movies", LibraryType::Movie)],
            true,
        );
        let pending = group("Pending", "jellyfin", "j", vec![], false);
        let rows = derive_rows(&[loaded, pending], &HashSet::new(), false);
        // Exactly one Loading row, belonging to the second group.
        assert_eq!(
            rows.iter()
                .filter(|r| matches!(r, SidebarRow::Loading))
                .count(),
            1
        );
        // The Loading row comes after the second header.
        let pending_header = rows
            .iter()
            .position(|r| matches!(r, SidebarRow::SourceHeader { name, .. } if name == "Pending"))
            .unwrap();
        assert!(matches!(rows[pending_header + 1], SidebarRow::Loading));
    }

    #[test]
    fn normal_mode_hides_hidden_library() {
        let libs = vec![
            section("1", "Movies", LibraryType::Movie),
            section("2", "Home Videos", LibraryType::Movie),
        ];
        let sources = vec![group("S", "plex", "srv", libs, true)];
        let h = hidden(&["plex:srv:2"]);
        let rows = derive_rows(&sources, &h, false);
        let lib_titles: Vec<_> = rows
            .iter()
            .filter_map(|r| match r {
                SidebarRow::Library { title, .. } => Some(title.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(lib_titles, vec!["Movies"]);
    }

    #[test]
    fn edit_mode_lists_all_libraries_with_flags() {
        let libs = vec![
            section("1", "Movies", LibraryType::Movie),
            section("2", "Home Videos", LibraryType::Movie),
        ];
        let sources = vec![group("S", "plex", "srv", libs, true)];
        let h = hidden(&["plex:srv:2"]);
        let rows = derive_rows(&sources, &h, true);
        let libs_in_rows: Vec<_> = rows
            .iter()
            .filter_map(|r| match r {
                SidebarRow::Library { title, visible, .. } => Some((title.as_str(), *visible)),
                _ => None,
            })
            .collect();
        assert_eq!(libs_in_rows, vec![("Movies", true), ("Home Videos", false)]);
    }

    #[test]
    fn all_hidden_keeps_source_node_with_marker() {
        let libs = vec![section("1", "Movies", LibraryType::Movie)];
        // Hide the library and Collections.
        let sources = vec![group("S", "plex", "srv", libs, true)];
        let h = hidden(&["plex:srv:1", "plex:srv:__collections__"]);
        let rows = derive_rows(&sources, &h, false);
        assert_eq!(
            rows,
            vec![
                SidebarRow::Home,
                SidebarRow::Downloads,
                SidebarRow::SourceHeader {
                    name: "S".to_string(),
                    source_type: "plex".to_string(),
                    source_id: "srv".to_string(),
                },
                SidebarRow::AllHidden,
            ]
        );
    }

    #[test]
    fn collections_can_be_hidden_independently() {
        let libs = vec![section("1", "Movies", LibraryType::Movie)];
        let sources = vec![group("S", "plex", "srv", libs, true)];
        let h = hidden(&["plex:srv:__collections__"]);
        let rows = derive_rows(&sources, &h, false);
        assert!(
            !rows
                .iter()
                .any(|r| matches!(r, SidebarRow::Collections { .. }))
        );
        // The library is still visible, so no all-hidden marker.
        assert!(!rows.iter().any(|r| matches!(r, SidebarRow::AllHidden)));
    }
}
