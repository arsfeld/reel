use crate::platforms::cocoa::utils::{CGFloat, CGSize, main_thread_marker};
use dispatch::Queue;
use objc2::{ClassType, msg_send, msg_send_id, rc::Retained, runtime::NSObject, sel};
use objc2_app_kit::{
    NSCollectionView, NSCollectionViewFlowLayout, NSCollectionViewScrollDirection,
    NSLayoutAttribute, NSLayoutConstraint, NSLayoutRelation, NSScrollView, NSStackView,
    NSStackViewDistribution, NSTextField, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSArray, NSIndexPath, NSString};
use std::sync::Arc;
use tracing::{debug, info};

use crate::core::viewmodels::{HomeViewModel, Property};
use crate::models::MediaItem;
use crate::platforms::cocoa::error::{CocoaError, CocoaResult};
use crate::platforms::cocoa::utils::{AutoLayout, NSEdgeInsets};
use crate::platforms::cocoa::views::library_view::MediaItemCell;

const SECTION_HEIGHT: CGFloat = 320.0;
const ITEM_WIDTH: CGFloat = 150.0;
const ITEM_HEIGHT: CGFloat = 225.0;
const ITEM_SPACING: CGFloat = 10.0;

pub struct HomeView {
    container: Retained<NSStackView>,
    sections: Vec<HomeSection>,
    view_model: Arc<HomeViewModel>,
}

#[derive(Debug)]
struct HomeSection {
    title_label: Retained<NSTextField>,
    scroll_view: Retained<NSScrollView>,
    collection_view: Retained<NSCollectionView>,
    items: Vec<MediaItem>,
}

impl HomeView {
    pub fn new(view_model: Arc<HomeViewModel>) -> CocoaResult<Self> {
        debug!("Creating HomeView with sections");

        // Create main container stack view
        let container = unsafe {
            let mtm = main_thread_marker();
            let stack = NSStackView::new(mtm);
            // stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical); // TODO: Fix enum variant
            stack.setDistribution(NSStackViewDistribution::Fill);
            stack.setSpacing(20.0);
            stack.setTranslatesAutoresizingMaskIntoConstraints(false);

            // Add padding
            // stack.setEdgeInsets(NSEdgeInsets { ... }); // TODO: Fix NSEdgeInsets type

            stack
        };

        let mut home_view = Self {
            container,
            sections: Vec::new(),
            view_model,
        };

        home_view.setup_sections()?;
        home_view.setup_bindings()?;

        Ok(home_view)
    }

    pub fn view(&self) -> &NSStackView {
        &self.container
    }

    fn setup_sections(&mut self) -> CocoaResult<()> {
        debug!("Setting up home view sections");

        // Create sections
        let section_titles = [
            "Continue Watching",
            "Recently Added",
            "Movies",
            "TV Shows",
            "Up Next",
        ];

        for title in &section_titles {
            let section = self.create_section(title)?;
            self.sections.push(section);
        }

        Ok(())
    }

    fn create_section(&self, title: &str) -> CocoaResult<HomeSection> {
        debug!("Creating section: {}", title);

        // Create section container
        let section_container = unsafe {
            let mtm = main_thread_marker();
            let container = NSView::new(mtm);
            container.setTranslatesAutoresizingMaskIntoConstraints(false);
            container
        };

        // Create title label
        let title_label = unsafe {
            let mtm = main_thread_marker();
            let label = NSTextField::new(mtm);
            label.setStringValue(&NSString::from_str(title));
            label.setEditable(false);
            label.setBordered(false);
            label.setBackgroundColor(None);
            label.setFont(Some(&objc2_app_kit::NSFont::systemFontOfSize_weight(
                18.0, 0.0, // TODO: Fix NSFontWeightSemibold constant
            )));
            label.setTranslatesAutoresizingMaskIntoConstraints(false);
            label
        };

        // Create horizontal flow layout
        let flow_layout = unsafe {
            let mtm = main_thread_marker();
            let layout = NSCollectionViewFlowLayout::new(mtm);
            // layout.setItemSize(CGSize::new(ITEM_WIDTH, ITEM_HEIGHT)); // TODO: Fix CGSize type
            layout.setMinimumInteritemSpacing(ITEM_SPACING);
            layout.setMinimumLineSpacing(ITEM_SPACING);
            // layout.setSectionInset(NSEdgeInsets { ... }); // TODO: Fix NSEdgeInsets type
            // layout.setScrollDirection(NSCollectionViewScrollDirection::Horizontal); // TODO: Fix enum variant
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

        // Create horizontal scroll view
        let scroll_view = unsafe {
            let mtm = main_thread_marker();
            let scroll_view = NSScrollView::new(mtm);
            scroll_view.setDocumentView(Some(&collection_view));
            scroll_view.setHasVerticalScroller(false);
            scroll_view.setHasHorizontalScroller(true);
            scroll_view.setAutohidesScrollers(true);
            // scroll_view.setBorderType(objc2_app_kit::NSBorderType::None); // TODO: Fix enum variant
            scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);
            scroll_view
        };

        // Add subviews
        unsafe {
            section_container.addSubview(&title_label);
            section_container.addSubview(&scroll_view);
        }

        // Setup constraints
        // TODO: Section container constraints simplified - removed complex layout

        // TODO: Section container constraints simplified - complex layout removed

        // Set fixed height for section container
        unsafe {
            let height_constraint = NSLayoutConstraint::constraintWithItem_attribute_relatedBy_toItem_attribute_multiplier_constant(
                &section_container,
                NSLayoutAttribute::Height,
                NSLayoutRelation::Equal,
                None,
                NSLayoutAttribute::NotAnAttribute,
                1.0,
                SECTION_HEIGHT
            );
            section_container.addConstraint(&height_constraint);
        }

        // Add to main container
        unsafe {
            self.container.addArrangedSubview(&section_container);
        }

        Ok(HomeSection {
            title_label,
            scroll_view,
            collection_view,
            items: Vec::new(),
        })
    }

    fn setup_bindings(&mut self) -> CocoaResult<()> {
        debug!("Setting up HomeView bindings");

        // Subscribe to continue watching items
        let continue_watching = self.view_model.continue_watching();
        if self.sections.len() > 0 {
            Self::bind_section_items(continue_watching, &self.sections[0]);
        }

        // Subscribe to recently added items
        let recently_added = self.view_model.recently_added();
        if self.sections.len() > 1 {
            Self::bind_section_items(recently_added, &self.sections[1]);
        }

        // Subscribe to continue watching items (previously called up_next)
        let continue_watching = self.view_model.continue_watching();
        if self.sections.len() > 4 {
            Self::bind_section_items(continue_watching, &self.sections[4]);
        }

        Ok(())
    }

    fn bind_section_items(property: &Property<Vec<MediaItem>>, section: &HomeSection) {
        let collection_view = section.collection_view.clone();

        // TODO: Fix property subscription API
        // property.subscribe(Box::new(move |items| { ... }));
    }

    pub async fn refresh(&self) -> CocoaResult<()> {
        info!("Refreshing home view");
        self.view_model.refresh().await;
        Ok(())
    }

    pub fn navigate_to_section(&self, section_index: usize) {
        if section_index < self.sections.len() {
            info!("Navigating to section {}", section_index);
            // Could implement "See All" navigation here
        }
    }
}
