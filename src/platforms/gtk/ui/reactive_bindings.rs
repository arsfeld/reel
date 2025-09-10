use gtk4::{glib, prelude::*};
use std::sync::Arc;
use tracing::trace;

use crate::core::viewmodels::Property;
use crate::models::MediaItem;
use crate::platforms::gtk::ui::pages::library::MediaCard;
use crate::utils::ImageSize;

/// Reactive binding utilities for GTK widgets
///
/// This module provides utilities to create declarative bindings between
/// reactive properties and GTK widgets, eliminating manual DOM manipulation.

/// Binds a FlowBox to display items from a reactive property
///
/// This replaces manual display_media_items() and differential_update_items()
/// with a declarative binding that automatically updates the FlowBox when
/// the property changes.
pub fn bind_flowbox_to_media_items<F>(
    flow_box: &gtk4::FlowBox,
    items_property: &Property<Vec<MediaItem>>,
    card_factory: F,
) where
    F: Fn(&MediaItem, ImageSize) -> MediaCard + Clone + 'static,
{
    let flow_box_weak = flow_box.downgrade();
    let card_factory = Arc::new(card_factory);

    // Subscribe to property changes
    let mut subscriber = items_property.subscribe();
    let items_property_clone = items_property.clone();
    let card_factory_clone = card_factory.clone();

    glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(flow_box) = flow_box_weak.upgrade() {
                let items = items_property_clone.get_sync();
                trace!(
                    "[REACTIVE] FlowBox binding updating with {} items",
                    items.len()
                );

                // Use reactive differ to update FlowBox content
                update_flowbox_content(&flow_box, &items, &card_factory_clone);
            }
        }
    });

    // Set initial state
    let initial_items = items_property.get_sync();
    update_flowbox_content(flow_box, &initial_items, &card_factory);
}

/// Updates FlowBox content using reactive diffing
///
/// This replaces the complex differential_update_items() logic with a
/// simpler reactive approach that focuses on data-driven updates.
fn update_flowbox_content<F>(
    flow_box: &gtk4::FlowBox,
    new_items: &[MediaItem],
    card_factory: &Arc<F>,
) where
    F: Fn(&MediaItem, ImageSize) -> MediaCard,
{
    trace!(
        "[REACTIVE] Updating FlowBox content with {} items",
        new_items.len()
    );

    if new_items.is_empty() {
        // Clear all children
        while let Some(child) = flow_box.first_child() {
            flow_box.remove(&child);
        }
        return;
    }

    // Build current items map for comparison
    let mut current_items = Vec::new();
    let mut child = flow_box.first_child();
    while let Some(flow_child) = child {
        if let Some(fc) = flow_child.downcast_ref::<gtk4::FlowBoxChild>()
            && let Some(card) = fc.child().and_then(|w| w.downcast::<MediaCard>().ok())
        {
            current_items.push((flow_child.clone(), card.media_item()));
        }
        child = flow_child.next_sibling();
    }

    // Simple strategy: if lists are significantly different, do full refresh
    if should_do_full_refresh(&current_items, new_items) {
        trace!("[REACTIVE] Doing full refresh");
        full_refresh_flowbox(flow_box, new_items, card_factory);
    } else {
        trace!("[REACTIVE] Doing differential update");
        differential_update_flowbox(flow_box, &current_items, new_items, card_factory);
    }
}

/// Determines if we should do a full refresh vs differential update
fn should_do_full_refresh(
    current_items: &[(gtk4::Widget, MediaItem)],
    new_items: &[MediaItem],
) -> bool {
    // Use full refresh if:
    // 1. No current items (initial load)
    // 2. More than 50% of items changed
    // 3. Order changed significantly

    if current_items.is_empty() {
        return true;
    }

    let current_ids: std::collections::HashSet<String> = current_items
        .iter()
        .map(|(_, item)| item.id().to_string())
        .collect();
    let new_ids: std::collections::HashSet<String> =
        new_items.iter().map(|item| item.id().to_string()).collect();

    let changes = current_ids.symmetric_difference(&new_ids).count();
    let change_ratio = changes as f32 / current_items.len() as f32;

    change_ratio > 0.5
}

