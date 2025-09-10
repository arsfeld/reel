use crate::core::viewmodels::property::{ComputedProperty, Property, PropertyLike};
use crate::utils::{ImageLoader, ImageSize};
use gtk4::glib;
use gtk4::prelude::*;
use tracing::error;

use once_cell::sync::Lazy;
static IMAGE_LOADER: Lazy<ImageLoader> =
    Lazy::new(|| ImageLoader::new().expect("Failed to create ImageLoader"));

/// Handle for managing the lifecycle of reactive bindings
/// Automatically cleans up background tasks when dropped
pub struct BindingHandle {
    _task_handle: glib::JoinHandle<()>,
}

impl BindingHandle {
    fn new(task_handle: glib::JoinHandle<()>) -> Self {
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

/// Binds a Box container to a property containing a collection of items
/// Automatically updates the Box children when the collection changes
pub fn bind_box_to_collection<T, F, W>(
    container: &gtk4::Box,
    property: Property<Vec<T>>,
    create_widget: F,
) -> BindingHandle
where
    T: Clone + Send + Sync + 'static,
    F: Fn(&T) -> W + Send + Sync + 'static,
    W: IsA<gtk4::Widget>,
{
    let container_weak = container.downgrade();
    let mut subscriber = property.subscribe();

    // Set initial collection
    if let Some(container) = container_weak.upgrade() {
        let initial_items = property.get_sync();
        update_box_children(&container, &initial_items, &create_widget);
    }

    let handle = glib::spawn_future_local(async move {
        while subscriber.wait_for_change().await {
            if let Some(container) = container_weak.upgrade() {
                let items = property.get().await;
                update_box_children(&container, &items, &create_widget);
            } else {
                break; // Widget destroyed, exit loop
            }
        }
    });

    BindingHandle::new(handle)
}

/// Helper function to update Box children with new collection
fn update_box_children<T, F, W>(container: &gtk4::Box, items: &[T], create_widget: &F)
where
    T: Clone,
    F: Fn(&T) -> W,
    W: IsA<gtk4::Widget>,
{
    // Clear existing children
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }

    // Add new children
    for item in items {
        let widget = create_widget(item);
        container.append(&widget);
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
