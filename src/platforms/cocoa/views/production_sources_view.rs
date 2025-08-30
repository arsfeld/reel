use core::cell::OnceCell;
use objc2::{
    define_class, msg_send, msg_send_id,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject, Sel},
    sel, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSButtonType, NSColor, NSControlSize, NSFont, NSImage,
    NSImageName, NSImageSymbolConfiguration, NSImageSymbolScale, NSLayoutConstraint,
    NSProgressIndicator, NSProgressIndicatorStyle, NSScrollView, NSStackView,
    NSStackViewDistribution, NSTableColumn, NSTableView, NSTableViewRowSizeStyle,
    NSTableViewSelectionHighlightStyle, NSTextField, NSTextFieldBezelStyle,
    NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSIndexSet, NSInteger, NSObject, NSObjectProtocol, NSString};
use std::sync::Arc;
use tracing::{debug, info};

use crate::core::viewmodels::SourcesViewModel;
use crate::platforms::cocoa::dialogs::show_auth_dialog_for_backend;
use crate::backends::BackendType;

#[derive(Default)]
struct Ivars {
    view_model: OnceCell<Arc<SourcesViewModel>>,
    table_view: OnceCell<Retained<NSTableView>>,
    add_button: OnceCell<Retained<NSButton>>,
    refresh_button: OnceCell<Retained<NSButton>>,
    status_label: OnceCell<Retained<NSTextField>>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    pub struct ProductionSourcesView;

    unsafe impl NSObjectProtocol for ProductionSourcesView {}
);

impl ProductionSourcesView {
    pub fn new(mtm: MainThreadMarker, view_model: Arc<SourcesViewModel>) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(Ivars::default());
        let this: Retained<Self> = unsafe { msg_send_id![super(this), init] };
        
        // Store view model
        this.ivars().view_model.set(view_model).expect("ViewModel already set");
        
        // Create UI
        this.setup_ui(mtm);
        
        // Set up bindings
        this.setup_bindings();
        