/// Performs full refresh of FlowBox content
fn full_refresh_flowbox<F>(flow_box: &gtk4::FlowBox, items: &[MediaItem], card_factory: &Arc<F>)
where
    F: Fn(&MediaItem, ImageSize) -> MediaCard,
{
    // Clear existing children
    while let Some(child) = flow_box.first_child() {
        flow_box.remove(&child);
    }

    // Add new children
    for item in items {
        let card = card_factory(item, ImageSize::Medium);
        let child = gtk4::FlowBoxChild::new();
        child.set_child(Some(&card));
        flow_box.append(&child);

        // Trigger image load
        card.trigger_load(ImageSize::Medium);
    }
}

/// Performs differential update of FlowBox content
fn differential_update_flowbox<F>(
    flow_box: &gtk4::FlowBox,
    current_items: &[(gtk4::Widget, MediaItem)],
    new_items: &[MediaItem],
    card_factory: &Arc<F>,
) where
    F: Fn(&MediaItem, ImageSize) -> MediaCard,
{
    let current_ids: std::collections::HashSet<String> = current_items
        .iter()
        .map(|(_, item)| item.id().to_string())
        .collect();
    let new_ids: std::collections::HashSet<String> =
        new_items.iter().map(|item| item.id().to_string()).collect();

    // Remove items that are no longer in new list
    let to_remove: std::collections::HashSet<String> =
        current_ids.difference(&new_ids).cloned().collect();

    for (widget, item) in current_items {
        if to_remove.contains(&item.id().to_string()) {
            flow_box.remove(widget);
        }
    }

    // Add new items that weren't in current list
    let to_add: std::collections::HashSet<String> =
        new_ids.difference(&current_ids).cloned().collect();

    for item in new_items {
        if to_add.contains(&item.id().to_string()) {
            let card = card_factory(item, ImageSize::Medium);
            let child = gtk4::FlowBoxChild::new();
            child.set_child(Some(&card));
            flow_box.append(&child);

            // Trigger image load
            card.trigger_load(ImageSize::Medium);
        }
    }

    // Update content for existing items (in case of progress changes, etc.)
    let new_by_id: std::collections::HashMap<String, &MediaItem> = new_items
        .iter()
        .map(|item| (item.id().to_string(), item))
        .collect();

    let mut child = flow_box.first_child();
    while let Some(flow_child) = child {
        let next = flow_child.next_sibling();

        if let Some(fc) = flow_child.downcast_ref::<gtk4::FlowBoxChild>()
            && let Some(card) = fc.child().and_then(|w| w.downcast::<MediaCard>().ok())
        {
            let card_id = card.media_item().id().to_string();
            if let Some(new_item) = new_by_id.get(&card_id) {
                card.update_content((*new_item).clone());
            }
        }

        child = next;
    }
}

/// Creates a two-way binding between a search entry and a property
///
/// This enables the search entry to both update the property when the user types,
/// and be updated when the property changes programmatically.
pub fn bind_search_entry_two_way(entry: &gtk4::SearchEntry, property: &Property<String>) {
    let entry_weak = entry.downgrade();
    let property_clone = property.clone();

    // Property -> Widget binding
    let mut subscriber = property.subscribe();
    glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(entry) = entry_weak.upgrade() {
                let value = property_clone.get_sync();
                if entry.text().as_str() != value {
                    entry.set_text(&value);
                }
            }
        }
    });

    // Widget -> Property binding
    let property_for_signal = property.clone();
    entry.connect_search_changed(move |entry| {
        let text = entry.text().to_string();
        let property = property_for_signal.clone();
        glib::spawn_future_local(async move {
            property.set(text).await;
        });
    });

    // Set initial value
    let initial_value = property.get_sync();
    entry.set_text(&initial_value);
}

/// Binds a widget's visibility to a boolean property
pub fn bind_visibility(widget: &impl IsA<gtk4::Widget>, property: &Property<bool>) {
    let widget_weak = widget.downgrade();
    let mut subscriber = property.subscribe();
    let property_clone = property.clone();

    glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(widget) = widget_weak.upgrade() {
                let visible = property_clone.get_sync();
                widget.set_visible(visible);
            }
        }
    });

    // Set initial state
    let initial_visible = property.get_sync();
    widget.set_visible(initial_visible);
}
