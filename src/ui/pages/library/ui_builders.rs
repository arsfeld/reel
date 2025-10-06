use gtk::prelude::*;
use relm4::prelude::*;

use super::LibraryPage;
use super::messages::LibraryPageInput;
use super::types::WatchStatus;

impl LibraryPage {
    /// Update genre popover with filter options
    pub(super) fn update_genre_popover(
        &self,
        popover: &gtk::Popover,
        sender: AsyncComponentSender<Self>,
    ) {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_width_request(250);

        // Header with clear button
        let header_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let header_label = gtk::Label::new(Some("Filter by Genre"));
        header_label.set_halign(gtk::Align::Start);
        header_label.set_hexpand(true);
        header_label.add_css_class("heading");
        header_box.append(&header_label);

        if !self.selected_genres.is_empty() {
            let clear_button = gtk::Button::with_label("Clear");
            clear_button.add_css_class("flat");
            let sender_clone = sender.clone();
            clear_button.connect_clicked(move |_| {
                sender_clone.input(LibraryPageInput::ClearGenreFilters);
            });
            header_box.append(&clear_button);
        }
        content.append(&header_box);

        // Separator
        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        content.append(&separator);

        // Scrolled window for genre list
        let scrolled_window = gtk::ScrolledWindow::new();
        scrolled_window.set_max_content_height(400);
        scrolled_window.set_propagate_natural_height(true);
        scrolled_window.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

        let genre_box = gtk::Box::new(gtk::Orientation::Vertical, 2);

        // Add checkboxes for each genre
        for genre in &self.available_genres {
            let is_selected = self.selected_genres.contains(genre);
            let check_button = gtk::CheckButton::with_label(genre);
            check_button.set_active(is_selected);
            check_button.add_css_class("flat");

            let genre_clone = genre.clone();
            let sender_clone = sender.clone();
            check_button.connect_toggled(move |_| {
                sender_clone.input(LibraryPageInput::ToggleGenreFilter(genre_clone.clone()));
            });

            genre_box.append(&check_button);
        }

        scrolled_window.set_child(Some(&genre_box));
        content.append(&scrolled_window);

        popover.set_child(Some(&content));
    }

    /// Update year range popover with filter options
    pub(super) fn update_year_popover(
        &self,
        popover: &gtk::Popover,
        sender: AsyncComponentSender<Self>,
    ) {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_width_request(250);

        // Header with clear button
        let header_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let header_label = gtk::Label::new(Some("Filter by Year"));
        header_label.set_halign(gtk::Align::Start);
        header_label.set_hexpand(true);
        header_label.add_css_class("heading");
        header_box.append(&header_label);

        if self.selected_min_year.is_some() || self.selected_max_year.is_some() {
            let clear_button = gtk::Button::with_label("Clear");
            clear_button.add_css_class("flat");
            let sender_clone = sender.clone();
            clear_button.connect_clicked(move |_| {
                sender_clone.input(LibraryPageInput::ClearYearRange);
            });
            header_box.append(&clear_button);
        }
        content.append(&header_box);

        // Separator
        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        content.append(&separator);

        // Year range info
        if let (Some(min), Some(max)) = (self.min_year, self.max_year) {
            let info_label = gtk::Label::new(Some(&format!("Available: {} - {}", min, max)));
            info_label.set_halign(gtk::Align::Start);
            info_label.add_css_class("dim-label");
            content.append(&info_label);

            // Min year input
            let min_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            min_box.set_margin_top(6);
            let min_label = gtk::Label::new(Some("From:"));
            min_label.set_width_request(50);
            min_label.set_halign(gtk::Align::Start);
            min_box.append(&min_label);

            let min_spinbutton = gtk::SpinButton::with_range(min as f64, max as f64, 1.0);
            min_spinbutton.set_value(self.selected_min_year.unwrap_or(min) as f64);
            min_spinbutton.set_hexpand(true);
            min_box.append(&min_spinbutton);
            content.append(&min_box);

            // Max year input
            let max_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            max_box.set_margin_top(6);
            let max_label = gtk::Label::new(Some("To:"));
            max_label.set_width_request(50);
            max_label.set_halign(gtk::Align::Start);
            max_box.append(&max_label);

            let max_spinbutton = gtk::SpinButton::with_range(min as f64, max as f64, 1.0);
            max_spinbutton.set_value(self.selected_max_year.unwrap_or(max) as f64);
            max_spinbutton.set_hexpand(true);
            max_box.append(&max_spinbutton);
            content.append(&max_box);

            // Apply button
            let apply_button = gtk::Button::with_label("Apply");
            apply_button.set_margin_top(12);
            apply_button.add_css_class("suggested-action");

            let sender_clone = sender.clone();
            apply_button.connect_clicked(move |_| {
                let min_year = min_spinbutton.value() as i32;
                let max_year = max_spinbutton.value() as i32;
                sender_clone.input(LibraryPageInput::SetYearRange {
                    min: Some(min_year),
                    max: Some(max_year),
                });
            });
            content.append(&apply_button);
        }

        popover.set_child(Some(&content));
    }

