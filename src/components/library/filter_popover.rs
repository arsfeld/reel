//! Adwaita filter popover for the library view.
//!
//! Owns a `gtk::Popover` whose content is rebuilt from the current item set
//! plus the active `FilterState`. Each control mutates a shared `FilterState`
//! and emits `LibraryViewMsg::FilterStateChanged`. Programmatic updates
//! (`set_state` / `reset`) set a suppress flag so they don't echo change
//! events back to the parent.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use relm4::Sender;

use crate::models::media::MediaItem;
use crate::services::library_filter::{
    ContentRatingFilter, FilterState, GenreFilter, HdrSelection, RatingFilter, ResolutionFilter,
    RuntimeRangeFilter, WatchStatus, WatchStatusFilter, YearRangeFilter, extract_content_ratings,
    extract_genres, extract_resolution_buckets,
};

use super::LibraryViewMsg;

/// Year spin-row sentinel bounds. A "from" at the floor or "to" at the ceiling
/// is treated as "no bound". The floor sits below the earliest film year
/// (~1888) so every real year, including 1900, is selectable as a real bound.
const YEAR_MIN: f64 = 1880.0;
const YEAR_MAX: f64 = 2100.0;
/// Runtime ceiling in minutes; "to" at this value means "no upper bound".
const RUNTIME_MAX: f64 = 600.0;

pub struct FilterPopover {
    pub popover: gtk::Popover,
    /// Vertical box holding the preference groups; cleared and rebuilt.
    content: gtk::Box,
    sender: Sender<LibraryViewMsg>,
    state: Rc<RefCell<FilterState>>,
    items: Vec<MediaItem>,
    /// When true, control change handlers do not emit (used during rebuild).
    suppress: Rc<Cell<bool>>,
}

impl FilterPopover {
    pub fn new(sender: Sender<LibraryViewMsg>) -> Self {
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(18)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .width_request(320)
            .build();

        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .max_content_height(560)
            .propagate_natural_height(true)
            .child(&content)
            .build();

        let popover = gtk::Popover::builder().child(&scroller).build();

        let me = Self {
            popover,
            content,
            sender,
            state: Rc::new(RefCell::new(FilterState::default())),
            items: Vec::new(),
            suppress: Rc::new(Cell::new(false)),
        };
        me.rebuild();
        me
    }

    /// Replace the active filter state (e.g. restoring persisted filters) and
    /// reflect it in the controls without emitting change events.
    pub fn set_state(&mut self, state: &FilterState) {
        *self.state.borrow_mut() = state.clone();
        self.rebuild();
    }

    /// Set both the item set and the filter state, rebuilding the popover only
    /// once. Used on library load to avoid two back-to-back reconstructions.
    pub fn reset(&mut self, items: &[MediaItem], state: &FilterState) {
        self.items = items.to_vec();
        *self.state.borrow_mut() = state.clone();
        self.rebuild();
    }

    /// Clear and reconstruct all preference groups from `items` + `state`.
    fn rebuild(&self) {
        self.suppress.set(true);

        while let Some(child) = self.content.first_child() {
            self.content.remove(&child);
        }

        self.build_watch_status_group();
        self.build_year_group();
        self.build_genre_group();
        self.build_rating_group();
        self.build_runtime_group();
        self.build_content_rating_group();
        self.build_resolution_group();
        self.build_clear_all();

        self.suppress.set(false);
    }

    fn build_watch_status_group(&self) {
        let group = adw::PreferencesGroup::builder()
            .title("Watch status")
            .build();
        let model = gtk::StringList::new(&["Any", "Unwatched", "In Progress", "Watched"]);
        let row = adw::ComboRow::builder()
            .title("Status")
            .model(&model)
            .build();

        let selected = match self.state.borrow().watch_status.map(|w| w.status) {
            None => 0,
            Some(WatchStatus::Unwatched) => 1,
            Some(WatchStatus::InProgress) => 2,
            Some(WatchStatus::Watched) => 3,
        };
        row.set_selected(selected);

        let state = self.state.clone();
        let suppress = self.suppress.clone();
        let sender = self.sender.clone();
        row.connect_selected_notify(move |r| {
            if suppress.get() {
                return;
            }
            let status = match r.selected() {
                1 => Some(WatchStatus::Unwatched),
                2 => Some(WatchStatus::InProgress),
                3 => Some(WatchStatus::Watched),
                _ => None,
            };
            state.borrow_mut().watch_status = status.map(|status| WatchStatusFilter { status });
            let _ = sender.send(LibraryViewMsg::FilterStateChanged(state.borrow().clone()));
        });

        group.add(&row);
        self.content.append(&group);
    }

