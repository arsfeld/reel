use std::sync::Arc;
use objc2::{rc::Retained, runtime::NSObject, ClassType, DefinedClass, msg_send, msg_send_id, sel};
use objc2_foundation::{NSString, NSArray, NSInteger, NSUInteger, NSNumber, NSIndexSet, NSDictionary};
use objc2_app_kit::{
    NSView, NSTableView, NSTableColumn, NSScrollView, NSTableViewDataSource,
    NSTableViewDelegate, NSButton, NSTextField, NSProgressIndicator, NSImageView,
    NSImage, NSStackView, NSLayoutConstraint, NSLayoutAttribute, NSLayoutRelation,
    NSColor, NSFont, NSTextAlignment, NSLineBreakMode, NSBezelStyle,
    NSButtonType, NSControlSize, NSBorderType, NSProgressIndicatorStyle,
    NSUserInterfaceLayoutOrientation, NSStackViewDistribution,
    NSAlert, NSAlertStyle, NSModalResponse, NSWindow, NSWindowController,
    NSSegmentedControl, NSSegmentStyle, NSPopUpButton, NSMenu, NSMenuItem,
};
use crate::platforms::cocoa::utils::CGFloat;
use dispatch::Queue;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use keyring::Entry;

use crate::core::viewmodels::{SourcesViewModel, Property, SourceInfo};
use crate::db::entities::sources::Model as Source;
use crate::backends::BackendType;
use crate::models::{Credentials, PlexCredentials, JellyfinCredentials};
use crate::platforms::cocoa::error::{CocoaError, CocoaResult};
use crate::platforms::cocoa::utils::{AutoLayout, NSEdgeInsets};
use crate::platforms::cocoa::dialogs::show_auth_dialog_for_backend;

const ROW_HEIGHT: CGFloat = 80.0;
const BUTTON_WIDTH: CGFloat = 100.0;
const PROGRESS_WIDTH: CGFloat = 150.0;

// SourceCellView - Using simple NSObject for now due to objc2 0.6 macro complexity
// TODO: Convert to proper define_class! when macro syntax is stabilized
pub type SourceCellView = NSObject;

