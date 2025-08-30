use core::cell::OnceCell;
use objc2::{
    define_class, msg_send, msg_send_id,
    rc::Retained,
    runtime::{AnyObject, NSObject as NSObjectRuntime, ProtocolObject, Sel},
    sel, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSOutlineView, NSOutlineViewDataSource, NSOutlineViewDelegate, NSTableCellView, NSTableColumn,
    NSTextField, NSView,
};
use objc2_foundation::{NSIndexSet, NSInteger, NSNotification, NSObject, NSObjectProtocol, NSString};
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

use crate::platforms::cocoa::controllers::{NavigationController, NavigationDestination};

#[derive(Debug)]
pub struct SidebarItem {
    pub title: String,
    pub icon: Option<String>,
    pub destination: NavigationDestination,
    pub children: Vec<SidebarItem>,
    pub is_group: bool,
}

impl SidebarItem {
    pub fn group(title: impl Into<String>, children: Vec<SidebarItem>) -> Self {
        Self {
            title: title.into(),
            icon: None,
            destination: NavigationDestination::Home,
            children,
            is_group: true,
        }
    }

    pub fn item(title: impl Into<String>, icon: Option<String>, destination: NavigationDestination) -> Self {
        Self {
            title: title.into(),
            icon,
            destination,
            children: Vec::new(),
            is_group: false,
        }
    }
}

#[derive(Default)]
struct Ivars {
    items: OnceCell<Arc<Mutex<Vec<SidebarItem>>>>,
    navigation_controller: OnceCell<Arc<NavigationController>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    pub struct OutlineViewDelegate;

    unsafe impl NSObjectProtocol for OutlineViewDelegate {}

    unsafe impl NSOutlineViewDataSource for OutlineViewDelegate {
        #[unsafe(method(outlineView:numberOfChildrenOfItem:))]
        unsafe fn outline_view_number_of_children(
            &self,
            _outline_view: &NSOutlineView,
            item: Option<&AnyObject>,
        ) -> NSInteger {
            let items = self.ivars().items.get().expect("Items not initialized");
            let items = items.lock().unwrap();
            
            if item.is_none() {
                // Root level items
                items.len() as NSInteger
            } else {
                // TODO: Handle nested items
                0
            }
        }

        #[unsafe(method(outlineView:child:ofItem:))]
        unsafe fn outline_view_child(
            &self,
            _outline_view: &NSOutlineView,
            index: NSInteger,
            item: Option<&AnyObject>,
        ) -> &NSObject {
            // Return a static reference to a placeholder object
            // In production, we'd return an actual object representing the item
            static PLACEHOLDER: std::sync::OnceLock<Retained<NSObject>> = std::sync::OnceLock::new();
            PLACEHOLDER.get_or_init(|| NSObject::new())
        }

        #[unsafe(method(outlineView:isItemExpandable:))]
        unsafe fn outline_view_is_item_expandable(
            &self,
            _outline_view: &NSOutlineView,
            _item: &AnyObject,
        ) -> bool {
            // For now, no items are expandable
            false
        }
    }

    unsafe impl NSOutlineViewDelegate for OutlineViewDelegate {
        #[unsafe(method(outlineView:viewForTableColumn:item:))]
        unsafe fn outline_view_view_for_table_column(
            &self,
            outline_view: &NSOutlineView,
            _table_column: Option<&NSTableColumn>,
            _item: &AnyObject,
        ) -> Option<&NSView> {
            let mtm = self.mtm();
            
            // Create or reuse a cell view
            let identifier = NSString::from_str("SidebarCell");
            let cell_view: Option<Retained<NSTableCellView>> = unsafe {
                msg_send_id![outline_view, makeViewWithIdentifier: &*identifier, owner: None::<&NSObject>]
            };
            
            let cell_view = if let Some(existing) = cell_view {
                existing
            } else {
                // Create new cell view
                let cell = unsafe { NSTableCellView::new(mtm) };
                
                // Create text field for the cell
                let text_field = unsafe { NSTextField::labelWithString(&NSString::from_str("Item"), mtm) };
                unsafe {
                    cell.setTextField(Some(&text_field));
                    cell.addSubview(&text_field);
                }
                
                cell
            };
            
            // Configure the cell with item data
            if let Some(text_field) = unsafe { cell_view.textField() } {
                // Get item index and set appropriate text
                let items = self.ivars().items.get().expect("Items not initialized");
                let items = items.lock().unwrap();
                
                // For simplicity, we'll just use a placeholder text for now
                // In production, we'd map the item to the correct index
                unsafe {
                    text_field.setStringValue(&NSString::from_str("Sidebar Item"));
                }
            }
            
            Some(&**cell_view)
        }

        #[unsafe(method(outlineViewSelectionDidChange:))]
        unsafe fn outline_view_selection_did_change(&self, notification: &NSNotification) {
            let outline_view = notification
                .object()
                .expect("Notification should have outline view")
                .downcast_ref::<NSOutlineView>()
                .expect("Object should be NSOutlineView");
            
            let selected_row = unsafe { outline_view.selectedRow() };
            
            if selected_row >= 0 {
                debug!("Sidebar selection changed to row {}", selected_row);
                
                // Get the corresponding item and navigate
                let items = self.ivars().items.get().expect("Items not initialized");
                let items = items.lock().unwrap();
                
                if let Some(item) = items.get(selected_row as usize) {
                    if !item.is_group {
                        info!("Navigating to {:?}", item.destination);
                        
                        if let Some(nav) = self.ivars().navigation_controller.get() {
                            nav.navigate_to(item.destination.clone());
                        }
                    }
                }
            }
        }
    }
);

impl OutlineViewDelegate {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc::<Self>().set_ivars(Ivars::default());
        let this: Retained<Self> = unsafe { msg_send_id![super(this), init] };
        
        // Initialize with default items
        let items = vec![
            SidebarItem::item("Home", Some("🏠".to_string()), NavigationDestination::Home),
            SidebarItem::item("Sources", Some("⚙️".to_string()), NavigationDestination::Sources),
        ];
        
        this.ivars().items.set(Arc::new(Mutex::new(items))).expect("Items already set");
        
        this
    }
    
    pub fn set_navigation_controller(&self, nav: Arc<NavigationController>) {
        self.ivars().navigation_controller.set(nav).ok();
    }
    
    pub fn add_library(&self, id: String, title: String) {
        if let Some(items) = self.ivars().items.get() {
            let mut items = items.lock().unwrap();
            items.push(SidebarItem::item(
                title,
                Some("📚".to_string()),
                NavigationDestination::Library(id),
            ));
        }
    }
    
    pub fn reload_data(&self, outline_view: &NSOutlineView) {
        unsafe {
            outline_view.reloadData();
        }
    }
}