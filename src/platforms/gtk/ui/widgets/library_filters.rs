use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::core::viewmodels::library_view_model::{FilterOptions, SortOrder, WatchStatus};

/// Filter state structure for communicating filter changes
#[derive(Debug, Clone)]
pub struct FilterState {
    pub watch_status: WatchStatus,
    pub sort_order: SortOrder,
    pub genres: Vec<String>,
    pub years: Option<(i32, i32)>,
    pub min_rating: Option<f32>,
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            watch_status: WatchStatus::All,
            sort_order: SortOrder::TitleAsc,
            genres: Vec::new(),
            years: None,
            min_rating: None,
        }
    }
}

impl From<FilterState> for FilterOptions {
    fn from(state: FilterState) -> Self {
        FilterOptions {
            search: String::new(), // Search is handled separately
            genres: state.genres,
            years: state.years,
            min_rating: state.min_rating,
            watch_status: state.watch_status,
        }
    }
}

/// Library filters widget for managing filter controls
pub struct LibraryFilters {
    widget: gtk4::Box,
    chips_box: gtk4::FlowBox,
    on_filter_changed: RefCell<Option<Rc<dyn Fn(FilterState)>>>,
    current_state: RefCell<FilterState>,
}

impl std::fmt::Debug for LibraryFilters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryFilters")
            .field("widget", &self.widget)
            .field("chips_box", &self.chips_box)
            .field("on_filter_changed", &"<callback>")
            .field("current_state", &self.current_state)
            .finish()
    }
}

impl LibraryFilters {
    /// Create a new LibraryFilters widget
    pub fn new() -> Self {
        // Main container with vertical orientation
        let main_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(8)
            .build();

        // Controls container
        let controls_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .build();

        // Chips container for active filters
        let chips_box = gtk4::FlowBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .column_spacing(4)
            .row_spacing(4)
            .margin_top(4)
            .build();

        // Add controls and chips to main container
        main_box.append(&controls_box);
        main_box.append(&chips_box);

        let instance = Self {
            widget: main_box,
            chips_box,
            on_filter_changed: RefCell::new(None),
            current_state: RefCell::new(FilterState::default()),
        };

        instance.build_controls(&controls_box);
        instance
    }

    /// Get the GTK widget
    pub fn widget(&self) -> &gtk4::Box {
        &self.widget
    }

    /// Set callback for when filter state changes
    pub fn set_on_filter_changed<F>(&self, callback: F)
    where
        F: Fn(FilterState) + 'static,
    {
        *self.on_filter_changed.borrow_mut() = Some(Rc::new(callback));
    }

    /// Get current filter state
    pub fn get_filter_state(&self) -> FilterState {
        self.current_state.borrow().clone()
    }

