use std::collections::HashSet;

use gtk::prelude::*;
use relm4::prelude::*;

use crate::models::library::{LibrarySection, LibraryType};

/// Reserved section key for the per-source Collections entry's visibility.
/// A real Plex section key is numeric, so this never collides.
pub const COLLECTIONS_KEY: &str = "__collections__";

/// What the sidebar currently has selected, for highlight sync.
#[derive(Debug, Clone, PartialEq)]
pub enum SidebarSelection {
    Home,
    Library(String),
    Collections,
    Downloads,
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
    /// The source node header. Carries the edit affordance.
    SourceHeader {
        name: String,
    },
    /// Placeholder shown under the source header before libraries resolve.
    Loading,
    /// Shown when the source loaded but reported no (supported) libraries — a
    /// terminal state, distinct from `Loading`, so the sidebar never spins
    /// forever if a fetch comes back empty.
    NoLibraries,
    /// Shown in normal mode when the source has libraries but all are hidden,
    /// so the source node never collapses to nothing.
    AllHidden,
    Library {
        key: String,
        title: String,
        library_type: LibraryType,
        visible: bool,
    },
    Collections {
        visible: bool,
    },
}

/// Derive the ordered list of sidebar rows from current state. In normal mode
/// only visible libraries (and Collections) appear; in edit mode every library
/// appears with its visibility flag so a toggle can be rendered.
pub fn derive_rows(
    source_name: Option<&str>,
    source_type: &str,
    source_id: &str,
    libraries: &[LibrarySection],
    hidden: &HashSet<String>,
    edit_mode: bool,
    loaded: bool,
) -> Vec<SidebarRow> {
    let mut rows = vec![SidebarRow::Home, SidebarRow::Downloads];

    let Some(name) = source_name else {
        return rows;
    };
    rows.push(SidebarRow::SourceHeader {
        name: name.to_string(),
    });

    // Source connected but its libraries have not been fetched yet.
    if !loaded {
        rows.push(SidebarRow::Loading);
        return rows;
    }

    // Loaded, but the source reported no supported libraries.
    if libraries.is_empty() {
        rows.push(SidebarRow::NoLibraries);
        return rows;
    }

    let is_hidden = |key: &str| {
        hidden.contains(&LibrarySection::visibility_key_for(
            source_type,
            source_id,
            key,
        ))
    };

    let mut any_visible = false;
    for lib in libraries {
        let visible = !is_hidden(&lib.key);
        any_visible |= visible;
        if edit_mode || visible {
            rows.push(SidebarRow::Library {
                key: lib.key.clone(),
                title: lib.title.clone(),
                library_type: lib.library_type,
                visible,
            });
        }
    }

    let collections_visible = !is_hidden(COLLECTIONS_KEY);
    any_visible |= collections_visible;
    if edit_mode || collections_visible {
        rows.push(SidebarRow::Collections {
            visible: collections_visible,
        });
    }

    if !edit_mode && !any_visible {
        rows.push(SidebarRow::AllHidden);
    }

    rows
}

/// Navigation action a row triggers when activated. `None` for non-actionable
/// rows (header, loading, all-hidden). Indexed in parallel with the listbox.
#[derive(Debug, Clone)]
enum RowAction {
    None,
    Home,
    Library(LibrarySection),
    Collections,
    Downloads,
}

pub struct Sidebar {
    source_name: Option<String>,
    source_type: String,
    source_id: String,
    libraries: Vec<LibrarySection>,
    /// True once a libraries fetch has completed (success or empty), so the
    /// sidebar shows a terminal state instead of spinning on "Loading" forever.
    libraries_loaded: bool,
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
    /// Set the connected source's display name + identity (used to build
    /// per-library visibility keys).
    SetSource {
        name: String,
        source_type: String,
        source_id: String,
    },
    /// Provide the source's libraries (fetched async by the app).
    SetLibraries(Vec<LibrarySection>),
    /// Update the hidden-library set.
    SetVisibility(HashSet<String>),
    /// A listbox row was selected (by index into `row_actions`).
    RowSelected(usize),
    /// Toggle a library/collections entry's visibility (from its edit switch).
    ToggleEntry { key: String, visible: bool },
    /// Enter/leave edit mode.
    ToggleEditMode,
    /// Select a row programmatically (e.g. on startup / back navigation)
    /// without emitting a navigation output.
    SelectExternal(SidebarSelection),
}