impl SourceCellView {
    pub fn init_stub() -> Option<Retained<Self>> {
        Some(unsafe { NSObject::new() })
    }
}
    fn setup_views(&mut self) {
        unsafe {
            self.setWantsLayer(true);
            
            // Create icon view
            let icon_view = NSImageView::new();
            icon_view.setImageScaling(objc2_app_kit::NSImageScaling::NSImageScaleProportionallyUpOrDown);
            icon_view.setTranslatesAutoresizingMaskIntoConstraints(false);
            
            // Create name label
            let name_label = NSTextField::labelWithString(&NSString::from_str(""));
            name_label.setFont(Some(&NSFont::boldSystemFontOfSize(14.0)));
            name_label.setTextColor(Some(&NSColor::labelColor()));
            name_label.setTranslatesAutoresizingMaskIntoConstraints(false);
            
            // Create status label
            let status_label = NSTextField::labelWithString(&NSString::from_str(""));
            status_label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
            status_label.setTextColor(Some(&NSColor::secondaryLabelColor()));
            status_label.setTranslatesAutoresizingMaskIntoConstraints(false);
            
            // Create library count label
            let library_count_label = NSTextField::labelWithString(&NSString::from_str(""));
            library_count_label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
            library_count_label.setTextColor(Some(&NSColor::tertiaryLabelColor()));
            library_count_label.setTranslatesAutoresizingMaskIntoConstraints(false);
            
            // Create sync progress indicator
            let sync_progress = NSProgressIndicator::new();
            sync_progress.setStyle(NSProgressIndicatorStyle::NSProgressIndicatorStyleBar);
            sync_progress.setIndeterminate(false);
            sync_progress.setHidden(true);
            sync_progress.setTranslatesAutoresizingMaskIntoConstraints(false);
            
            // Create buttons
            let sync_button = NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Sync"),
                None,
                Some(sel!(syncClicked:))
            );
            sync_button.setBezelStyle(NSBezelStyle::NSBezelStyleRounded);
            sync_button.setControlSize(NSControlSize::Small);
            sync_button.setTranslatesAutoresizingMaskIntoConstraints(false);
            
            let edit_button = NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Edit"),
                None,
                Some(sel!(editClicked:))
            );
            edit_button.setBezelStyle(NSBezelStyle::NSBezelStyleRounded);
            edit_button.setControlSize(NSControlSize::Small);
            edit_button.setTranslatesAutoresizingMaskIntoConstraints(false);
            
            let delete_button = NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Delete"),
                None,
                Some(sel!(deleteClicked:))
            );
            delete_button.setBezelStyle(NSBezelStyle::NSBezelStyleRounded);
            delete_button.setControlSize(NSControlSize::Small);
            delete_button.setTranslatesAutoresizingMaskIntoConstraints(false);
            
            // Add subviews
            self.addSubview(&icon_view);
            self.addSubview(&name_label);
            self.addSubview(&status_label);
            self.addSubview(&library_count_label);
            self.addSubview(&sync_progress);
            self.addSubview(&sync_button);
            self.addSubview(&edit_button);
            self.addSubview(&delete_button);
            
            // Setup constraints
            AutoLayout::new(self)
                .leading(&icon_view, 16.0)
                .centerY(&icon_view, 0.0)
                .width(&icon_view, 48.0)
                .height(&icon_view, 48.0)
                .activate();
            
            AutoLayout::new(self)
                .leading_to_trailing(&name_label, &icon_view, 12.0)
                .top(&name_label, 16.0)
                .activate();
            
            AutoLayout::new(self)
                .leading_to_trailing(&status_label, &icon_view, 12.0)
                .top_to_bottom(&status_label, &name_label, 4.0)
                .activate();
            
            AutoLayout::new(self)
                .leading_to_trailing(&library_count_label, &icon_view, 12.0)
                .top_to_bottom(&library_count_label, &status_label, 4.0)
                .activate();
            
            AutoLayout::new(self)
                .trailing(&delete_button, 16.0)
                .centerY(&delete_button, 0.0)
                .width(&delete_button, 60.0)
                .activate();
            
            AutoLayout::new(self)
                .trailing_to_leading(&edit_button, &delete_button, 8.0)
                .centerY(&edit_button, 0.0)
                .width(&edit_button, 60.0)
                .activate();
            
            AutoLayout::new(self)
                .trailing_to_leading(&sync_button, &edit_button, 8.0)
                .centerY(&sync_button, 0.0)
                .width(&sync_button, 60.0)
                .activate();
            
            AutoLayout::new(self)
                .trailing_to_leading(&sync_progress, &sync_button, 12.0)
                .centerY(&sync_progress, 0.0)
                .width(&sync_progress, PROGRESS_WIDTH)
                .activate();
            
            // Store references
            self.ivars().icon_view.lock().unwrap().replace(icon_view);
            self.ivars().name_label.lock().unwrap().replace(name_label);
            self.ivars().status_label.lock().unwrap().replace(status_label);
            self.ivars().library_count_label.lock().unwrap().replace(library_count_label);
            self.ivars().sync_progress.lock().unwrap().replace(sync_progress);
            self.ivars().sync_button.lock().unwrap().replace(sync_button);
            self.ivars().edit_button.lock().unwrap().replace(edit_button);
            self.ivars().delete_button.lock().unwrap().replace(delete_button);
        }
    }
    
    pub fn configure(&self, source_info: &SourceInfo) {
        unsafe {
            // Set icon based on backend type
            if let Some(icon_view) = self.ivars().icon_view.lock().unwrap().as_ref() {
                let icon_name = match source_info.source.backend_type.as_str() {
                    "plex" => "plex-icon",
                    "jellyfin" => "jellyfin-icon",
                    _ => "folder-icon",
                };
                // TODO: Load actual icons
                icon_view.setImage(None);
            }
            
            // Set name
            if let Some(name_label) = self.ivars().name_label.lock().unwrap().as_ref() {
                name_label.setStringValue(&NSString::from_str(&source_info.source.name));
            }
            
            // Set status
            if let Some(status_label) = self.ivars().status_label.lock().unwrap().as_ref() {
                let status_text = if source_info.source.is_online {
                    if source_info.is_syncing {
                        format!("Syncing... {:.0}%", source_info.sync_progress * 100.0)
                    } else if let Some(ref error) = source_info.last_error {
                        format!("Error: {}", error)
                    } else {
                        "Online".to_string()
                    }
                } else {
                    "Offline".to_string()
                };
                
                status_label.setStringValue(&NSString::from_str(&status_text));
                
                let color = if source_info.source.is_online {
                    if source_info.last_error.is_some() {
                        NSColor::systemRedColor()
                    } else {
                        NSColor::systemGreenColor()
                    }
                } else {
                    NSColor::systemOrangeColor()
                };
                status_label.setTextColor(Some(&color));
            }
            
            // Set library count
            if let Some(count_label) = self.ivars().library_count_label.lock().unwrap().as_ref() {
                let count_text = format!("{} libraries", source_info.libraries.len());
                count_label.setStringValue(&NSString::from_str(&count_text));
            }
            
            // Configure sync progress
            if let Some(progress) = self.ivars().sync_progress.lock().unwrap().as_ref() {
                progress.setHidden(!source_info.is_syncing);
                if source_info.is_syncing {
                    progress.setDoubleValue(source_info.sync_progress as f64 * 100.0);
                }
            }
            
            // Configure sync button
            if let Some(button) = self.ivars().sync_button.lock().unwrap().as_ref() {
                button.setEnabled(!source_info.is_syncing);
                let title = if source_info.is_syncing {
                    "Syncing"
                } else {
                    "Sync"
                };
                button.setTitle(&NSString::from_str(title));
            }
        }
    }
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
        
        // Create container
        let container = unsafe { NSView::new() };
        
        // Create toolbar
        let toolbar = Self::create_toolbar()?;
        
        // Create table view
        let table_view = unsafe {
            let tv = NSTableView::new();
            tv.setRowHeight(ROW_HEIGHT);
            tv.setAllowsColumnReordering(false);
            tv.setAllowsColumnResizing(false);
            tv.setAllowsMultipleSelection(false);
            tv.setUsesAlternatingRowBackgroundColors(true);
            
            // Create single column for source cells
            let column = NSTableColumn::initWithIdentifier(
                NSTableColumn::alloc(),
                &NSString::from_str("sourceColumn")
            );
            column.setWidth(800.0);
            column.setMinWidth(600.0);
            column.setTitle(&NSString::from_str("Sources"));
            tv.addTableColumn(&column);
            
            tv
        };
        
        // Create scroll view
        let scroll_view = unsafe {
            let sv = NSScrollView::new();
            sv.setDocumentView(Some(&table_view));
            sv.setHasVerticalScroller(true);
            sv.setHasHorizontalScroller(false);
            sv.setAutohidesScrollers(true);
            sv.setBorderType(NSBorderType::NSNoBorder);
            sv.setTranslatesAutoresizingMaskIntoConstraints(false);
            sv
        };
        
        // Create add button
        let add_button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Add Source"),
                None,
                Some(sel!(addSource:))
            )
        };
        unsafe {
            add_button.setBezelStyle(NSBezelStyle::NSBezelStyleRounded);
            add_button.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        
        // Create refresh button
        let refresh_button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Refresh"),
                None,
                Some(sel!(refreshSources:))
            )
        };
        unsafe {
            refresh_button.setBezelStyle(NSBezelStyle::NSBezelStyleRounded);
            refresh_button.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        
        // Create status label
        let status_label = unsafe {
            NSTextField::labelWithString(&NSString::from_str("0 sources"))
        };
        unsafe {
            status_label.setFont(Some(&NSFont::systemFontOfSize(13.0)));
            status_label.setTextColor(Some(&NSColor::secondaryLabelColor()));
            status_label.setTranslatesAutoresizingMaskIntoConstraints(false);
        }
        
        // Setup toolbar
        unsafe {
            toolbar.addSubview(&add_button);
            toolbar.addSubview(&refresh_button);
            toolbar.addSubview(&status_label);
            
            AutoLayout::new(&toolbar)
                .leading(&add_button, 16.0)
                .centerY(&add_button, 0.0)
                .activate();
            
            AutoLayout::new(&toolbar)
                .leading_to_trailing(&refresh_button, &add_button, 8.0)
                .centerY(&refresh_button, 0.0)
                .activate();
            
            AutoLayout::new(&toolbar)
                .trailing(&status_label, 16.0)
                .centerY(&status_label, 0.0)
                .activate();
        }
        
        // Setup container layout
        unsafe {
            container.addSubview(&toolbar);
            container.addSubview(&scroll_view);
            
            AutoLayout::new(&container)
                .top(&toolbar, 0.0)
                .leading(&toolbar, 0.0)
                .trailing(&toolbar, 0.0)
                .height(&toolbar, 44.0)
                .activate();
            
            AutoLayout::new(&container)
                .top_to_bottom(&scroll_view, &toolbar, 0.0)
                .leading(&scroll_view, 0.0)
                .trailing(&scroll_view, 0.0)
                .bottom(&scroll_view, 0.0)
                .activate();
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
        sources_view.setup_actions()?;
        
        Ok(sources_view)
    }
    
    fn create_toolbar() -> CocoaResult<Retained<NSView>> {
        unsafe {
            let toolbar = NSView::new();
            toolbar.setWantsLayer(true);
            toolbar.layer().unwrap().setBackgroundColor(
                NSColor::controlBackgroundColor().CGColor()
            );
            toolbar.setTranslatesAutoresizingMaskIntoConstraints(false);
            Ok(toolbar)
        }
    }
    
    fn setup_bindings(&mut self) -> CocoaResult<()> {
        debug!("Setting up SourcesView bindings");
        
        // Subscribe to sources property
        let sources_property = self.view_model.sources();
        let data_source = self.data_source.clone();
        let table_view = self.table_view.clone();
        let status_label = self.status_label.clone();
        
        sources_property.subscribe(Box::new(move |sources| {
            debug!("Sources updated, count: {}", sources.len());
            
            let data_source = data_source.clone();
            let table_view = table_view.clone();
            let status_label = status_label.clone();
            let sources = sources.clone();
            
            tokio::spawn(async move {
                {
                    let mut data = data_source.write().await;
                    *data = sources.clone();
                }
                
                Queue::main().exec_async(move || {
                    unsafe {
                        table_view.reloadData();
                        
                        let online_count = sources.iter().filter(|s| s.source.is_online).count();
                        let status_text = format!(
                            "{} sources ({} online)",
                            sources.len(),
                            online_count
                        );
                        status_label.setStringValue(&NSString::from_str(&status_text));
                    }
                });
            });
        }));
        
        // Subscribe to loading state
        let loading_property = self.view_model.is_loading();
        let refresh_button = self.refresh_button.clone();
        
        loading_property.subscribe(Box::new(move |is_loading| {
            let refresh_button = refresh_button.clone();
            let loading = *is_loading;
            
            Queue::main().exec_async(move || {
                unsafe {
                    refresh_button.setEnabled(!loading);
                    let title = if loading { "Loading..." } else { "Refresh" };
                    refresh_button.setTitle(&NSString::from_str(title));
                }
            });
        }));
        
        Ok(())
    }
    
    fn setup_table_view(&mut self) -> CocoaResult<()> {
        debug!("Setting up table view data source and delegate");
        
        // In a production implementation, we'd properly implement NSTableViewDataSource
        // and NSTableViewDelegate protocols. For now, this is a simplified approach.
        
        unsafe {
            msg_send![&self.table_view, setDataSource: self as *const _ as *mut NSObject];
            msg_send![&self.table_view, setDelegate: self as *const _ as *mut NSObject];
        }
        
        Ok(())
    }
    
    fn setup_actions(&mut self) -> CocoaResult<()> {
        debug!("Setting up button actions");
        
        unsafe {
            self.add_button.setTarget(Some(&*self.add_button));
            self.refresh_button.setTarget(Some(&*self.refresh_button));
        }
        
        Ok(())
    }
    
    pub fn view(&self) -> &NSView {
        &self.container
    }
    
    pub async fn refresh(&self) -> CocoaResult<()> {
        info!("Refreshing sources");
        self.view_model.load_sources().await
            .map_err(|e| CocoaError::ViewModelError(e.to_string()))?;
        Ok(())
    }
    
    pub async fn add_source(&self, backend_type: BackendType) -> CocoaResult<()> {
        info!("Adding new source: {:?}", backend_type);
        
        match backend_type {
            BackendType::Plex => self.show_plex_auth_dialog().await?,
            BackendType::Jellyfin => self.show_jellyfin_auth_dialog().await?,
            BackendType::Local => self.show_local_folder_dialog().await?,
        }
        
        Ok(())
    }
    
    async fn show_plex_auth_dialog(&self) -> CocoaResult<()> {
        info!("Showing Plex authentication dialog");
        
        match show_auth_dialog_for_backend(BackendType::Plex).await {
            Ok(credentials) => {
                // Create a new source with the credentials
                let source_name = if let Credentials::Plex(ref plex) = credentials {
                    format!("Plex Server")
                } else {
                    "Plex".to_string()
                };
                
                // Store credentials in keychain
                let source_id = uuid::Uuid::new_v4().to_string();
                if let Err(e) = store_credentials_in_keychain(&source_id, &credentials) {
                    error!("Failed to store credentials in keychain: {}", e);
                }
                
                // Add source to database via DataService
                // TODO: Implement source creation in DataService
                info!("Plex source authenticated successfully");
                
                // Refresh sources list
                self.refresh().await?;
                Ok(())
            }
            Err(CocoaError::UserCancelled) => {
                debug!("User cancelled Plex authentication");
                Ok(())
            }
            Err(e) => {
                error!("Plex authentication failed: {}", e);
                Err(e)
            }
        }
    }
    
    async fn show_jellyfin_auth_dialog(&self) -> CocoaResult<()> {
        info!("Showing Jellyfin authentication dialog");
        
        match show_auth_dialog_for_backend(BackendType::Jellyfin).await {
            Ok(credentials) => {
                // Create a new source with the credentials
                let source_name = if let Credentials::Jellyfin(ref jf) = credentials {
                    format!("Jellyfin - {}", jf.server_url)
                } else {
                    "Jellyfin".to_string()
                };
                
                // Store credentials in keychain
                let source_id = uuid::Uuid::new_v4().to_string();
                if let Err(e) = store_credentials_in_keychain(&source_id, &credentials) {
                    error!("Failed to store credentials in keychain: {}", e);
                }
                
                // Add source to database via DataService
                // TODO: Implement source creation in DataService
                info!("Jellyfin source authenticated successfully");
                
                // Refresh sources list
                self.refresh().await?;
                Ok(())
            }
            Err(CocoaError::UserCancelled) => {
                debug!("User cancelled Jellyfin authentication");
                Ok(())
            }
            Err(e) => {
                error!("Jellyfin authentication failed: {}", e);
                Err(e)
            }
        }
    }
    
    async fn show_local_folder_dialog(&self) -> CocoaResult<()> {
        // In production, this would show a folder selection dialog
        // For now, this is a placeholder
        warn!("Local folder dialog not yet implemented");
        Ok(())
    }
    
    pub async fn delete_source(&self, source_id: &str) -> CocoaResult<()> {
        info!("Deleting source: {}", source_id);
        
        // Show confirmation dialog
        let alert = unsafe {
            let alert = NSAlert::new();
            alert.setMessageText(&NSString::from_str("Delete Source"));
            alert.setInformativeText(&NSString::from_str(
                "Are you sure you want to delete this source? This action cannot be undone."
            ));
            alert.addButtonWithTitle(&NSString::from_str("Delete"));
            alert.addButtonWithTitle(&NSString::from_str("Cancel"));
            alert.setAlertStyle(NSAlertStyle::NSAlertStyleWarning);
            alert
        };
        
        unsafe {
            let response = alert.runModal();
            if response == NSModalResponse::NSAlertFirstButtonReturn {
                // TODO: Implement source deletion in DataService
                warn!("Source deletion not yet implemented in DataService");
            }
        }
        
        Ok(())
    }
    
    pub async fn sync_source(&self, source_id: &str) -> CocoaResult<()> {
        info!("Syncing source: {}", source_id);
        self.view_model.sync_source(source_id.to_string()).await
            .map_err(|e| CocoaError::ViewModelError(e.to_string()))?;
        Ok(())
    }
}