    /// Update rating popover with filter options
    pub(super) fn update_rating_popover(
        &self,
        popover: &gtk::Popover,
        sender: AsyncComponentSender<Self>,
    ) {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_width_request(250);

        // Header with clear button
        let header_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let header_label = gtk::Label::new(Some("Filter by Rating"));
        header_label.set_halign(gtk::Align::Start);
        header_label.set_hexpand(true);
        header_label.add_css_class("heading");
        header_box.append(&header_label);

        if self.min_rating.is_some() {
            let clear_button = gtk::Button::with_label("Clear");
            clear_button.add_css_class("flat");
            let sender_clone = sender.clone();
            clear_button.connect_clicked(move |_| {
                sender_clone.input(LibraryPageInput::ClearRatingFilter);
            });
            header_box.append(&clear_button);
        }
        content.append(&header_box);

        // Separator
        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        content.append(&separator);

        // Info label
        let info_label = gtk::Label::new(Some("Minimum rating (0-10):"));
        info_label.set_halign(gtk::Align::Start);
        info_label.add_css_class("dim-label");
        content.append(&info_label);

        // Rating scale
        let rating_scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 10.0, 0.5);
        rating_scale.set_value(self.min_rating.unwrap_or(0.0) as f64);
        rating_scale.set_draw_value(true);
        rating_scale.set_value_pos(gtk::PositionType::Right);
        rating_scale.set_digits(1);
        rating_scale.set_hexpand(true);

        // Add marks for better UX
        for i in 0..=10 {
            rating_scale.add_mark(i as f64, gtk::PositionType::Bottom, None);
        }

        content.append(&rating_scale);

        // Apply button
        let apply_button = gtk::Button::with_label("Apply");
        apply_button.set_margin_top(12);
        apply_button.add_css_class("suggested-action");

        let sender_clone = sender.clone();
        apply_button.connect_clicked(move |_| {
            let rating = rating_scale.value() as f32;
            sender_clone.input(LibraryPageInput::SetRatingFilter(if rating > 0.0 {
                Some(rating)
            } else {
                None
            }));
        });
        content.append(&apply_button);

