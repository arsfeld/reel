use objc2::{msg_send, msg_send_id, rc::Retained, ClassType};
use objc2_app_kit::{
    NSOutlineView, NSScrollView, NSTableColumn, NSTableViewRowSizeStyle,
    NSTableViewSelectionHighlightStyle, NSTableViewStyle, NSView,
};
use objc2_foundation::{MainThreadMarker, NSString};
use tracing::debug;

use crate::platforms::cocoa::delegates::outline_view_delegate::OutlineViewDelegate;

/// Production-ready sidebar with native macOS styling
pub struct ProductionSidebar {
    scroll_view: Retained<NSScrollView>,
    outline_view: Retained<NSOutlineView>,
    mtm: MainThreadMarker,
}

impl ProductionSidebar {
    pub fn new(mtm: MainThreadMarker) -> Self {
        debug!("Creating production sidebar");
        
        // Create scroll view
        let scroll_view = Self::create_scroll_view(mtm);
        
        // Create outline view
        let outline_view = Self::create_outline_view(mtm);
        
        // Configure outline view
        Self::configure_outline_view(&outline_view);
        
        // Set document view
        unsafe {
            scroll_view.setDocumentView(Some(&outline_view));
        }
        
        Self {
            scroll_view,
            outline_view,
            mtm,
        }
    }
    
    fn create_scroll_view(mtm: MainThreadMarker) -> Retained<NSScrollView> {
        let scroll_view = unsafe { NSScrollView::new(mtm) };
        
        unsafe {
            // Configure for sidebar appearance
            scroll_view.setHasVerticalScroller(true);
            scroll_view.setHasHorizontalScroller(false);
            scroll_view.setAutohidesScrollers(true);
            scroll_view.setBorderType(objc2_app_kit::NSBorderType::NoBorder);
            scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);
            
            // Set background to transparent for visual effect to show through
            scroll_view.setDrawsBackground(false);
        }
        
        scroll_view
    }
    
    fn create_outline_view(mtm: MainThreadMarker) -> Retained<NSOutlineView> {
        let outline_view = unsafe { NSOutlineView::new(mtm) };
        
        // Create column
        let column = unsafe {
            NSTableColumn::initWithIdentifier(
                mtm.alloc::<NSTableColumn>(),
                &NSString::from_str("MainColumn"),
            )
        };
        
        unsafe {
            // Configure column
            column.setWidth(250.0);
            column.setMinWidth(200.0);
            column.setMaxWidth(400.0);
            column.setTitle(&NSString::from_str("Navigation"));
            
            // Add column to outline view
            outline_view.addTableColumn(&column);
            outline_view.setOutlineTableColumn(Some(&column));
        }
        
        outline_view
    }
    
    fn configure_outline_view(outline_view: &NSOutlineView) {
        unsafe {
            // Use source list style for native sidebar appearance
            outline_view.setSelectionHighlightStyle(
                NSTableViewSelectionHighlightStyle::SourceList
            );
            
            // Configure appearance
            outline_view.setFloatsGroupRows(true);
            outline_view.setRowSizeStyle(NSTableViewRowSizeStyle::Default);
            outline_view.setIndentationPerLevel(14.0);
            
            // Selection behavior
            outline_view.setAllowsMultipleSelection(false);
            outline_view.setAllowsEmptySelection(true);
            outline_view.setAllowsColumnSelection(false);
            
            // Modern sidebar styling
            outline_view.setStyle(NSTableViewStyle::SourceList);
            
            // Hide header
            outline_view.setHeaderView(None);
            
            // Set row height for better appearance
            outline_view.setRowHeight(24.0);
            
            // Enable autosaving of expanded state
            outline_view.setAutosaveExpandedItems(true);
            outline_view.setAutosaveName(Some(&NSString::from_str("SidebarOutlineView")));
        }
    }
    
    pub fn set_delegate(&self, delegate: &OutlineViewDelegate) {
        unsafe {
            // Set both data source and delegate
            let delegate_obj = delegate as *const _ as *const objc2::runtime::AnyObject;
            self.outline_view.setDataSource(Some(
                &ProtocolObject::from_ref(delegate_obj)
            ));
            self.outline_view.setDelegate(Some(
                &ProtocolObject::from_ref(delegate_obj)
            ));
            
            // Reload data
            self.outline_view.reloadData();
        }
    }
    
    pub fn view(&self) -> &NSView {
        // Cast NSScrollView to NSView
        &**self.scroll_view
    }
    
    pub fn mtm(&self) -> MainThreadMarker {
        self.mtm
    }
    
    pub fn reload_data(&self) {
        unsafe {
            self.outline_view.reloadData();
        }
    }
    
    pub fn expand_all(&self) {
        unsafe {
            self.outline_view.expandItem_expandChildren(None, true);
        }
    }
    
    pub fn select_row(&self, row: i64) {
        use objc2_foundation::NSIndexSet;
        
        unsafe {
            let index_set = NSIndexSet::indexSetWithIndex(row as usize);
            self.outline_view.selectRowIndexes_byExtendingSelection(&index_set, false);
        }
    }
}

// Import ProtocolObject for delegate setting
use objc2::runtime::ProtocolObject;