// NSTableViewDataSource implementation helpers
impl SourcesView {
    extern "C" fn number_of_rows_in_table_view(&self, _tv: &NSTableView) -> NSInteger {
        let data = self.data_source.clone();
        tokio::runtime::Handle::current().block_on(async {
            data.read().await.len() as NSInteger
        })
    }
    
    extern "C" fn table_view_view_for_table_column_row(
        &self,
        _tv: &NSTableView,
        _column: &NSTableColumn,
        row: NSInteger,
    ) -> Retained<NSView> {
        let data = self.data_source.clone();
        
        if let Some(source_info) = tokio::runtime::Handle::current().block_on(async {
            data.read().await.get(row as usize).cloned()
        }) {
            unsafe {
                let cell = SourceCellView::new();
                cell.configure(&source_info);
                cell
            }
        } else {
            unsafe { NSView::new() }
        }
    }
    
    extern "C" fn table_view_should_select_row(&self, _tv: &NSTableView, row: NSInteger) -> bool {
        let mut selected = tokio::runtime::Handle::current().block_on(
            self.selected_index.write()
        );
        *selected = Some(row);
        true
    }
}

// Keychain integration for credential storage
fn store_credentials_in_keychain(source_id: &str, credentials: &Credentials) -> Result<(), keyring::Error> {
    let service = "dev.arsfeld.Reel";
    let entry = Entry::new(service, &format!("source_{}", source_id))?;
    
    let credential_json = serde_json::to_string(credentials)
        .map_err(|e| keyring::Error::NoEntry)?;
    
    entry.set_password(&credential_json)?;
    Ok(())
}

fn get_credentials_from_keychain(source_id: &str) -> Result<Credentials, keyring::Error> {
    let service = "dev.arsfeld.Reel";
    let entry = Entry::new(service, &format!("source_{}", source_id))?;
    
    let credential_json = entry.get_password()?;
    let credentials = serde_json::from_str(&credential_json)
        .map_err(|e| keyring::Error::NoEntry)?;
    
    Ok(credentials)
}

fn delete_credentials_from_keychain(source_id: &str) -> Result<(), keyring::Error> {
    let service = "dev.arsfeld.Reel";
    let entry = Entry::new(service, &format!("source_{}", source_id))?;
    entry.delete_password()?;
    Ok(())
}