    fn build_year_group(&self) {
        let group = adw::PreferencesGroup::builder().title("Year").build();
        let (from_val, to_val) = match self.state.borrow().year_range {
            Some(yr) => (
                yr.from.map_or(YEAR_MIN, f64::from),
                yr.to.map_or(YEAR_MAX, f64::from),
            ),
            None => (YEAR_MIN, YEAR_MAX),
        };

        let from_row = adw::SpinRow::with_range(YEAR_MIN, YEAR_MAX, 1.0);
        from_row.set_title("From");
        from_row.set_digits(0);
        from_row.set_value(from_val);

        let to_row = adw::SpinRow::with_range(YEAR_MIN, YEAR_MAX, 1.0);
        to_row.set_title("To");
        to_row.set_digits(0);
        to_row.set_value(to_val);

        let on_change = {
            let state = self.state.clone();
            let suppress = self.suppress.clone();
            let sender = self.sender.clone();
            let from_row = from_row.clone();
            let to_row = to_row.clone();
            move || {
                if suppress.get() {
                    return;
                }
                let from = from_row.value();
                let to = to_row.value();
                let from_opt = (from > YEAR_MIN).then_some(from as i32);
                let to_opt = (to < YEAR_MAX).then_some(to as i32);
                state.borrow_mut().year_range = if from_opt.is_none() && to_opt.is_none() {
                    None
                } else {
                    Some(YearRangeFilter {
                        from: from_opt,
                        to: to_opt,
                    })
                };
                let _ = sender.send(LibraryViewMsg::FilterStateChanged(state.borrow().clone()));
            }
        };
        let oc1 = on_change.clone();
        from_row.connect_value_notify(move |_| oc1());
        to_row.connect_value_notify(move |_| on_change());

        group.add(&from_row);
        group.add(&to_row);
        self.content.append(&group);
    }

    fn build_genre_group(&self) {
        let genres = extract_genres(&self.items);
        if genres.is_empty() {
            return;
        }
        let selected = self
            .state
            .borrow()
            .genres
            .as_ref()
            .map(|g| g.selected_genres.clone())
            .unwrap_or_default();

        let group = adw::PreferencesGroup::builder().title("Genre").build();
        let expander = adw::ExpanderRow::builder().title("Genres").build();
        if !selected.is_empty() {
            expander.set_subtitle(&format!("{} selected", selected.len()));
        }

        for genre in &genres {
            let row = adw::SwitchRow::builder()
                .title(genre)
                .active(selected.contains(genre))
                .build();
            let state = self.state.clone();
            let suppress = self.suppress.clone();
            let sender = self.sender.clone();
            let genre = genre.clone();
            row.connect_active_notify(move |r| {
                if suppress.get() {
                    return;
                }
                {
                    let mut s = state.borrow_mut();
                    let gf = s.genres.get_or_insert_with(|| GenreFilter {
                        selected_genres: Vec::new(),
                    });
                    if r.is_active() {
                        if !gf.selected_genres.contains(&genre) {
                            gf.selected_genres.push(genre.clone());
                        }
                    } else {
                        gf.selected_genres.retain(|g| g != &genre);
                    }
                    if gf.selected_genres.is_empty() {
                        s.genres = None;
                    }
                }
                let _ = sender.send(LibraryViewMsg::FilterStateChanged(state.borrow().clone()));
            });
            expander.add_row(&row);
        }

        group.add(&expander);
        self.content.append(&group);
    }

    fn build_rating_group(&self) {
        let group = adw::PreferencesGroup::builder().title("Rating").build();
        let row = adw::SpinRow::with_range(0.0, 10.0, 0.5);
        row.set_title("Minimum");
        row.set_digits(1);
        row.set_value(self.state.borrow().rating_min.map_or(0.0, |r| r.min));

        let state = self.state.clone();
        let suppress = self.suppress.clone();
        let sender = self.sender.clone();
        row.connect_value_notify(move |r| {
            if suppress.get() {
                return;
            }
            let v = r.value();
            state.borrow_mut().rating_min = (v > 0.0).then_some(RatingFilter { min: v });
            let _ = sender.send(LibraryViewMsg::FilterStateChanged(state.borrow().clone()));
        });

        group.add(&row);
        self.content.append(&group);
    }

    fn build_runtime_group(&self) {
        let group = adw::PreferencesGroup::builder()
            .title("Runtime (minutes)")
            .build();
        let (from_val, to_val) = match self.state.borrow().runtime_range {
            Some(rt) => (rt.from.map_or(0.0, f64::from), rt.to.map_or(0.0, f64::from)),
            None => (0.0, 0.0),
        };

        let from_row = adw::SpinRow::with_range(0.0, RUNTIME_MAX, 5.0);
        from_row.set_title("Min");
        from_row.set_digits(0);
        from_row.set_value(from_val);

        let to_row = adw::SpinRow::with_range(0.0, RUNTIME_MAX, 5.0);
        to_row.set_title("Max (0 = none)");
        to_row.set_digits(0);
        to_row.set_value(to_val);

        let on_change = {
            let state = self.state.clone();
            let suppress = self.suppress.clone();
            let sender = self.sender.clone();
            let from_row = from_row.clone();
            let to_row = to_row.clone();
            move || {
                if suppress.get() {
                    return;
                }
                let from = from_row.value();
                let to = to_row.value();
                let from_opt = (from > 0.0).then_some(from as i32);
                let to_opt = (to > 0.0).then_some(to as i32);
                state.borrow_mut().runtime_range = if from_opt.is_none() && to_opt.is_none() {
                    None
                } else {
                    Some(RuntimeRangeFilter {
                        from: from_opt,
                        to: to_opt,
                    })
                };
                let _ = sender.send(LibraryViewMsg::FilterStateChanged(state.borrow().clone()));
            }
        };
        let oc1 = on_change.clone();
        from_row.connect_value_notify(move |_| oc1());
        to_row.connect_value_notify(move |_| on_change());

        group.add(&from_row);
        group.add(&to_row);
        self.content.append(&group);
    }

