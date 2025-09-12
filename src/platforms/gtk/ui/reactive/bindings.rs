use crate::core::viewmodels::property::{ComputedProperty, Property};
use crate::utils::{ImageLoader, ImageSize};
use gtk4::glib;
use gtk4::prelude::*;
use std::sync::Arc;
use tracing::error;

use once_cell::sync::Lazy;
static IMAGE_LOADER: Lazy<ImageLoader> =
    Lazy::new(|| ImageLoader::new().expect("Failed to create ImageLoader"));

/// Handle for managing the lifecycle of reactive bindings
/// Automatically cleans up background tasks when dropped
#[derive(Debug)]
pub struct BindingHandle {
    _task_handle: glib::JoinHandle<()>,
}

impl BindingHandle {
    pub fn new(task_handle: glib::JoinHandle<()>) -> Self {
        Self {
            _task_handle: task_handle,
        }
    }
}

/// Bind a text widget to a ComputedProperty
pub fn bind_text_to_computed_property<T, F>(
    widget: &gtk4::Label,
    computed_property: ComputedProperty<T>,
    transform: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> String + Send + 'static,
{
    let widget_weak = widget.downgrade();
    let mut subscriber = computed_property.subscribe();

    // Set initial value
    if let Some(widget) = widget_weak.upgrade() {
        let initial_value = transform(&computed_property.get_sync());
        widget.set_text(&initial_value);
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(widget) = widget_weak.upgrade() {
                let value = computed_property.get().await;
                let text = transform(&value);
                widget.set_text(&text);
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

/// Bind widget visibility to a ComputedProperty
pub fn bind_visibility_to_computed_property<T, F>(
    widget: &impl WidgetExt,
    computed_property: ComputedProperty<T>,
    transform: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> bool + Send + 'static,
{
    let widget_weak = widget.downgrade();
    let mut subscriber = computed_property.subscribe();

    // Set initial value
    if let Some(widget) = widget_weak.upgrade() {
        let initial_value = transform(&computed_property.get_sync());
        widget.set_visible(initial_value);
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(widget) = widget_weak.upgrade() {
                let value = computed_property.get().await;
                let visible = transform(&value);
                widget.set_visible(visible);
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

pub fn bind_text_to_property<T, F>(
    widget: &gtk4::Label,
    property: Property<T>,
    transform: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> String + Send + 'static,
{
    let widget_weak = widget.downgrade();
    let mut subscriber = property.subscribe();

    // Set initial value
    if let Some(widget) = widget_weak.upgrade() {
        let initial_value = transform(&property.get_sync());
        widget.set_text(&initial_value);
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(widget) = widget_weak.upgrade() {
                let value = property.get().await;
                let text = transform(&value);
                widget.set_text(&text);
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

pub fn bind_visibility_to_property<T, F>(
    widget: &impl WidgetExt,
    property: Property<T>,
    transform: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> bool + Send + 'static,
{
    let widget_weak = widget.downgrade();
    let mut subscriber = property.subscribe();

    // Set initial value
    if let Some(widget) = widget_weak.upgrade() {
        let initial_value = transform(&property.get_sync());
        widget.set_visible(initial_value);
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(widget) = widget_weak.upgrade() {
                let value = property.get().await;
                let visible = transform(&value);
                widget.set_visible(visible);
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

pub fn bind_sensitivity_to_property<T, F>(
    widget: &impl WidgetExt,
    property: Property<T>,
    transform: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> bool + Send + 'static,
{
    let widget_weak = widget.downgrade();
    let mut subscriber = property.subscribe();

    // Set initial value
    if let Some(widget) = widget_weak.upgrade() {
        let initial_value = transform(&property.get_sync());
        widget.set_sensitive(initial_value);
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(widget) = widget_weak.upgrade() {
                let value = property.get().await;
                let sensitive = transform(&value);
                widget.set_sensitive(sensitive);
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

pub fn bind_label_to_property<T, F>(
    widget: &gtk4::Label,
    property: Property<T>,
    transform: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> String + Send + 'static,
{
    let widget_weak = widget.downgrade();
    let mut subscriber = property.subscribe();

    // Set initial value
    if let Some(widget) = widget_weak.upgrade() {
        let initial_value = transform(&property.get_sync());
        widget.set_label(&initial_value);
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(widget) = widget_weak.upgrade() {
                let value = property.get().await;
                let text = transform(&value);
                widget.set_label(&text);
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

pub fn bind_image_to_property<T, F>(
    widget: &gtk4::Picture,
    property: Property<T>,
    transform: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> Option<String> + Send + 'static,
{
    let widget_weak = widget.downgrade();
    let mut subscriber = property.subscribe();

    // Add appropriate CSS classes for the image type
    if widget.widget_name() == "picture" {
        // Determine image type based on context and add appropriate CSS
        let parent = widget.parent();
        if let Some(parent_widget) = parent {
            let parent_name = parent_widget.widget_name();
            if parent_name.contains("backdrop") {
                widget.add_css_class("show-backdrop");
            } else {
                widget.add_css_class("show-poster");
            }
        }
    }

    // Set initial value
    if let Some(widget) = widget_weak.upgrade() {
        let initial_value = property.get_sync();
        if let Some(url) = transform(&initial_value) {
            glib::spawn_future_local({
                let widget = widget.clone();
                async move {
                    match IMAGE_LOADER.load_image(&url, ImageSize::Large).await {
                        Ok(texture) => {
                            widget.set_paintable(Some(&texture));
                        }
                        Err(e) => {
                            error!("Failed to load image: {}", e);
                            widget.set_paintable(gtk4::gdk::Paintable::NONE);
                        }
                    }
                }
            });
        } else {
            widget.set_paintable(gtk4::gdk::Paintable::NONE);
        }
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(widget) = widget_weak.upgrade() {
                let value = property.get().await;
                if let Some(url) = transform(&value) {
                    // Load image asynchronously
                    glib::spawn_future_local({
                        let widget = widget.clone();
                        async move {
                            match IMAGE_LOADER.load_image(&url, ImageSize::Large).await {
                                Ok(texture) => {
                                    widget.set_paintable(Some(&texture));
                                }
                                Err(e) => {
                                    error!("Failed to load image: {}", e);
                                    widget.set_paintable(gtk4::gdk::Paintable::NONE);
                                }
                            }
                        }
                    });
                } else {
                    widget.set_paintable(gtk4::gdk::Paintable::NONE);
                }
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

pub fn bind_icon_to_property<T, F>(
    widget: &gtk4::Button,
    property: Property<T>,
    transform: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> String + Send + 'static,
{
    let widget_weak = widget.downgrade();
    let mut subscriber = property.subscribe();

    // Set initial value
    if let Some(widget) = widget_weak.upgrade() {
        let initial_value = transform(&property.get_sync());
        widget.set_icon_name(&initial_value);
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(widget) = widget_weak.upgrade() {
                let value = property.get().await;
                let icon_name = transform(&value);
                widget.set_icon_name(&icon_name);
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

/// Bind a Scale widget's value to a ComputedProperty
pub fn bind_value_to_computed_property<T, F>(
    widget: &gtk4::Scale,
    computed_property: ComputedProperty<T>,
    transform: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> f64 + Send + 'static,
{
    let widget_weak = widget.downgrade();
    let mut subscriber = computed_property.subscribe();

    // Set initial value
    if let Some(widget) = widget_weak.upgrade() {
        let initial_value = transform(&computed_property.get_sync());
        widget.set_value(initial_value);
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(widget) = widget_weak.upgrade() {
                let value = computed_property.get().await;
                let scale_value = transform(&value);
                widget.set_value(scale_value);
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

pub fn bind_value_to_property<T, F>(
    widget: &gtk4::Scale,
    property: Property<T>,
    transform: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> f64 + Send + 'static,
{
    let widget_weak = widget.downgrade();
    let mut subscriber = property.subscribe();

    // Set initial value
    if let Some(widget) = widget_weak.upgrade() {
        let initial_value = transform(&property.get_sync());
        widget.set_value(initial_value);
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(widget) = widget_weak.upgrade() {
                let value = property.get().await;
                let scale_value = transform(&value);
                widget.set_value(scale_value);
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

/// Binds a FlowBox to a property containing a collection of items
/// Automatically updates the FlowBox children when the collection changes
pub fn bind_flowbox_to_property<T, F, W>(
    flowbox: &gtk4::FlowBox,
    property: Property<Vec<T>>,
    create_widget: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> W + Send + 'static,
    W: IsA<gtk4::Widget>,
{
    let flowbox_weak = flowbox.downgrade();
    let mut subscriber = property.subscribe();

    // Set initial collection
    if let Some(flowbox) = flowbox_weak.upgrade() {
        let initial_items = property.get_sync();
        update_flowbox_children(&flowbox, &initial_items, &create_widget);
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(flowbox) = flowbox_weak.upgrade() {
                let items = property.get().await;
                update_flowbox_children(&flowbox, &items, &create_widget);
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

/// Helper function to update FlowBox children with new collection
fn update_flowbox_children<T, F, W>(flowbox: &gtk4::FlowBox, items: &[T], create_widget: &F)
where
    T: Clone,
    F: Fn(&T) -> W,
    W: IsA<gtk4::Widget>,
{
    // Clear existing children
    while let Some(child) = flowbox.first_child() {
        flowbox.remove(&child);
    }

    // Add new children
    for item in items {
        let widget = create_widget(item);
        flowbox.insert(&widget, -1);
    }
}

/// Binds an Image widget icon name to a property  
pub fn bind_image_icon_to_property<T, F>(
    widget: &gtk4::Image,
    property: Property<T>,
    transform: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> String + Send + 'static,
{
    let widget_weak = widget.downgrade();
    let mut subscriber = property.subscribe();

    // Set initial value
    if let Some(widget) = widget_weak.upgrade() {
        let initial_value = transform(&property.get_sync());
        widget.set_icon_name(Some(&initial_value));
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(widget) = widget_weak.upgrade() {
                let value = property.get().await;
                let icon_name = transform(&value);
                widget.set_icon_name(Some(&icon_name));
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

/// Binds a Spinner widget's visibility and spinning state to a property
pub fn bind_spinner_to_property<T, F>(
    widget: &gtk4::Spinner,
    property: Property<T>,
    transform: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> bool + Send + 'static,
{
    let widget_weak = widget.downgrade();
    let mut subscriber = property.subscribe();

    // Set initial value
    if let Some(widget) = widget_weak.upgrade() {
        let initial_value = transform(&property.get_sync());
        widget.set_visible(initial_value);
        widget.set_spinning(initial_value);
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(widget) = widget_weak.upgrade() {
                let value = property.get().await;
                let should_spin = transform(&value);
                widget.set_visible(should_spin);
                widget.set_spinning(should_spin);
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

/// Binds a widget CSS class to a property (adds/removes based on boolean condition)
pub fn bind_css_class_to_property<T, F>(
    widget: &impl WidgetExt,
    property: Property<T>,
    css_class: &str,
    should_add: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> bool + Send + 'static,
{
    let widget_weak = widget.downgrade();
    let mut subscriber = property.subscribe();
    let css_class = css_class.to_string();

    // Set initial value
    if let Some(widget) = widget_weak.upgrade() {
        let initial_value = property.get_sync();
        if should_add(&initial_value) {
            widget.add_css_class(&css_class);
        } else {
            widget.remove_css_class(&css_class);
        }
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(widget) = widget_weak.upgrade() {
                let value = property.get().await;
                if should_add(&value) {
                    widget.add_css_class(&css_class);
                } else {
                    widget.remove_css_class(&css_class);
                }
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

/// Binds a widget's tooltip text to a property
pub fn bind_tooltip_to_property<T, F>(
    widget: &impl WidgetExt,
    property: Property<T>,
    transform: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> Option<String> + Send + 'static,
{
    let widget_weak = widget.downgrade();
    let mut subscriber = property.subscribe();

    // Set initial value
    if let Some(widget) = widget_weak.upgrade() {
        let initial_value = transform(&property.get_sync());
        widget.set_tooltip_text(initial_value.as_deref());
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(widget) = widget_weak.upgrade() {
                let value = property.get().await;
                let tooltip = transform(&value);
                widget.set_tooltip_text(tooltip.as_deref());
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

/// Binds a DropDown widget to a property containing a collection of items
/// Automatically updates the dropdown model when the collection changes
pub fn bind_dropdown_to_property<T, F>(
    dropdown: &gtk4::DropDown,
    property: Property<Vec<T>>,
    transform: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> String + Send + 'static,
{
    let dropdown_weak = dropdown.downgrade();
    let mut subscriber = property.subscribe();

    // Set initial value
    if let Some(dropdown) = dropdown_weak.upgrade() {
        let initial_items = property.get_sync();
        update_dropdown_model(&dropdown, &initial_items, &transform);
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(dropdown) = dropdown_weak.upgrade() {
                let items = property.get().await;
                update_dropdown_model(&dropdown, &items, &transform);
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

/// Helper function to update DropDown model with new collection
fn update_dropdown_model<T, F>(dropdown: &gtk4::DropDown, items: &[T], transform: &F)
where
    T: Clone,
    F: Fn(&T) -> String,
{
    let string_list = gtk4::StringList::new(&[]);
    for item in items {
        string_list.append(&transform(item));
    }

    // Set the new model
    dropdown.set_model(Some(&string_list));

    // Clear any previous expression to use default string display
    dropdown.set_expression(gtk4::Expression::NONE);

    // Set a reasonable width if not already set
    if dropdown.width_request() <= 0 {
        dropdown.set_width_request(140);
    }

    // Select the first item by default if there are items
    if string_list.n_items() > 0 {
        dropdown.set_selected(0);
    }
}

// Library-specific reactive binding functions

/// Binds a FlowBox to display items from a reactive property
///
/// This replaces manual display_media_items() and differential_update_items()
/// with a declarative binding that automatically updates the FlowBox when
/// the property changes.
pub fn bind_flowbox_to_media_items<F>(
    flow_box: &gtk4::FlowBox,
    items_property: &crate::core::viewmodels::Property<Vec<crate::models::MediaItem>>,
    card_factory: F,
) -> BindingHandle
where
    F: Fn(
            &crate::models::MediaItem,
            crate::utils::ImageSize,
        ) -> crate::platforms::gtk::ui::pages::library::MediaCard
        + Clone
        + 'static,
{
    use std::sync::Arc;
    use tracing::trace;

    let flow_box_weak = flow_box.downgrade();
    let card_factory = Arc::new(card_factory);

    // Subscribe to property changes
    let mut subscriber = items_property.subscribe();
    let items_property_clone = items_property.clone();
    let card_factory_clone = card_factory.clone();

    let handle = glib::spawn_future_local(async move {
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

    BindingHandle::new(handle)
}

/// Creates a two-way binding between a search entry and a property
///
/// This enables the search entry to both update the property when the user types,
/// and be updated when the property changes programmatically.
pub fn bind_search_entry_two_way(
    entry: &gtk4::SearchEntry,
    property: &crate::core::viewmodels::Property<String>,
) -> BindingHandle {
    use gtk4::prelude::*;

    let entry_weak = entry.downgrade();
    let property_clone = property.clone();

    // Property -> Widget binding
    let mut subscriber = property.subscribe();
    let handle1 = glib::spawn_future_local(async move {
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

    BindingHandle::new(handle1)
}

// Helper functions for FlowBox media items binding

/// Updates FlowBox content using reactive diffing
///
/// This replaces the complex differential_update_items() logic with a
/// simpler reactive approach that focuses on data-driven updates.
fn update_flowbox_content<F>(
    flow_box: &gtk4::FlowBox,
    new_items: &[crate::models::MediaItem],
    card_factory: &Arc<F>,
) where
    F: Fn(
        &crate::models::MediaItem,
        crate::utils::ImageSize,
    ) -> crate::platforms::gtk::ui::pages::library::MediaCard,
{
    use tracing::trace;

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
            && let Some(card) = fc.child().and_then(|w| {
                w.downcast::<crate::platforms::gtk::ui::pages::library::MediaCard>()
                    .ok()
            })
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
    current_items: &[(gtk4::Widget, crate::models::MediaItem)],
    new_items: &[crate::models::MediaItem],
) -> bool {
    use std::collections::HashSet;

    // Use full refresh if:
    // 1. No current items (initial load)
    // 2. More than 50% of items changed
    // 3. Order changed significantly

    if current_items.is_empty() {
        return true;
    }

    let current_ids: HashSet<String> = current_items
        .iter()
        .map(|(_, item)| item.id().to_string())
        .collect();
    let new_ids: HashSet<String> = new_items.iter().map(|item| item.id().to_string()).collect();

    let changes = current_ids.symmetric_difference(&new_ids).count();
    let change_ratio = changes as f32 / current_items.len() as f32;

    change_ratio > 0.5
}

/// Performs full refresh of FlowBox content
fn full_refresh_flowbox<F>(
    flow_box: &gtk4::FlowBox,
    items: &[crate::models::MediaItem],
    card_factory: &Arc<F>,
) where
    F: Fn(
        &crate::models::MediaItem,
        crate::utils::ImageSize,
    ) -> crate::platforms::gtk::ui::pages::library::MediaCard,
{
    use gtk4::prelude::*;

    // Clear existing children
    while let Some(child) = flow_box.first_child() {
        flow_box.remove(&child);
    }

    // Add new children
    for item in items {
        let card = card_factory(item, crate::utils::ImageSize::Medium);
        let child = gtk4::FlowBoxChild::new();
        child.set_child(Some(&card));
        flow_box.append(&child);

        // Trigger image load
        card.trigger_load(crate::utils::ImageSize::Medium);
    }
}

/// Performs differential update of FlowBox content
fn differential_update_flowbox<F>(
    flow_box: &gtk4::FlowBox,
    current_items: &[(gtk4::Widget, crate::models::MediaItem)],
    new_items: &[crate::models::MediaItem],
    card_factory: &Arc<F>,
) where
    F: Fn(
        &crate::models::MediaItem,
        crate::utils::ImageSize,
    ) -> crate::platforms::gtk::ui::pages::library::MediaCard,
{
    use gtk4::prelude::*;
    use std::collections::{HashMap, HashSet};

    let current_ids: HashSet<String> = current_items
        .iter()
        .map(|(_, item)| item.id().to_string())
        .collect();
    let new_ids: HashSet<String> = new_items.iter().map(|item| item.id().to_string()).collect();

    // Remove items that are no longer in new list
    let to_remove: HashSet<String> = current_ids.difference(&new_ids).cloned().collect();

    for (widget, item) in current_items {
        if to_remove.contains(&item.id().to_string()) {
            flow_box.remove(widget);
        }
    }

    // Add new items that weren't in current list
    let to_add: HashSet<String> = new_ids.difference(&current_ids).cloned().collect();

    for item in new_items {
        if to_add.contains(&item.id().to_string()) {
            let card = card_factory(item, crate::utils::ImageSize::Medium);
            let child = gtk4::FlowBoxChild::new();
            child.set_child(Some(&card));
            flow_box.append(&child);

            // Trigger image load
            card.trigger_load(crate::utils::ImageSize::Medium);
        }
    }

    // Update content for existing items (in case of progress changes, etc.)
    let new_by_id: HashMap<String, &crate::models::MediaItem> = new_items
        .iter()
        .map(|item| (item.id().to_string(), item))
        .collect();

    let mut child = flow_box.first_child();
    while let Some(flow_child) = child {
        let next = flow_child.next_sibling();

        if let Some(fc) = flow_child.downcast_ref::<gtk4::FlowBoxChild>()
            && let Some(card) = fc.child().and_then(|w| {
                w.downcast::<crate::platforms::gtk::ui::pages::library::MediaCard>()
                    .ok()
            })
        {
            let card_id = card.media_item().id().to_string();
            if let Some(new_item) = new_by_id.get(&card_id) {
                card.update_content((*new_item).clone());
            }
        }

        child = next;
    }
}