#[derive(Debug)]
pub enum SidebarOutput {
    NavigateHome,
    Navigate(LibrarySection),
    ShowCollections,
    ShowDownloads,
    /// Persist a visibility change for a library (or Collections) entry.
    SetLibraryVisible {
        key: String,
        visible: bool,
    },
    /// Edit mode was exited; the app re-evaluates the current view (a hidden
    /// current library redirects to Home).
    EditModeExited,
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
            source_name: None,
            source_type: String::new(),
            source_id: String::new(),
            libraries: Vec::new(),
            libraries_loaded: false,
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
                self.source_name = Some(name);
                self.source_type = source_type;
                self.source_id = source_id;
                self.rebuild(&sender);
            }
            SidebarMsg::SetLibraries(libraries) => {
                self.libraries = libraries;
                self.libraries_loaded = true;
                self.rebuild(&sender);
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
            SidebarMsg::ToggleEntry { key, visible } => {
                let vis_key =
                    LibrarySection::visibility_key_for(&self.source_type, &self.source_id, &key);
                if visible {
                    self.hidden.remove(&vis_key);
                } else {
                    self.hidden.insert(vis_key);
                }
                // The switch already reflects the new state; no rebuild while in
                // edit mode (rows stay put). Persistence + re-filter happen in
                // the app via the output below.
                let _ = sender.output(SidebarOutput::SetLibraryVisible { key, visible });
            }
            SidebarMsg::RowSelected(index) => {
                // Programmatic selection is blocked at the signal, so this only
                // fires for real user clicks on selectable (actionable) rows.
                match self.row_actions.get(index) {
                    Some(RowAction::Home) => {
                        self.selected = SidebarSelection::Home;
                        let _ = sender.output(SidebarOutput::NavigateHome);
                    }
                    Some(RowAction::Library(section)) => {
                        self.selected = SidebarSelection::Library(section.key.clone());
                        let _ = sender.output(SidebarOutput::Navigate(section.clone()));
                    }
                    Some(RowAction::Collections) => {
                        self.selected = SidebarSelection::Collections;
                        let _ = sender.output(SidebarOutput::ShowCollections);
                    }
                    Some(RowAction::Downloads) => {
                        self.selected = SidebarSelection::Downloads;
                        let _ = sender.output(SidebarOutput::ShowDownloads);
                    }
                    Some(RowAction::None) | None => {}
                }
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
    /// Clear and rebuild the listbox from current state, refreshing the parallel
    /// `row_actions`. The row-selected handler is blocked throughout so clearing
    /// and re-selecting can't emit spurious navigation.
    fn rebuild(&mut self, sender: &ComponentSender<Self>) {
        self.listbox.block_signal(&self.row_selected_handler);

        while let Some(child) = self.listbox.first_child() {
            self.listbox.remove(&child);
        }

        let rows = derive_rows(
            self.source_name.as_deref(),
            &self.source_type,
            &self.source_id,
            &self.libraries,
            &self.hidden,
            self.edit_mode,
            self.libraries_loaded,
        );

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
            SidebarRow::SourceHeader { name } => {
                (self.source_header_row(name, sender), RowAction::None)
            }
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
            } => {
                let icon = match library_type {
                    LibraryType::Movie => "video-display-symbolic",
                    LibraryType::Show => "view-list-symbolic",
                };
                let switch = self
                    .edit_mode
                    .then(|| self.entry_switch(key, *visible, sender));
                let r = nav_row_indented(icon, title, switch);
                (
                    r,
                    RowAction::Library(LibrarySection {
                        key: key.clone(),
                        title: title.clone(),
                        library_type: *library_type,
                        count: None,
                    }),
                )
            }
            SidebarRow::Collections { visible } => {
                let switch = self
                    .edit_mode
                    .then(|| self.entry_switch(COLLECTIONS_KEY, *visible, sender));
                let r = nav_row_indented("view-grid-symbolic", "Collections", switch);
                (r, RowAction::Collections)
            }
        }
    }

    /// The source node header row: the source name plus the edit/Done affordance.
    fn source_header_row(&self, name: &str, sender: &ComponentSender<Self>) -> gtk::ListBoxRow {
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

        r.set_child(Some(&b));
        r
    }

    /// A visibility switch for a library/collections entry in edit mode.
    fn entry_switch(
        &self,
        key: &str,
        visible: bool,
        sender: &ComponentSender<Self>,
    ) -> gtk::Switch {
        let switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        // Set state before connecting so the initial value doesn't fire.
        switch.set_active(visible);
        let s = sender.input_sender().clone();
        let key = key.to_string();
        switch.connect_active_notify(move |sw| {
            let _ = s.send(SidebarMsg::ToggleEntry {
                key: key.clone(),
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
                (SidebarSelection::Library(k), RowAction::Library(section)) => *k == section.key,
                (SidebarSelection::Collections, RowAction::Collections) => true,
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

    #[test]
    fn no_source_shows_home_and_downloads() {
        // Downloads is a top-level destination, present with zero sources (R1).
        let rows = derive_rows(None, "plex", "srv", &[], &HashSet::new(), false, false);
        assert_eq!(rows, vec![SidebarRow::Home, SidebarRow::Downloads]);
    }

    #[test]
    fn downloads_row_precedes_source_header() {
        let libs = vec![section("1", "Movies", LibraryType::Movie)];
        let rows = derive_rows(
            Some("S"),
            "plex",
            "srv",
            &libs,
            &HashSet::new(),
            false,
            true,
        );
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
        let rows = derive_rows(
            Some("My Plex"),
            "plex",
            "srv",
            &[],
            &HashSet::new(),
            false,
            false,
        );
        assert_eq!(
            rows,
            vec![
                SidebarRow::Home,
                SidebarRow::Downloads,
                SidebarRow::SourceHeader {
                    name: "My Plex".to_string()
                },
                SidebarRow::Loading,
            ]
        );
    }

    #[test]
    fn loaded_but_empty_shows_no_libraries() {
        let rows = derive_rows(
            Some("My Plex"),
            "plex",
            "srv",
            &[],
            &HashSet::new(),
            false,
            true,
        );
        assert_eq!(
            rows,
            vec![
                SidebarRow::Home,
                SidebarRow::Downloads,
                SidebarRow::SourceHeader {
                    name: "My Plex".to_string()
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
        let rows = derive_rows(
            Some("S"),
            "plex",
            "srv",
            &libs,
            &HashSet::new(),
            false,
            true,
        );
        // Home, Downloads, header, 2 libraries, collections.
        assert_eq!(rows.len(), 6);
        assert!(matches!(rows[3], SidebarRow::Library { ref title, .. } if title == "Movies"));
        assert!(matches!(rows[5], SidebarRow::Collections { visible: true }));
    }

    #[test]
    fn normal_mode_hides_hidden_library() {
        let libs = vec![
            section("1", "Movies", LibraryType::Movie),
            section("2", "Home Videos", LibraryType::Movie),
        ];
        let h = hidden(&["plex:srv:2"]);
        let rows = derive_rows(Some("S"), "plex", "srv", &libs, &h, false, true);
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
        let h = hidden(&["plex:srv:2"]);
        let rows = derive_rows(Some("S"), "plex", "srv", &libs, &h, true, true);
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
        let h = hidden(&["plex:srv:1", "plex:srv:__collections__"]);
        let rows = derive_rows(Some("S"), "plex", "srv", &libs, &h, false, true);
        assert_eq!(
            rows,
            vec![
                SidebarRow::Home,
                SidebarRow::Downloads,
                SidebarRow::SourceHeader {
                    name: "S".to_string()
                },
                SidebarRow::AllHidden,
            ]
        );
    }

    #[test]
    fn collections_can_be_hidden_independently() {
        let libs = vec![section("1", "Movies", LibraryType::Movie)];
        let h = hidden(&["plex:srv:__collections__"]);
        let rows = derive_rows(Some("S"), "plex", "srv", &libs, &h, false, true);
        assert!(
            !rows
                .iter()
                .any(|r| matches!(r, SidebarRow::Collections { .. }))
        );
        // The library is still visible, so no all-hidden marker.
        assert!(!rows.iter().any(|r| matches!(r, SidebarRow::AllHidden)));
    }
}
