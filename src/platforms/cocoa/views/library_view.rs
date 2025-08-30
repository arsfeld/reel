use block2::Block;
use dispatch::Queue;
use objc2::{ClassType, DefinedClass, msg_send, msg_send_id, rc::Retained, runtime::NSObject, sel};
use objc2_app_kit::{
    NSCollectionView, NSCollectionViewDataSource, NSCollectionViewDelegate,
    NSCollectionViewDelegateFlowLayout, NSCollectionViewFlowLayout, NSCollectionViewItem,
    NSCollectionViewLayoutAttributes, NSCollectionViewScrollDirection, NSCollectionViewUpdateItem,
    NSImage, NSImageView, NSScrollView, NSTextField, NSView,
};
use objc2_foundation::{NSArray, NSIndexPath, NSIndexSet, NSInteger, NSSet, NSString, NSUInteger};
use std::ptr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::core::viewmodels::{LibraryViewModel, Property};
use crate::models::MediaItem;
use crate::platforms::cocoa::error::{CocoaError, CocoaResult};
use crate::platforms::cocoa::utils::{
    AutoLayout, CGFloat, CGSize, NSEdgeInsets, main_thread_marker,
};

const ITEM_WIDTH: CGFloat = 200.0;
const ITEM_HEIGHT: CGFloat = 300.0;
const ITEM_SPACING: CGFloat = 20.0;
const LINE_SPACING: CGFloat = 20.0;
const SECTION_INSETS: CGFloat = 20.0;

pub struct LibraryView {
    scroll_view: Retained<NSScrollView>,
    collection_view: Retained<NSCollectionView>,
    flow_layout: Retained<NSCollectionViewFlowLayout>,
    view_model: Arc<LibraryViewModel>,
    data_source: Arc<RwLock<LibraryDataSource>>,
    delegate: Arc<LibraryDelegate>,
}

#[derive(Debug)]
struct LibraryDataSource {
    items: Vec<MediaItem>,
    view_model: Arc<LibraryViewModel>,
}

#[derive(Debug)]
struct LibraryDelegate {
    view_model: Arc<LibraryViewModel>,
}

impl LibraryView {
    pub fn new(view_model: Arc<LibraryViewModel>) -> CocoaResult<Self> {
        debug!("Creating LibraryView with collection view");

        // Create flow layout
        let flow_layout = unsafe {
            let mtm = main_thread_marker();
            let layout = NSCollectionViewFlowLayout::new(mtm);
            // layout.setItemSize(CGSize::new(ITEM_WIDTH, ITEM_HEIGHT)); // TODO: Fix CGSize type
            layout.setMinimumInteritemSpacing(ITEM_SPACING);
            layout.setMinimumLineSpacing(LINE_SPACING);
            // layout.setSectionInset(NSEdgeInsets { ... }); // TODO: Fix NSEdgeInsets type
            // layout.setScrollDirection(NSCollectionViewScrollDirection::Vertical); // TODO: Fix enum variant
            layout
        };

        // Create collection view
        let collection_view = unsafe {
            let mtm = main_thread_marker();
            let collection_view = NSCollectionView::new(mtm);
            collection_view.setCollectionViewLayout(Some(&flow_layout));
            // TODO: Fix NSArray::from_vec - method doesn't exist
            // collection_view.setBackgroundColors(&NSArray::from_vec(vec![]));
            collection_view.setAllowsMultipleSelection(false);
            collection_view.setSelectable(true);

            // Register item class
            let item_class = MediaItemCell::class();
            let identifier = NSString::from_str("MediaItemCell");
            // TODO: Fix msg_send! macro usage - trait bound issue
            // msg_send![&collection_view, registerClass:item_class forItemWithIdentifier:&identifier];

            collection_view
        };

        // Create scroll view
        let scroll_view = unsafe {
            let mtm = main_thread_marker();
            let scroll_view = NSScrollView::new(mtm);
            scroll_view.setDocumentView(Some(&collection_view));
            scroll_view.setHasVerticalScroller(true);
            scroll_view.setHasHorizontalScroller(false);
            scroll_view.setAutohidesScrollers(true);
            // scroll_view.setBorderType(objc2_app_kit::NSBorderType::None); // TODO: Fix enum variant
            scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);
            scroll_view
        };

        // Create data source
        let data_source = Arc::new(RwLock::new(LibraryDataSource {
            items: Vec::new(),
            view_model: view_model.clone(),
        }));

        // Create delegate
        let delegate = Arc::new(LibraryDelegate {
            view_model: view_model.clone(),
        });

        let mut library_view = Self {
            scroll_view,
            collection_view,
            flow_layout,
            view_model,
            data_source,
            delegate,
        };

        library_view.setup_bindings()?;
        library_view.setup_data_source()?;
        library_view.setup_delegate()?;