        popover.set_child(Some(&content));
    }

    /// Update watch status popover with filter options
    pub(super) fn update_watch_status_popover(
        &self,
        popover: &gtk::Popover,
        sender: AsyncComponentSender<Self>,
    ) {
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_width_request(250);

        // Header with clear button
        let header_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let header_label = gtk::Label::new(Some("Filter by Watch Status"));
        header_label.set_halign(gtk::Align::Start);
        header_label.set_hexpand(true);
        header_label.add_css_class("heading");
        header_box.append(&header_label);

        if self.watch_status_filter != WatchStatus::All {
            let clear_button = gtk::Button::with_label("Clear");
            clear_button.add_css_class("flat");
            let sender_clone = sender.clone();
            clear_button.connect_clicked(move |_| {
                sender_clone.input(LibraryPageInput::ClearWatchStatusFilter);
            });
            header_box.append(&clear_button);
        }
        content.append(&header_box);

        // Separator
        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        content.append(&separator);

        // Radio buttons for watch status
        let radio_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        radio_box.set_margin_top(6);

        // All items option
        let all_radio = gtk::CheckButton::with_label("All Items");
        all_radio.set_active(self.watch_status_filter == WatchStatus::All);
        let sender_clone = sender.clone();
        all_radio.connect_toggled(move |btn| {
            if btn.is_active() {
                sender_clone.input(LibraryPageInput::SetWatchStatusFilter(WatchStatus::All));
            }
        });
        radio_box.append(&all_radio);

        // Watched option
        let watched_radio = gtk::CheckButton::with_label("Watched");
        watched_radio.set_group(Some(&all_radio));
        watched_radio.set_active(self.watch_status_filter == WatchStatus::Watched);
        let sender_clone = sender.clone();
        watched_radio.connect_toggled(move |btn| {
            if btn.is_active() {
                sender_clone.input(LibraryPageInput::SetWatchStatusFilter(WatchStatus::Watched));
            }
        });
        radio_box.append(&watched_radio);

        // Unwatched option
        let unwatched_radio = gtk::CheckButton::with_label("Unwatched");
        unwatched_radio.set_group(Some(&all_radio));
        unwatched_radio.set_active(self.watch_status_filter == WatchStatus::Unwatched);
        let sender_clone = sender;
        unwatched_radio.connect_toggled(move |btn| {
            if btn.is_active() {
                sender_clone.input(LibraryPageInput::SetWatchStatusFilter(
                    WatchStatus::Unwatched,
                ));
            }
        });
        radio_box.append(&unwatched_radio);

        content.append(&radio_box);

        popover.set_child(Some(&content));
    }

    /// Build and update the unified filters popover with all filter options
    pub(super) fn update_unified_filters_popover(&self, sender: AsyncComponentSender<Self>) {
        if let Some(ref popover) = self.filters_popover {
            let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
            content.set_margin_top(12);
            content.set_margin_bottom(12);
            content.set_margin_start(12);
            content.set_margin_end(12);
            content.set_width_request(320);

            // Genre Filter Section
            let genre_section = self.build_genre_filter_section(sender.clone());
            content.append(&genre_section);

            // Separator
            content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

            // Year Range Filter Section
            let year_section = self.build_year_filter_section(sender.clone());
            content.append(&year_section);

            // Separator
            content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

            // Rating Filter Section
            let rating_section = self.build_rating_filter_section(sender.clone());
            content.append(&rating_section);

            // Separator
            content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

            // Watch Status Filter Section
            let watch_status_section = self.build_watch_status_filter_section(sender.clone());
            content.append(&watch_status_section);

            popover.set_child(Some(&content));
        }
    }

    /// Build genre filter section for unified popover
    pub(super) fn build_genre_filter_section(
        &self,
        sender: AsyncComponentSender<Self>,
    ) -> gtk::Box {
        let section = gtk::Box::new(gtk::Orientation::Vertical, 6);

        // Header
        let header = gtk::Label::new(Some("Genre"));
        header.set_halign(gtk::Align::Start);
        header.add_css_class("heading");
        section.append(&header);

        // Scrolled window for genres
        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_max_content_height(200);
        scrolled.set_propagate_natural_height(true);
        scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

        let genre_box = gtk::Box::new(gtk::Orientation::Vertical, 2);

        if self.available_genres.is_empty() {
            let no_genres = gtk::Label::new(Some("No genres available"));
            no_genres.add_css_class("dim-label");
            genre_box.append(&no_genres);
        } else {
            for genre in &self.available_genres {
                let check_button = gtk::CheckButton::with_label(genre);
                check_button.set_active(self.selected_genres.contains(genre));
                check_button.add_css_class("flat");

                let genre_clone = genre.clone();
                let sender_clone = sender.clone();
                check_button.connect_toggled(move |_| {
                    sender_clone.input(LibraryPageInput::ToggleGenreFilter(genre_clone.clone()));
                });

                genre_box.append(&check_button);
            }
        }

        scrolled.set_child(Some(&genre_box));
        section.append(&scrolled);

        section
    }

    /// Build year range filter section for unified popover
    pub(super) fn build_year_filter_section(&self, sender: AsyncComponentSender<Self>) -> gtk::Box {
        let section = gtk::Box::new(gtk::Orientation::Vertical, 6);

        // Header
        let header = gtk::Label::new(Some("Year Range"));
        header.set_halign(gtk::Align::Start);
        header.add_css_class("heading");
        section.append(&header);

        // Min/Max year controls
        let year_controls = gtk::Box::new(gtk::Orientation::Horizontal, 12);

        // Min year
        let min_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let min_label = gtk::Label::new(Some("From:"));
        min_label.set_halign(gtk::Align::Start);
        min_label.add_css_class("dim-label");
        min_box.append(&min_label);

        let min_entry = gtk::SpinButton::with_range(
            self.min_year.unwrap_or(1900) as f64,
            self.max_year.unwrap_or(2100) as f64,
            1.0,
        );
        if let Some(year) = self.selected_min_year {
            min_entry.set_value(year as f64);
        } else if let Some(year) = self.min_year {
            min_entry.set_value(year as f64);
        }

        let sender_clone = sender.clone();
        let max_year = self.selected_max_year;
        min_entry.connect_value_changed(move |spin| {
            let min = Some(spin.value_as_int());
            sender_clone.input(LibraryPageInput::SetYearRange { min, max: max_year });
        });

        min_box.append(&min_entry);
        year_controls.append(&min_box);

        // Max year
        let max_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let max_label = gtk::Label::new(Some("To:"));
        max_label.set_halign(gtk::Align::Start);
        max_label.add_css_class("dim-label");
        max_box.append(&max_label);

        let max_entry = gtk::SpinButton::with_range(
            self.min_year.unwrap_or(1900) as f64,
            self.max_year.unwrap_or(2100) as f64,
            1.0,
        );
        if let Some(year) = self.selected_max_year {
            max_entry.set_value(year as f64);
        } else if let Some(year) = self.max_year {
            max_entry.set_value(year as f64);
        }

        let sender_clone = sender.clone();
        let min_year = self.selected_min_year;
        max_entry.connect_value_changed(move |spin| {
            let max = Some(spin.value_as_int());
            sender_clone.input(LibraryPageInput::SetYearRange { min: min_year, max });
        });

        max_box.append(&max_entry);
        year_controls.append(&max_box);

        section.append(&year_controls);

        section
    }

    /// Build rating filter section for unified popover
    pub(super) fn build_rating_filter_section(
        &self,
        sender: AsyncComponentSender<Self>,
    ) -> gtk::Box {
        let section = gtk::Box::new(gtk::Orientation::Vertical, 6);

        // Header
        let header = gtk::Label::new(Some("Minimum Rating"));
        header.set_halign(gtk::Align::Start);
        header.add_css_class("heading");
        section.append(&header);

        // Rating scale
        let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 10.0, 0.5);
        scale.set_draw_value(true);
        scale.set_value_pos(gtk::PositionType::Right);

        if let Some(rating) = self.min_rating {
            scale.set_value(rating as f64);
        } else {
            scale.set_value(0.0);
        }

        let sender_clone = sender.clone();
        scale.connect_value_changed(move |scale| {
            let value = scale.value() as f32;
            let rating = if value > 0.0 { Some(value) } else { None };
            sender_clone.input(LibraryPageInput::SetRatingFilter(rating));
        });

        section.append(&scale);

        section
    }

    /// Build watch status filter section for unified popover
    pub(super) fn build_watch_status_filter_section(
        &self,
        sender: AsyncComponentSender<Self>,
    ) -> gtk::Box {
        let section = gtk::Box::new(gtk::Orientation::Vertical, 6);

        // Header
        let header = gtk::Label::new(Some("Watch Status"));
        header.set_halign(gtk::Align::Start);
        header.add_css_class("heading");
        section.append(&header);

        // Radio buttons
        let all_radio = gtk::CheckButton::with_label("All");
        all_radio.set_active(self.watch_status_filter == WatchStatus::All);

        let watched_radio = gtk::CheckButton::with_label("Watched");
        watched_radio.set_group(Some(&all_radio));
        watched_radio.set_active(self.watch_status_filter == WatchStatus::Watched);

        let unwatched_radio = gtk::CheckButton::with_label("Unwatched");
        unwatched_radio.set_group(Some(&all_radio));
        unwatched_radio.set_active(self.watch_status_filter == WatchStatus::Unwatched);

        let sender_clone = sender.clone();
        all_radio.connect_toggled(move |btn| {
            if btn.is_active() {
                sender_clone.input(LibraryPageInput::SetWatchStatusFilter(WatchStatus::All));
            }
        });

        let sender_clone = sender.clone();
        watched_radio.connect_toggled(move |btn| {
            if btn.is_active() {
                sender_clone.input(LibraryPageInput::SetWatchStatusFilter(WatchStatus::Watched));
            }
        });

        let sender_clone = sender.clone();
        unwatched_radio.connect_toggled(move |btn| {
            if btn.is_active() {
                sender_clone.input(LibraryPageInput::SetWatchStatusFilter(
                    WatchStatus::Unwatched,
                ));
            }
        });

        section.append(&all_radio);
        section.append(&watched_radio);
        section.append(&unwatched_radio);

        section
    }
}
