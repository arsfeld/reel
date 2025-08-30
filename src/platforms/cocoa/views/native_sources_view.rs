use objc2::{ClassType, msg_send, msg_send_id, rc::Retained};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSColor, NSControlSize, NSFont, NSProgressIndicator,
    NSProgressIndicatorStyle, NSScrollView, NSStackView, NSStackViewDistribution, NSTableColumn,
    NSTableView, NSTableViewRowSizeStyle, NSTableViewSelectionHighlightStyle, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{MainThreadMarker, NSString};
use std::sync::Arc;
use tracing::{debug, info};

use crate::backends::BackendType;
use crate::core::viewmodels::SourcesViewModel;
use crate::platforms::cocoa::dialogs::show_auth_dialog_for_backend;

/// Native macOS Sources view with professional styling
pub struct NativeSourcesView {
    container: Retained<NSView>,
    table_view: Retained<NSTableView>,
    add_button: Retained<NSButton>,
    refresh_button: Retained<NSButton>,
    status_label: Retained<NSTextField>,
    view_model: Arc<SourcesViewModel>,
}

impl NativeSourcesView {
    pub fn new(mtm: MainThreadMarker, view_model: Arc<SourcesViewModel>) -> Self {
        debug!("Creating native sources view");

        // Create container
        let container = unsafe { NSView::new(mtm) };
        unsafe {
            container.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        // Create toolbar
        let toolbar = Self::create_toolbar(mtm);

        // Create table view
        let (scroll_view, table_view) = Self::create_table_view(mtm);

        // Create buttons
        let add_button = Self::create_add_button(mtm);
        let refresh_button = Self::create_refresh_button(mtm);

        // Create status label
        let status_label = Self::create_status_label(mtm);

        // Layout
        unsafe {
            // Add buttons to toolbar
            toolbar.addSubview(&add_button);
            toolbar.addSubview(&refresh_button);
            toolbar.addSubview(&status_label);

            // Layout buttons horizontally
            use crate::platforms::cocoa::utils::AutoLayout;
            let button_constraints = vec![
                AutoLayout::leading(&add_button, 12.0),
                AutoLayout::center_vertically(&add_button),
                AutoLayout::width(&add_button, 100.0),
                AutoLayout::center_vertically(&refresh_button),
                AutoLayout::width(&refresh_button, 100.0),
            ];
            AutoLayout::activate(&button_constraints);

            // Position refresh button next to add button
            // Simplified positioning - just place it to the right

            // Position status label
            let status_constraints = vec![
                AutoLayout::center_vertically(&status_label),
                AutoLayout::trailing(&status_label, -12.0),
            ];
            AutoLayout::activate(&status_constraints);

            // Add toolbar and scroll view to container
            container.addSubview(&toolbar);
            container.addSubview(&scroll_view);

            // Layout toolbar and scroll view vertically
            let toolbar_height = AutoLayout::height(&toolbar, 52.0);
            AutoLayout::activate(&[toolbar_height]);

            let _: () = msg_send![&toolbar, setTranslatesAutoresizingMaskIntoConstraints: false];
            let _: () =
                msg_send![&scroll_view, setTranslatesAutoresizingMaskIntoConstraints: false];

            // Pin toolbar to top - simplified

            // Pin toolbar horizontally
            let toolbar_constraints = vec![
                AutoLayout::leading(&toolbar, 0.0),
                AutoLayout::trailing(&toolbar, 0.0),
            ];
            AutoLayout::activate(&toolbar_constraints);

            // Pin scroll view below toolbar - simplified
            // Will use frames instead of constraints for simplicity

            // Pin scroll view to other edges
            let scroll_constraints = vec![
                AutoLayout::leading(&scroll_view, 0.0),
                AutoLayout::trailing(&scroll_view, 0.0),
                AutoLayout::bottom(&scroll_view, 0.0),
            ];
            AutoLayout::activate(&scroll_constraints);
        }

        let mut view = Self {
            container,
            table_view,
            add_button,
            refresh_button,
            status_label,
            view_model,
        };

        // Set up initial state
        view.update_status("No sources configured. Click 'Add Source' to add Plex or Jellyfin.");
        view.setup_bindings();

        view
    }

    fn create_toolbar(mtm: MainThreadMarker) -> Retained<NSView> {
        let toolbar = unsafe { NSView::new(mtm) };
        unsafe {
            toolbar.setWantsLayer(true);
        }
        toolbar
    }

    fn create_add_button(mtm: MainThreadMarker) -> Retained<NSButton> {
        let button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Add Source"),
                None,
                None,
                mtm,
            )
        };
        unsafe {
            button.setBezelStyle(NSBezelStyle::RoundRect);
            button.setControlSize(NSControlSize::Regular);
        }
        button
    }

    fn create_refresh_button(mtm: MainThreadMarker) -> Retained<NSButton> {
        let button = unsafe {
            NSButton::buttonWithTitle_target_action(&NSString::from_str("Refresh"), None, None, mtm)
        };
        unsafe {
            button.setBezelStyle(NSBezelStyle::RoundRect);
            button.setControlSize(NSControlSize::Regular);
        }
        button
    }

    fn create_status_label(mtm: MainThreadMarker) -> Retained<NSTextField> {
        let label = unsafe { NSTextField::labelWithString(&NSString::from_str("Loading..."), mtm) };
        unsafe {
            label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
            label.setTextColor(Some(&NSColor::secondaryLabelColor()));
        }
        label
    }

    fn create_table_view(mtm: MainThreadMarker) -> (Retained<NSScrollView>, Retained<NSTableView>) {
        // Create table view
        let table_view = unsafe { NSTableView::new(mtm) };
        unsafe {
            table_view.setRowSizeStyle(NSTableViewRowSizeStyle::Large);
            table_view.setSelectionHighlightStyle(NSTableViewSelectionHighlightStyle::Regular);
            table_view.setAllowsMultipleSelection(false);
            table_view.setUsesAlternatingRowBackgroundColors(true);
            table_view.setRowHeight(72.0);

            // Create columns
            let name_column = NSTableColumn::initWithIdentifier(
                mtm.alloc::<NSTableColumn>(),
                &NSString::from_str("name"),
            );
            name_column.setTitle(&NSString::from_str("Source"));
            name_column.setWidth(300.0);

            let type_column = NSTableColumn::initWithIdentifier(
                mtm.alloc::<NSTableColumn>(),
                &NSString::from_str("type"),
            );
            type_column.setTitle(&NSString::from_str("Type"));
            type_column.setWidth(100.0);

            let status_column = NSTableColumn::initWithIdentifier(
                mtm.alloc::<NSTableColumn>(),
                &NSString::from_str("status"),
            );
            status_column.setTitle(&NSString::from_str("Status"));
            status_column.setWidth(150.0);

            table_view.addTableColumn(&name_column);
            table_view.addTableColumn(&type_column);
            table_view.addTableColumn(&status_column);
        }

        // Create scroll view
        let scroll_view = unsafe { NSScrollView::new(mtm) };
        unsafe {
            scroll_view.setDocumentView(Some(&table_view));
            scroll_view.setHasVerticalScroller(true);
            scroll_view.setHasHorizontalScroller(false);
            scroll_view.setAutohidesScrollers(true);
            scroll_view.setBorderType(objc2_app_kit::NSBorderType::NoBorder);
        }

        (scroll_view, table_view)
    }

    fn setup_bindings(&mut self) {
        // Set up button click handling with a simple polling approach
        // In production, we'd use proper target-action or delegates
        self.setup_button_monitor();
    }

    fn setup_button_monitor(&self) {
        // For now, just log that buttons are ready
        info!("Sources view buttons ready - click handling would be implemented here");
    }

    fn update_status(&self, text: &str) {
        unsafe {
            self.status_label.setStringValue(&NSString::from_str(text));
        }
    }

    pub fn show_add_source_dialog(&self) {
        info!("Showing add source dialog");
        // For now, show Plex auth directly
        show_auth_dialog_for_backend(BackendType::Plex);
    }

    pub fn refresh(&self) {
        info!("Refreshing sources");
        self.update_status("Refreshing sources...");

        let vm = self.view_model.clone();
        tokio::spawn(async move {
            vm.refresh().await;
        });
    }

    pub fn view(&self) -> &NSView {
        &self.container
    }

    pub fn into_view(self) -> Retained<NSView> {
        self.container
    }
}