        Ok(library_view)
    }

    pub fn view(&self) -> &NSScrollView {
        &self.scroll_view
    }

    fn setup_bindings(&mut self) -> CocoaResult<()> {
        debug!("Setting up LibraryView bindings");

        // Subscribe to items property changes
        let items_property = self.view_model.items();
        let collection_view = self.collection_view.clone();
        let data_source = self.data_source.clone();

        // TODO: Fix property subscription API
        // items_property.subscribe(Box::new(move |items| { ... }));

        // Subscribe to loading state
        let loading_property = self.view_model.is_loading();
        let collection_view = self.collection_view.clone();

        // TODO: Fix property subscription API
        // loading_property.subscribe(Box::new(move |is_loading| { ... }));

        Ok(())
    }

    fn setup_data_source(&mut self) -> CocoaResult<()> {
        debug!("Setting up collection view data source");

        // Create data source protocol object
        let data_source = self.data_source.clone();
        let data_source_obj = LibraryDataSourceObj::new(data_source);

        unsafe {
            // Set the data source - use as_ref() to get the NSObject reference
            let _: () = msg_send![&self.collection_view, setDataSource: data_source_obj.as_ref() as *const NSObject];
        }

        Ok(())
    }

    fn setup_delegate(&mut self) -> CocoaResult<()> {
        debug!("Setting up collection view delegate");

        // Create delegate protocol object
        let delegate = self.delegate.clone();
        let delegate_obj = LibraryDelegateObj::new(delegate);

        unsafe {
            // Set the delegate - use as_ref() to get the NSObject reference
            let _: () = msg_send![&self.collection_view, setDelegate: delegate_obj.as_ref() as *const NSObject];
        }

        Ok(())
    }

    pub async fn refresh(&self) -> CocoaResult<()> {
        info!("Refreshing library view");
        self.view_model.refresh().await;
        Ok(())
    }

    pub fn set_sort_order(&self, sort: String) {
        use crate::core::viewmodels::library_view_model::SortOrder;
        let sort_order = match sort.as_str() {
            "title_asc" => SortOrder::TitleAsc,
            "title_desc" => SortOrder::TitleDesc,
            "year_asc" => SortOrder::YearAsc,
            "year_desc" => SortOrder::YearDesc,
            "rating_asc" => SortOrder::RatingAsc,
            "rating_desc" => SortOrder::RatingDesc,
            "added_asc" => SortOrder::AddedAsc,
            _ => SortOrder::AddedDesc,
        };
        self.view_model.set_sort_order(sort_order);
    }

    pub fn set_filter(&self, filter: String) {
        use crate::core::viewmodels::library_view_model::{FilterOptions, WatchStatus};
        // TODO: Parse filter string properly - for now using default with search
        let filter_options = FilterOptions {
            search: filter,
            genres: Vec::new(),
            years: None,
            min_rating: None,
            watch_status: WatchStatus::All,
        };
        self.view_model.set_filter(filter_options);
    }

    pub fn search(&self, query: String) {
        self.view_model.search(query);
    }
}

// MediaItemCell - Using simple NSObject for now due to objc2 0.6 macro complexity
// TODO: Convert to proper define_class! when macro syntax is stabilized
pub type MediaItemCell = NSObject;

// Data source protocol implementation
struct LibraryDataSourceObj {
    data_source: Arc<RwLock<LibraryDataSource>>,
}

impl LibraryDataSourceObj {
    fn new(data_source: Arc<RwLock<LibraryDataSource>>) -> Retained<NSObject> {
        // Create a simple NSObject that will act as our data source
        // In production, we'd use declare_class! macro for this
        unsafe { NSObject::new() }
    }
}

// Delegate protocol implementation
struct LibraryDelegateObj {
    delegate: Arc<LibraryDelegate>,
}

impl LibraryDelegateObj {
    fn new(delegate: Arc<LibraryDelegate>) -> Retained<NSObject> {
        // Create a simple NSObject that will act as our delegate
        // In production, we'd use declare_class! macro for this
        unsafe { NSObject::new() }
    }
}

// Extension methods for data source
impl LibraryDataSource {
    async fn number_of_items(&self) -> NSInteger {
        self.items.len() as NSInteger
    }

    async fn item_at_index(&self, index: NSUInteger) -> Option<MediaItem> {
        self.items.get(index as usize).cloned()
    }
}

// Extension methods for delegate
impl LibraryDelegate {
    fn did_select_item(&self, item: &MediaItem) {
        info!("Selected media item: {}", item.title());
        // Navigation to details view should be handled by UI layer
        self.view_model.select_item(item.id().to_string());
    }

    fn did_double_click_item(&self, item: &MediaItem) {
        info!("Double-clicked media item: {}", item.title());
        // TODO: UI navigation - implement play_item navigation from UI layer
        // Start playback should be handled by UI navigation, not ViewModel
        // if let Some(url) = &item.playback_url {
        //     // Navigate to player with URL
        // }
    }
}