    fn build_content_rating_group(&self) {
        let ratings = extract_content_ratings(&self.items);
        if ratings.is_empty() {
            return;
        }
        let selected = self
            .state
            .borrow()
            .content_ratings
            .as_ref()
            .map(|c| c.selected.clone())
            .unwrap_or_default();

        let group = adw::PreferencesGroup::builder()
            .title("Content rating")
            .build();
        let expander = adw::ExpanderRow::builder().title("Ratings").build();
        if !selected.is_empty() {
            expander.set_subtitle(&format!("{} selected", selected.len()));
        }

        for rating in &ratings {
            let row = adw::SwitchRow::builder()
                .title(rating)
                .active(selected.contains(rating))
                .build();
            let state = self.state.clone();
            let suppress = self.suppress.clone();
            let sender = self.sender.clone();
            let rating = rating.clone();
            row.connect_active_notify(move |r| {
                if suppress.get() {
                    return;
                }
                {
                    let mut s = state.borrow_mut();
                    let cf = s
                        .content_ratings
                        .get_or_insert_with(|| ContentRatingFilter {
                            selected: Vec::new(),
                        });
                    if r.is_active() {
                        if !cf.selected.contains(&rating) {
                            cf.selected.push(rating.clone());
                        }
                    } else {
                        cf.selected.retain(|c| c != &rating);
                    }
                    if cf.selected.is_empty() {
                        s.content_ratings = None;
                    }
                }
                let _ = sender.send(LibraryViewMsg::FilterStateChanged(state.borrow().clone()));
            });
            expander.add_row(&row);
        }

        group.add(&expander);
        self.content.append(&group);
    }

    fn build_resolution_group(&self) {
        let buckets = extract_resolution_buckets(&self.items);
        let group = adw::PreferencesGroup::builder().title("Resolution").build();

        let (selected_buckets, hdr_sel) = match self.state.borrow().resolutions.as_ref() {
            Some(res) => (res.buckets.clone(), res.hdr),
            None => (Vec::new(), None),
        };

        for bucket in buckets {
            let row = adw::SwitchRow::builder()
                .title(bucket.label())
                .active(selected_buckets.contains(&bucket))
                .build();
            let state = self.state.clone();
            let suppress = self.suppress.clone();
            let sender = self.sender.clone();
            row.connect_active_notify(move |r| {
                if suppress.get() {
                    return;
                }
                {
                    let mut s = state.borrow_mut();
                    let res = s.resolutions.get_or_insert_with(ResolutionFilter::default);
                    if r.is_active() {
                        if !res.buckets.contains(&bucket) {
                            res.buckets.push(bucket);
                        }
                    } else {
                        res.buckets.retain(|b| b != &bucket);
                    }
                    if res.buckets.is_empty() && res.hdr.is_none() {
                        s.resolutions = None;
                    }
                }
                let _ = sender.send(LibraryViewMsg::FilterStateChanged(state.borrow().clone()));
            });
            group.add(&row);
        }

        // HDR toggle (always available).
        let hdr_row = adw::SwitchRow::builder()
            .title("HDR / Dolby Vision only")
            .active(hdr_sel == Some(HdrSelection::AnyHdr))
            .build();
        let state = self.state.clone();
        let suppress = self.suppress.clone();
        let sender = self.sender.clone();
        hdr_row.connect_active_notify(move |r| {
            if suppress.get() {
                return;
            }
            {
                let mut s = state.borrow_mut();
                let res = s.resolutions.get_or_insert_with(ResolutionFilter::default);
                res.hdr = if r.is_active() {
                    Some(HdrSelection::AnyHdr)
                } else {
                    None
                };
                if res.buckets.is_empty() && res.hdr.is_none() {
                    s.resolutions = None;
                }
            }
            let _ = sender.send(LibraryViewMsg::FilterStateChanged(state.borrow().clone()));
        });
        group.add(&hdr_row);

        self.content.append(&group);
    }

    fn build_clear_all(&self) {
        let btn = gtk::Button::builder()
            .label("Clear all filters")
            .css_classes(["flat"])
            .halign(gtk::Align::Center)
            .margin_top(4)
            .build();
        let sender = self.sender.clone();
        let popover = self.popover.clone();
        btn.connect_clicked(move |_| {
            let _ = sender.send(LibraryViewMsg::ClearFilters);
            popover.popdown();
        });
        self.content.append(&btn);
    }
}
