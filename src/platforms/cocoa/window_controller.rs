use objc2::{msg_send, msg_send_id, rc::Retained, ClassType};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSSplitView, NSSplitViewDividerStyle, NSToolbar,
    NSToolbarDisplayMode, NSToolbarItem,
    NSToolbarSizeMode, NSView, NSVisualEffectBlendingMode,
    NSVisualEffectState, NSVisualEffectView, NSWindow, NSWindowStyleMask, NSWindowTitleVisibility,
    NSWindowToolbarStyle,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};
use std::sync::Arc;
use tracing::info;

use crate::platforms::cocoa::controllers::{NavigationController, NavigationDestination};
use crate::platforms::cocoa::delegates::outline_view_delegate::OutlineViewDelegate;
use crate::platforms::cocoa::views::sidebar::ProductionSidebar;
use crate::state::app_state::AppState;

pub struct WindowController {
    window: Retained<NSWindow>,
    split_view: Retained<NSSplitView>,
    sidebar: ProductionSidebar,
    content_container: Retained<NSView>,
    navigation_controller: Arc<NavigationController>,
    outline_delegate: Retained<OutlineViewDelegate>,
}

impl WindowController {
    pub fn new(mtm: MainThreadMarker, app_state: Arc<AppState>) -> Self {
        info!("Creating production-ready window controller");
        
        // Create window with modern macOS styling
        let window = Self::create_window(mtm);
        
        // Create split view
        let split_view = Self::create_split_view(mtm);
        
        // Create sidebar with visual effect background
        let sidebar_container = Self::create_sidebar_container(mtm);
        let sidebar = ProductionSidebar::new(mtm);
        
        // Create content container with visual effect
        let content_container = Self::create_content_container(mtm);
        
        // Set up split view
        unsafe {
            // Add sidebar to visual effect container
            sidebar_container.addSubview(sidebar.view());
            
            // Add containers to split view
            split_view.addSubview(&sidebar_container);
            split_view.addSubview(&content_container);
            
            // Configure split view
            split_view.setPosition_ofDividerAtIndex(250.0, 0);
            
            // Set as window content
            window.setContentView(Some(&split_view));
        }
        
        // Create navigation controller
        let navigation_controller = Arc::new(NavigationController::new(
            content_container.clone(),
            app_state,
        ));
        
        // Create outline view delegate
        let outline_delegate = OutlineViewDelegate::new(mtm);
        outline_delegate.set_navigation_controller(navigation_controller.clone());
        
        // Set delegate on sidebar
        sidebar.set_delegate(&outline_delegate);
        
        // Create toolbar
        Self::setup_toolbar(&window, mtm);
        
        // Navigate to home by default
        navigation_controller.navigate_to(NavigationDestination::Home);
        
        Self {
            window,
            split_view,
            sidebar,
            content_container,
            navigation_controller,
            outline_delegate,
        }
    }
    
    fn create_window(mtm: MainThreadMarker) -> Retained<NSWindow> {
        let frame = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(1400.0, 900.0),
        );
        
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable
            | NSWindowStyleMask::FullSizeContentView;
        
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                mtm.alloc::<NSWindow>(),
                frame,
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        
        unsafe {
            // Modern window appearance
            window.setTitle(&NSString::from_str("Reel"));
            window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
            window.setTitlebarAppearsTransparent(true);
            
            // Set minimum size
            window.setContentMinSize(NSSize::new(1000.0, 600.0));
            
            // Center on screen
            window.center();
            
            // Set background color
            window.setBackgroundColor(&NSColor::windowBackgroundColor());
            
            // Enable full size content view
            window.setStyleMask(window.styleMask() | NSWindowStyleMask::FullSizeContentView);
        }
        
        window
    }
    
    fn create_split_view(mtm: MainThreadMarker) -> Retained<NSSplitView> {
        let split_view = unsafe { NSSplitView::new(mtm) };
        
        unsafe {
            split_view.setVertical(true);
            split_view.setDividerStyle(NSSplitViewDividerStyle::Thin);
            split_view.setAutosaveName(Some(&NSString::from_str("MainSplitView")));
        }
        
        split_view
    }
    
    fn create_sidebar_container(mtm: MainThreadMarker) -> Retained<NSVisualEffectView> {
        let container = unsafe { NSVisualEffectView::new(mtm) };
        
        unsafe {
            // Use sidebar material for proper macOS appearance
            // Note: NSVisualEffectMaterial values need to be set as integers
            // Sidebar material = 4
            let _: () = msg_send![&container, setMaterial: 4i32];
            container.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
            container.setState(NSVisualEffectState::Active);
            container.setWantsLayer(true);
        }
        
        container
    }
    
    fn create_content_container(mtm: MainThreadMarker) -> Retained<NSView> {
        let container = unsafe { NSView::new(mtm) };
        
        unsafe {
            container.setWantsLayer(true);
            
            // Set a subtle background color
            // Note: Setting layer background color requires additional setup
        }
        
        container
    }
    
    fn setup_toolbar(window: &NSWindow, mtm: MainThreadMarker) {
        let toolbar = unsafe { NSToolbar::new(mtm) };
        
        unsafe {
            toolbar.setDisplayMode(NSToolbarDisplayMode::IconOnly);
            toolbar.setSizeMode(NSToolbarSizeMode::Regular);
            toolbar.setShowsBaselineSeparator(false);
            
            // Set toolbar on window
            window.setToolbar(Some(&toolbar));
            window.setToolbarStyle(NSWindowToolbarStyle::Unified);
        }
    }
    
    pub fn show(&self) {
        unsafe {
            self.window.makeKeyAndOrderFront(None);
            
            // Activate the app
            let app = objc2_app_kit::NSApplication::sharedApplication(self.sidebar.mtm());
            app.activateIgnoringOtherApps(true);
        }
    }
    
    pub fn navigation_controller(&self) -> Arc<NavigationController> {
        self.navigation_controller.clone()
    }
}