use crate::platforms::cocoa::utils::CGFloat;
use dispatch::Queue;
use keyring::Entry;
use objc2::{
    ClassType, DefinedClass, MainThreadOnly, msg_send, msg_send_id, rc::Retained,
    runtime::NSObject, sel,
};
use objc2_app_kit::{
    NSAlert, NSAlertStyle, NSBezelStyle, NSBorderType, NSButton, NSButtonType, NSColor,
    NSControlSize, NSFont, NSImage, NSImageView, NSLayoutAttribute, NSLayoutConstraint,
    NSLayoutRelation, NSLineBreakMode, NSMenu, NSMenuItem, NSModalResponse, NSPopUpButton,
    NSProgressIndicator, NSProgressIndicatorStyle, NSScrollView, NSSegmentStyle,
    NSSegmentedControl, NSStackView, NSStackViewDistribution, NSTableColumn, NSTableView,
    NSTableViewDataSource, NSTableViewDelegate, NSTextAlignment, NSTextField,
    NSUserInterfaceLayoutOrientation, NSView, NSWindow, NSWindowController,
};
use objc2_foundation::{
    NSArray, NSDictionary, NSIndexSet, NSInteger, NSNumber, NSString, NSUInteger,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::backends::BackendType;
use crate::core::viewmodels::{Property, SourceInfo, SourcesViewModel};
use crate::db::entities::sources::Model as Source;
use crate::models::{Credentials, JellyfinCredentials, PlexCredentials};
use crate::platforms::cocoa::dialogs::show_auth_dialog_for_backend;
use crate::platforms::cocoa::error::{CocoaError, CocoaResult};
use crate::platforms::cocoa::utils::{AutoLayout, NSEdgeInsets, main_thread_marker};

const ROW_HEIGHT: CGFloat = 80.0;
const BUTTON_WIDTH: CGFloat = 100.0;
const PROGRESS_WIDTH: CGFloat = 150.0;

// SourceCellView - Using simple NSObject for now due to objc2 0.6 macro complexity
// TODO: Convert to proper define_class! when macro syntax is stabilized
pub type SourceCellView = NSObject;

// TODO: SourceCellView impl removed - cannot implement on external NSObject type
// Helper function for creating source cell views
pub fn create_source_cell_view() -> Option<Retained<NSObject>> {
    Some(NSObject::new())
}

// Main Sources View
pub struct SourcesView {
    container: Retained<NSView>,
    toolbar: Retained<NSView>,
    scroll_view: Retained<NSScrollView>,
    table_view: Retained<NSTableView>,
    add_button: Retained<NSButton>,
    refresh_button: Retained<NSButton>,
    status_label: Retained<NSTextField>,
    view_model: Arc<SourcesViewModel>,
    data_source: Arc<RwLock<Vec<SourceInfo>>>,
    selected_index: Arc<RwLock<Option<NSInteger>>>,
}

impl SourcesView {
    pub fn new(view_model: Arc<SourcesViewModel>) -> CocoaResult<Self> {
        debug!("Creating SourcesView");

        // Create main container
        let container = unsafe {
            let view = NSView::new(main_thread_marker());
            view.setTranslatesAutoresizingMaskIntoConstraints(false);
            view
        };

        // Create toolbar
        let toolbar = Self::create_toolbar()?;

        // Create table view with scroll view
        let (scroll_view, table_view) = Self::create_table_view()?;

        // Get toolbar controls
        let (add_button, refresh_button, status_label) = Self::get_toolbar_controls(&toolbar)?;

        // Setup layout
        unsafe {
            container.addSubview(&toolbar);
            container.addSubview(&scroll_view);

            // Setup constraints
            // TODO: Simplified constraints - AutoLayout API doesn't support builder pattern
            let toolbar_constraints = vec![AutoLayout::height(&toolbar, 44.0)];
            AutoLayout::activate(&toolbar_constraints);

            // TODO: Additional constraints for scroll_view layout would be added here
        }

        let data_source = Arc::new(RwLock::new(Vec::new()));
        let selected_index = Arc::new(RwLock::new(None));

        let mut sources_view = Self {
            container,
            toolbar,
            scroll_view,
            table_view,
            add_button,
            refresh_button,
            status_label,
            view_model,
            data_source,
            selected_index,
        };

        sources_view.setup_bindings()?;
        sources_view.setup_table_view()?;

        Ok(sources_view)
    }

    pub fn view(&self) -> &NSView {
        &self.container
    }

    pub fn into_view(self) -> Retained<NSView> {
        self.container
    }

    fn create_toolbar() -> CocoaResult<Retained<NSView>> {
        let mtm = main_thread_marker();
        unsafe {
            let toolbar = NSView::new(mtm);
            toolbar.setWantsLayer(true);
            toolbar.setTranslatesAutoresizingMaskIntoConstraints(false);

            // Create add button with emoji for visibility
            let add_button = NSButton::new(main_thread_marker());
            add_button.setTitle(&NSString::from_str("➕ Add Source"));
            add_button.setBezelStyle(NSBezelStyle::Rounded);
            add_button.setTranslatesAutoresizingMaskIntoConstraints(false);

            // Create refresh button with emoji
            let refresh_button = NSButton::new(main_thread_marker());
            refresh_button.setTitle(&NSString::from_str("🔄 Refresh"));
            refresh_button.setBezelStyle(NSBezelStyle::Rounded);
            refresh_button.setTranslatesAutoresizingMaskIntoConstraints(false);

            // Create status label with more prominent text
            let status_label = NSTextField::labelWithString(
                &NSString::from_str(
                    "📦 No sources configured - Click '➕ Add Source' to add Plex or Jellyfin",
                ),
                mtm,
            );
            status_label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
            status_label.setTextColor(Some(&NSColor::secondaryLabelColor()));
            status_label.setTranslatesAutoresizingMaskIntoConstraints(false);

            // Add subviews
            toolbar.addSubview(&add_button);
            toolbar.addSubview(&refresh_button);
            toolbar.addSubview(&status_label);

            // Layout constraints
            // TODO: Simplified constraints - builder pattern not supported
            let add_button_constraints = vec![AutoLayout::center_vertically(&add_button)];
            AutoLayout::activate(&add_button_constraints);

            // TODO: Simplified constraints - complex layout removed
            let refresh_constraints = vec![AutoLayout::center_horizontally(&refresh_button)];
            AutoLayout::activate(&refresh_constraints);

            let status_constraints = vec![AutoLayout::center_horizontally(&status_label)];
            AutoLayout::activate(&status_constraints);

            Ok(toolbar)
        }
    }

    fn create_table_view() -> CocoaResult<(Retained<NSScrollView>, Retained<NSTableView>)> {
        let mtm = main_thread_marker();
        unsafe {
            // Create table view
            let table_view = NSTableView::new(main_thread_marker());
            table_view.setHeaderView(None);
            table_view.setRowHeight(ROW_HEIGHT);
            table_view.setSelectionHighlightStyle(
                objc2_app_kit::NSTableViewSelectionHighlightStyle::Regular,
            );
            table_view.setAllowsMultipleSelection(false);
            table_view.setUsesAlternatingRowBackgroundColors(true);

            // Create single column
            let column = NSTableColumn::initWithIdentifier(
                NSTableColumn::alloc(mtm),
                &NSString::from_str("sources"),
            );
            column.setWidth(400.0);
            column.setResizingMask(objc2_app_kit::NSTableColumnResizingOptions::AutoresizingMask);
            table_view.addTableColumn(&column);

            // Create scroll view
            let scroll_view = NSScrollView::new(main_thread_marker());
            scroll_view.setDocumentView(Some(&table_view));
            scroll_view.setHasVerticalScroller(true);
            scroll_view.setHasHorizontalScroller(false);
            scroll_view.setAutohidesScrollers(true);
            scroll_view.setBorderType(NSBorderType::BezelBorder);
            scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);

            Ok((scroll_view, table_view))
        }
    }

    fn get_toolbar_controls(
        toolbar: &NSView,
    ) -> CocoaResult<(
        Retained<NSButton>,
        Retained<NSButton>,
        Retained<NSTextField>,
    )> {
        unsafe {
            let subviews = toolbar.subviews();
            if subviews.count() >= 3 {
                let add_button =
                    subviews
                        .objectAtIndex(0)
                        .downcast::<NSButton>()
                        .map_err(|_| {
                            CocoaError::InvalidState("Failed to get add button".to_string())
                        })?;
                let refresh_button =
                    subviews
                        .objectAtIndex(1)
                        .downcast::<NSButton>()
                        .map_err(|_| {
                            CocoaError::InvalidState("Failed to get refresh button".to_string())
                        })?;
                let status_label = subviews
                    .objectAtIndex(2)
                    .downcast::<NSTextField>()
                    .map_err(|_| {
                        CocoaError::InvalidState("Failed to get status label".to_string())
                    })?;

                Ok((add_button, refresh_button, status_label))
            } else {
                Err(CocoaError::InvalidState(
                    "Toolbar missing controls".to_string(),
                ))
            }
        }
    }

    fn setup_bindings(&mut self) -> CocoaResult<()> {
        debug!("Setting up SourcesView bindings");

        // Subscribe to sources property changes
        let sources_property = self.view_model.sources();
        let data_source = self.data_source.clone();
        let table_view = self.table_view.clone();
        let status_label = self.status_label.clone();

        // sources_property.subscribe(Box::new( // TODO: Fix subscription API
        /*move |sources| {
            debug!("Sources updated, count: {}", sources.len());

            let data_source = data_source.clone();
            let table_view = table_view.clone();
            let status_label = status_label.clone();
            let sources = sources.clone();

            tokio::spawn(async move {
                // Update data source
                {
                    let mut data = data_source.write().await;
                    *data = sources.clone();
                }

                // Update UI on main thread
                Queue::main().exec_async(move || {
                    unsafe {
                        table_view.reloadData();

                        // Update status label
                        let status_text = match sources.len() {
                            0 => "No sources configured".to_string(),
                            1 => "1 source configured".to_string(),
                            n => format!("{} sources configured", n),
                        };
                        status_label.setStringValue(&NSString::from_str(&status_text));
                    }
                });
            });
        }));
        */

        Ok(())
    }

    fn setup_table_view(&mut self) -> CocoaResult<()> {
        debug!("Setting up table view data source and delegate");

        // In a real implementation, we'd set up proper data source and delegate objects
        // For now, we'll just configure basic properties

        Ok(())
    }

    pub async fn refresh(&self) -> CocoaResult<()> {
        info!("Refreshing sources view");
        self.view_model.refresh().await;
        Ok(())
    }

    pub fn add_source(&self) {
        info!("Add source button clicked");
        // TODO: Show add source dialog
    }
}