    /// Build the filter controls UI
    fn build_controls(&self, controls_box: &gtk4::Box) {
        // Create watch status dropdown
        let watch_status_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(4)
            .build();

        let watch_label = gtk4::Label::builder().label("Show:").build();
        watch_label.add_css_class("dim-label");

        let watch_model = gtk4::StringList::new(&["All", "Unwatched", "Watched", "In Progress"]);

        let watch_dropdown = gtk4::DropDown::builder()
            .model(&watch_model)
            .selected(0)
            .tooltip_text("Filter by watch status")
            .build();

        watch_status_box.append(&watch_label);
        watch_status_box.append(&watch_dropdown);

        // Create sort order dropdown
        let sort_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(4)
            .build();

        let sort_label = gtk4::Label::builder().label("Sort:").build();
        sort_label.add_css_class("dim-label");

        let sort_model = gtk4::StringList::new(&[
            "Title (A-Z)",
            "Title (Z-A)",
            "Year (Oldest)",
            "Year (Newest)",
            "Rating (Low-High)",
            "Rating (High-Low)",
            "Added (Oldest)",
            "Added (Newest)",
        ]);

        let sort_dropdown = gtk4::DropDown::builder()
            .model(&sort_model)
            .selected(0)
            .tooltip_text("Sort order")
            .build();

        sort_box.append(&sort_label);
        sort_box.append(&sort_dropdown);

        // Create genre filter dropdown
        let genre_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(4)
            .build();

        let genre_label = gtk4::Label::builder().label("Genre:").build();
        genre_label.add_css_class("dim-label");

        // TODO: Get actual genres from library - for now use common ones
        let genre_model = gtk4::StringList::new(&[
            "All Genres",
            "Action",
            "Adventure",
            "Comedy",
            "Drama",
            "Horror",
            "Romance",
            "Sci-Fi",
            "Thriller",
        ]);

        let genre_dropdown = gtk4::DropDown::builder()
            .model(&genre_model)
            .selected(0)
            .tooltip_text("Filter by genre")
            .build();

        genre_box.append(&genre_label);
        genre_box.append(&genre_dropdown);

        // Create year range filter
        let year_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(4)
            .build();

        let year_label = gtk4::Label::builder().label("Year:").build();
        year_label.add_css_class("dim-label");

        let year_from_spin = gtk4::SpinButton::builder()
            .adjustment(&gtk4::Adjustment::new(
                1900.0, 1900.0, 2030.0, 1.0, 10.0, 0.0,
            ))
            .value(1900.0)
            .tooltip_text("From year")
            .width_chars(6)
            .build();

        let year_to_label = gtk4::Label::builder().label("to").build();
        year_to_label.add_css_class("dim-label");

        let year_to_spin = gtk4::SpinButton::builder()
            .adjustment(&gtk4::Adjustment::new(
                2030.0, 1900.0, 2030.0, 1.0, 10.0, 0.0,
            ))
            .value(2030.0)
            .tooltip_text("To year")
            .width_chars(6)
            .build();

        year_box.append(&year_label);
        year_box.append(&year_from_spin);
        year_box.append(&year_to_label);
        year_box.append(&year_to_spin);

        // Create rating filter
        let rating_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(4)
            .build();

        let rating_label = gtk4::Label::builder().label("Min Rating:").build();
        rating_label.add_css_class("dim-label");

        let rating_spin = gtk4::SpinButton::builder()
            .adjustment(&gtk4::Adjustment::new(0.0, 0.0, 10.0, 0.1, 1.0, 0.0))
            .value(0.0)
            .digits(1)
            .tooltip_text("Minimum rating")
            .width_chars(4)
            .build();

        rating_box.append(&rating_label);
        rating_box.append(&rating_spin);

        // Connect watch status filter handler
        let state_ref = Rc::new(RefCell::new(self.current_state.borrow().clone()));
        let callback_ref = Rc::new(RefCell::new(self.on_filter_changed.borrow().clone()));
        let chips_box_ref = Rc::new(self.chips_box.clone());

        let state_clone = state_ref.clone();
        let callback_clone = callback_ref.clone();
        let chips_box_clone = chips_box_ref.clone();
        watch_dropdown.connect_selected_notify(move |dropdown| {
            let status = match dropdown.selected() {
                0 => WatchStatus::All,
                1 => WatchStatus::Unwatched,
                2 => WatchStatus::Watched,
                3 => WatchStatus::InProgress,
                _ => WatchStatus::All,
            };

            // Update state
            {
                let mut state = state_clone.borrow_mut();
                state.watch_status = status;
            }

            // Update chips display
            Self::update_chips_display(&chips_box_clone, &state_clone.borrow());

            // Notify callback
            if let Some(ref callback) = *callback_clone.borrow() {
                callback(state_clone.borrow().clone());
            }
        });

        // Connect sort order handler
        let state_clone = state_ref.clone();
        let callback_clone = callback_ref.clone();
        let chips_box_clone = chips_box_ref.clone();
        sort_dropdown.connect_selected_notify(move |dropdown| {
            let order = match dropdown.selected() {
                0 => SortOrder::TitleAsc,
                1 => SortOrder::TitleDesc,
                2 => SortOrder::YearAsc,
                3 => SortOrder::YearDesc,
                4 => SortOrder::RatingAsc,
                5 => SortOrder::RatingDesc,
                6 => SortOrder::AddedAsc,
                7 => SortOrder::AddedDesc,
                _ => SortOrder::TitleAsc,
            };

            // Update state
            {
                let mut state = state_clone.borrow_mut();
                state.sort_order = order;
            }

            // Update chips display
            Self::update_chips_display(&chips_box_clone, &state_clone.borrow());

            // Notify callback
            if let Some(ref callback) = *callback_clone.borrow() {
                callback(state_clone.borrow().clone());
            }
        });

        // Connect genre filter handler
        let state_clone = state_ref.clone();
        let callback_clone = callback_ref.clone();
        let chips_box_clone = chips_box_ref.clone();
        genre_dropdown.connect_selected_notify(move |dropdown| {
            let genres = match dropdown.selected() {
                0 => Vec::new(), // "All Genres"
                1 => vec!["Action".to_string()],
                2 => vec!["Adventure".to_string()],
                3 => vec!["Comedy".to_string()],
                4 => vec!["Drama".to_string()],
                5 => vec!["Horror".to_string()],
                6 => vec!["Romance".to_string()],
                7 => vec!["Sci-Fi".to_string()],
                8 => vec!["Thriller".to_string()],
                _ => Vec::new(),
            };

            // Update state
            {
                let mut state = state_clone.borrow_mut();
                state.genres = genres;
            }

            // Update chips display
            Self::update_chips_display(&chips_box_clone, &state_clone.borrow());

            // Notify callback
            if let Some(ref callback) = *callback_clone.borrow() {
                callback(state_clone.borrow().clone());
            }
        });

        // Connect year range handlers
        let state_clone = state_ref.clone();
        let callback_clone = callback_ref.clone();
        let chips_box_clone = chips_box_ref.clone();
        let year_to_spin_clone = year_to_spin.clone();
        year_from_spin.connect_value_changed(move |spin| {
            let from_year = spin.value() as i32;
            let to_year = year_to_spin_clone.value() as i32;

            // Update state
            {
                let mut state = state_clone.borrow_mut();
                if from_year != 1900 || to_year != 2030 {
                    state.years = Some((from_year, to_year));
                } else {
                    state.years = None; // Reset to no filter when at defaults
                }
            }

            // Update chips display
            Self::update_chips_display(&chips_box_clone, &state_clone.borrow());

            // Notify callback
            if let Some(ref callback) = *callback_clone.borrow() {
                callback(state_clone.borrow().clone());
            }
        });

        let state_clone = state_ref.clone();
        let callback_clone = callback_ref.clone();
        let chips_box_clone = chips_box_ref.clone();
        let year_from_spin_clone = year_from_spin.clone();
        year_to_spin.connect_value_changed(move |spin| {
            let to_year = spin.value() as i32;
            let from_year = year_from_spin_clone.value() as i32;

            // Update state
            {
                let mut state = state_clone.borrow_mut();
                if from_year != 1900 || to_year != 2030 {
                    state.years = Some((from_year, to_year));
                } else {
                    state.years = None; // Reset to no filter when at defaults
                }
            }

            // Update chips display
            Self::update_chips_display(&chips_box_clone, &state_clone.borrow());

            // Notify callback
            if let Some(ref callback) = *callback_clone.borrow() {
                callback(state_clone.borrow().clone());
            }
        });

        // Connect rating filter handler
        let state_clone = state_ref.clone();
        let callback_clone = callback_ref.clone();
        let chips_box_clone = chips_box_ref.clone();
        rating_spin.connect_value_changed(move |spin| {
            let rating = spin.value() as f32;

            // Update state
            {
                let mut state = state_clone.borrow_mut();
                if rating > 0.0 {
                    state.min_rating = Some(rating);
                } else {
                    state.min_rating = None; // Reset to no filter when 0
                }
            }

            // Update chips display
            Self::update_chips_display(&chips_box_clone, &state_clone.borrow());

            // Notify callback
            if let Some(ref callback) = *callback_clone.borrow() {
                callback(state_clone.borrow().clone());
            }
        });

        // Create reset button
        let reset_button = gtk4::Button::builder()
            .icon_name("edit-clear-symbolic")
            .tooltip_text("Reset all filters")
            .build();
        reset_button.add_css_class("flat");

        // Connect reset button handler
        let state_clone = state_ref.clone();
        let callback_clone = callback_ref.clone();
        let chips_box_clone = chips_box_ref.clone();
        let watch_dropdown_clone = watch_dropdown.clone();
        let sort_dropdown_clone = sort_dropdown.clone();
        let genre_dropdown_clone = genre_dropdown.clone();
        let year_from_spin_clone = year_from_spin.clone();
        let year_to_spin_clone = year_to_spin.clone();
        let rating_spin_clone = rating_spin.clone();

        reset_button.connect_clicked(move |_| {
            // Reset all controls to default values
            watch_dropdown_clone.set_selected(0); // "All"
            sort_dropdown_clone.set_selected(0); // "Title (A-Z)"
            genre_dropdown_clone.set_selected(0); // "All Genres"
            year_from_spin_clone.set_value(1900.0);
            year_to_spin_clone.set_value(2030.0);
            rating_spin_clone.set_value(0.0);

            // Reset state
            {
                let mut state = state_clone.borrow_mut();
                *state = FilterState::default();
            }

            // Update chips display
            Self::update_chips_display(&chips_box_clone, &state_clone.borrow());

            // Notify callback
            if let Some(ref callback) = *callback_clone.borrow() {
                callback(state_clone.borrow().clone());
            }
        });

        // Add controls to the controls container
        controls_box.append(&watch_status_box);
        controls_box.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
        controls_box.append(&sort_box);
        controls_box.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
        controls_box.append(&genre_box);
        controls_box.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
        controls_box.append(&year_box);
        controls_box.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
        controls_box.append(&rating_box);
        controls_box.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
        controls_box.append(&reset_button);

        // Initialize chips display
        self.update_filter_chips();
    }