        this
    }
    
    fn setup_ui(&self, mtm: MainThreadMarker) {
        // Create main stack view
        let main_stack = unsafe { NSStackView::new(mtm) };
        unsafe {
            main_stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            main_stack.setDistribution(NSStackViewDistribution::Fill);
            main_stack.setSpacing(0.0);
            main_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        
        // Create toolbar
        let toolbar = self.create_toolbar(mtm);
        
        // Create table container
        let table_container = self.create_table_view(mtm);
        
        // Add to main stack
        unsafe {
            main_stack.addArrangedSubview(&toolbar);
            main_stack.addArrangedSubview(&table_container);
            
            // Add main stack to self
            self.addSubview(&main_stack);
            
            // Pin to edges
            NSLayoutConstraint::activateConstraints(&NSArray::from_slice(&[
                &*main_stack.topAnchor().constraintEqualToAnchor(&self.topAnchor()),
                &*main_stack.leadingAnchor().constraintEqualToAnchor(&self.leadingAnchor()),
                &*main_stack.trailingAnchor().constraintEqualToAnchor(&self.trailingAnchor()),
                &*main_stack.bottomAnchor().constraintEqualToAnchor(&self.bottomAnchor()),
            ]));
        }
    }
    
    fn create_toolbar(&self, mtm: MainThreadMarker) -> Retained<NSView> {
        let toolbar = unsafe { NSView::new(mtm) };
        unsafe {
            toolbar.setTranslatesAutoresizingMaskIntoConstraints(false);
            
            // Set height constraint
            let height_constraint = toolbar.heightAnchor().constraintEqualToConstant(52.0);
            height_constraint.setActive(true);
        }
        
        // Create horizontal stack for buttons
        let button_stack = unsafe { NSStackView::new(mtm) };
        unsafe {
            button_stack.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            button_stack.setDistribution(NSStackViewDistribution::Fill);
            button_stack.setSpacing(8.0);
            button_stack.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        
        // Create Add button with text (system symbols require newer APIs)
        let add_button = unsafe {
            let btn = NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Add Source"),
                None, // We'll set up action handling differently
                None,
                mtm
            );
            btn.setBezelStyle(NSBezelStyle::RoundRect);
            btn.setControlSize(NSControlSize::Regular);
            btn.setToolTip(Some(&NSString::from_str("Add a new media source")));
            btn
        };
        
        // Create Refresh button with text
        let refresh_button = unsafe {
            let btn = NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Refresh"),
                None, // We'll set up action handling differently
                None,
                mtm
            );
            btn.setBezelStyle(NSBezelStyle::RoundRect);
            btn.setControlSize(NSControlSize::Regular);
            btn.setToolTip(Some(&NSString::from_str("Refresh all sources")));
            btn
        };
        
        // Create status label
        let status_label = unsafe {
            NSTextField::labelWithString(
                &NSString::from_str("No sources configured"),
                mtm
            )
        };
        unsafe {
            status_label.setFont(Some(&NSFont::systemFontOfSize(13.0)));
            status_label.setTextColor(Some(&NSColor::secondaryLabelColor()));
            status_label.setLineBreakMode(objc2_app_kit::NSLineBreakMode::TruncatingTail);
        }
        
        // Store references
        self.ivars().add_button.set(add_button.clone()).ok();
        self.ivars().refresh_button.set(refresh_button.clone()).ok();
        self.ivars().status_label.set(status_label.clone()).ok();
        
        // Add to button stack
        unsafe {
            button_stack.addArrangedSubview(&add_button);
            button_stack.addArrangedSubview(&refresh_button);
            
            // Add flexible space
            let spacer = NSView::new(mtm);
            button_stack.addArrangedSubview(&spacer);
            
            button_stack.addArrangedSubview(&status_label);
            
            // Add button stack to toolbar
            toolbar.addSubview(&button_stack);
            
            // Pin with padding
            NSLayoutConstraint::activateConstraints(&NSArray::from_slice(&[
                &*button_stack.leadingAnchor().constraintEqualToAnchor_constant(&toolbar.leadingAnchor(), 12.0),
                &*button_stack.trailingAnchor().constraintEqualToAnchor_constant(&toolbar.trailingAnchor(), -12.0),
                &*button_stack.centerYAnchor().constraintEqualToAnchor(&toolbar.centerYAnchor()),
            ]));
        }
        
        toolbar
    }
    
    fn create_table_view(&self, mtm: MainThreadMarker) -> Retained<NSScrollView> {
        // Create table view
        let table_view = unsafe { NSTableView::new(mtm) };
        unsafe {
            table_view.setRowSizeStyle(NSTableViewRowSizeStyle::Large);
            table_view.setSelectionHighlightStyle(NSTableViewSelectionHighlightStyle::Regular);
            table_view.setAllowsMultipleSelection(false);
            table_view.setUsesAlternatingRowBackgroundColors(true);
            table_view.setRowHeight(72.0);
            
            // Create columns
            let icon_column = NSTableColumn::initWithIdentifier(
                mtm.alloc::<NSTableColumn>(),
                &NSString::from_str("icon")
            );
            icon_column.setWidth(60.0);
            icon_column.setTitle(&NSString::from_str(""));
            
            let name_column = NSTableColumn::initWithIdentifier(
                mtm.alloc::<NSTableColumn>(),
                &NSString::from_str("name")
            );
            name_column.setTitle(&NSString::from_str("Source"));
            
            let status_column = NSTableColumn::initWithIdentifier(
                mtm.alloc::<NSTableColumn>(),
                &NSString::from_str("status")
            );
            status_column.setWidth(150.0);
            status_column.setTitle(&NSString::from_str("Status"));
            
            table_view.addTableColumn(&icon_column);
            table_view.addTableColumn(&name_column);
            table_view.addTableColumn(&status_column);
        }
        
        // Store table view reference
        self.ivars().table_view.set(table_view.clone()).ok();
        
        // Create scroll view
        let scroll_view = unsafe { NSScrollView::new(mtm) };
        unsafe {
            scroll_view.setDocumentView(Some(&table_view));
            scroll_view.setHasVerticalScroller(true);
            scroll_view.setHasHorizontalScroller(false);
            scroll_view.setAutohidesScrollers(true);
            scroll_view.setBorderType(objc2_app_kit::NSBorderType::NoBorder);
        }
        
        scroll_view
    }
    
    fn setup_bindings(&self) {
        debug!("Setting up SourcesView bindings");
        // TODO: Subscribe to view model changes
    }
    
    pub fn show_add_source_menu(&self) {
        // For now, just show Plex auth directly
        // In production, we'd show a menu
        info!("Showing add source options");
        show_auth_dialog_for_backend(BackendType::Plex);
    }
    
    pub fn refresh_sources(&self) {
        if let Some(vm) = self.ivars().view_model.get() {
            // Start refresh
            let vm = vm.clone();
            tokio::spawn(async move {
                vm.refresh().await;
            });
            
            // Update UI to show refreshing state
            if let Some(status) = self.ivars().status_label.get() {
                unsafe {
                    status.setStringValue(&NSString::from_str("Refreshing sources..."));
                }
            }
        }
    }
}

// Import NSArray and NSPoint
use objc2_foundation::{NSArray, NSPoint};