    /// Update the filter chips display based on current state
    fn update_filter_chips(&self) {
        Self::update_chips_display(&self.chips_box, &self.current_state.borrow());
    }

    /// Static method to update chips display for use in callbacks
    fn update_chips_display(chips_box: &gtk4::FlowBox, state: &FilterState) {
        // Clear existing chips
        while let Some(child) = chips_box.first_child() {
            chips_box.remove(&child);
        }

        // Add watch status chip (only if not "All")
        if !matches!(state.watch_status, WatchStatus::All) {
            let label = match state.watch_status {
                WatchStatus::Unwatched => "Unwatched",
                WatchStatus::Watched => "Watched",
                WatchStatus::InProgress => "In Progress",
                _ => "",
            };
            if !label.is_empty() {
                Self::add_chip_to_box(chips_box, label);
            }
        }

        // Add genre chips
        for genre in &state.genres {
            Self::add_chip_to_box(chips_box, genre);
        }

        // Add year range chip
        if let Some((from, to)) = state.years {
            let label = if from == to {
                format!("{}", from)
            } else {
                format!("{}-{}", from, to)
            };
            Self::add_chip_to_box(chips_box, &label);
        }

        // Add rating chip
        if let Some(rating) = state.min_rating {
            let label = format!("Rating ≥ {:.1}", rating);
            Self::add_chip_to_box(chips_box, &label);
        }
    }

    /// Add a simple display chip to the box (without removal functionality for now)
    fn add_chip_to_box(chips_box: &gtk4::FlowBox, label: &str) {
        let chip_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(4)
            .css_classes(vec!["pill".to_string()])
            .build();

        let chip_label = gtk4::Label::builder().label(label).build();

        chip_box.append(&chip_label);

        let flow_child = gtk4::FlowBoxChild::new();
        flow_child.set_child(Some(&chip_box));
        chips_box.append(&flow_child);
    }
}

impl Default for LibraryFilters {
    fn default() -> Self {
        Self::new()
    }
}
