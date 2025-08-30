use crate::platforms::cocoa::utils::{CGFloat, CGSize, main_thread_marker};
use dispatch::Queue;
use objc2::{class, msg_send, msg_send_id, rc::Retained, runtime::NSObject, sel};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSButtonType, NSFont, NSFontWeight, NSImage, NSImageView,
    NSLayoutAttribute, NSLineBreakMode, NSScrollView, NSStackView, NSStackViewDistribution,
    NSTextAlignment, NSTextField, NSTextView, NSUserInterfaceLayoutOrientation, NSView,
};
use objc2_foundation::{NSArray, NSData, NSString};
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::core::viewmodels::{DetailsViewModel, Property, ViewModel};
use crate::models::{MediaItem, Movie, Show};
use crate::platforms::cocoa::error::{CocoaError, CocoaResult};
use crate::platforms::cocoa::utils::{AutoLayout, NSEdgeInsets};

const POSTER_WIDTH: CGFloat = 300.0;
const POSTER_HEIGHT: CGFloat = 450.0;
const BACKDROP_HEIGHT: CGFloat = 300.0;

pub struct DetailsView {
    scroll_view: Retained<NSScrollView>,
    container: Retained<NSStackView>,
    backdrop_image: Retained<NSImageView>,
    poster_image: Retained<NSImageView>,
    title_label: Retained<NSTextField>,
    metadata_label: Retained<NSTextField>,
    synopsis_view: Retained<NSTextView>,
    play_button: Retained<NSButton>,
    watchlist_button: Retained<NSButton>,
    view_model: Arc<DetailsViewModel>,
}

impl DetailsView {
    pub fn new(view_model: Arc<DetailsViewModel>) -> CocoaResult<Self> {
        debug!("Creating DetailsView");

        // Create scroll view
        let scroll_view = unsafe {
            let mtm = main_thread_marker();
            let scroll_view = NSScrollView::new(mtm);
            scroll_view.setHasVerticalScroller(true);
            scroll_view.setHasHorizontalScroller(false);
            scroll_view.setAutohidesScrollers(true);
            // setBorderType not available in objc2-app-kit 0.3 - skip for now
            scroll_view.setTranslatesAutoresizingMaskIntoConstraints(false);
            scroll_view
        };

        // Create main container
        let container = unsafe {
            let mtm = main_thread_marker();
            let stack = NSStackView::new(mtm);
            let _: () = msg_send![&stack, setOrientation: 1]; // NSUserInterfaceLayoutOrientationVertical = 1
            stack.setDistribution(NSStackViewDistribution::Fill);
            stack.setSpacing(20.0);
            stack.setTranslatesAutoresizingMaskIntoConstraints(false);
            stack
        };

        // Create backdrop image view
        let backdrop_image = unsafe {
            let mtm = main_thread_marker();
            let image_view = NSImageView::new(mtm);
            let _: () = msg_send![&image_view, setImageScaling: 0]; // NSImageScaleProportionallyDown = 0
            image_view.setTranslatesAutoresizingMaskIntoConstraints(false);
            image_view
        };

        // Create content container (horizontal stack)
        let content_container = unsafe {
            let mtm = main_thread_marker();
            let stack = NSStackView::new(mtm);
            let _: () = msg_send![&stack, setOrientation: 0]; // NSUserInterfaceLayoutOrientationHorizontal = 0
            stack.setDistribution(NSStackViewDistribution::Fill);
            stack.setSpacing(30.0);
            let insets = NSEdgeInsets {
                top: 20.0,
                left: 20.0,
                bottom: 20.0,
                right: 20.0,
            };
            let _: () = msg_send![&stack, setEdgeInsets: insets];
            stack
        };

        // Create poster image view
        let poster_image = unsafe {
            let mtm = main_thread_marker();
            let image_view = NSImageView::new(mtm);
            let _: () = msg_send![&image_view, setImageScaling: 0]; // NSImageScaleProportionallyDown = 0
            image_view.setTranslatesAutoresizingMaskIntoConstraints(false);
            image_view
        };

        // Create info container (vertical stack)
        let info_container = unsafe {
            let mtm = main_thread_marker();
            let stack = NSStackView::new(mtm);
            let _: () = msg_send![&stack, setOrientation: 1]; // NSUserInterfaceLayoutOrientationVertical = 1
            stack.setDistribution(NSStackViewDistribution::Fill);
            stack.setSpacing(15.0);
            stack.setAlignment(objc2_app_kit::NSLayoutAttribute::Leading);
            stack
        };

        // Create title label
        let title_label = unsafe {
            let mtm = main_thread_marker();
            let label = NSTextField::new(mtm);
            label.setEditable(false);
            label.setBordered(false);
            label.setBackgroundColor(None);
            label.setFont(Some(&NSFont::systemFontOfSize_weight(
                28.0, 700.0, // NSFontWeightBold = 700
            )));
            let _: () = msg_send![&label, setLineBreakMode: 4]; // NSLineBreakByTruncatingTail = 4
            label.setMaximumNumberOfLines(2);
            label
        };

        // Create metadata label (year, rating, duration)
        let metadata_label = unsafe {
            let mtm = main_thread_marker();
            let label = NSTextField::new(mtm);
            label.setEditable(false);
            label.setBordered(false);
            label.setBackgroundColor(None);
            label.setFont(Some(&NSFont::systemFontOfSize(14.0)));
            label.setTextColor(Some(&objc2_app_kit::NSColor::secondaryLabelColor()));
            label
        };

        // Create synopsis text view
        let synopsis_view = unsafe {
            let mtm = main_thread_marker();
            let text_view = NSTextView::new(mtm);
            text_view.setEditable(false);
            let clear_color = objc2_app_kit::NSColor::clearColor();
            text_view.setBackgroundColor(&clear_color);
            text_view.setFont(Some(&NSFont::systemFontOfSize(14.0)));
            let inset = CGSize::new(0.0, 0.0);
            let _: () = msg_send![&text_view, setTextContainerInset: inset];
            text_view
        };

        // Create button container
        let button_container = unsafe {
            let mtm = main_thread_marker();
            let stack = NSStackView::new(mtm);
            let _: () = msg_send![&stack, setOrientation: 0]; // NSUserInterfaceLayoutOrientationHorizontal = 0
            stack.setDistribution(NSStackViewDistribution::FillEqually);
            stack.setSpacing(10.0);
            stack
        };

        // Create play button
        let play_button = unsafe {
            let mtm = main_thread_marker();
            let button = NSButton::new(mtm);
            button.setTitle(&NSString::from_str("▶️ Play"));
            button.setBezelStyle(NSBezelStyle::Rounded);
            button.setButtonType(NSButtonType::MomentaryPushIn);
            button.setKeyEquivalent(&NSString::from_str("\r")); // Enter key
            button
        };

        // Create watchlist button
        let watchlist_button = unsafe {
            let mtm = main_thread_marker();
            let button = NSButton::new(mtm);
            button.setTitle(&NSString::from_str("➕ Add to Watchlist"));
            button.setBezelStyle(NSBezelStyle::Rounded);
            button.setButtonType(NSButtonType::MomentaryPushIn);
            button
        };

        // Assemble the view hierarchy
        unsafe {
            // Add buttons to button container
            button_container.addArrangedSubview(&play_button);
            button_container.addArrangedSubview(&watchlist_button);

            // Add elements to info container
            info_container.addArrangedSubview(&title_label);
            info_container.addArrangedSubview(&metadata_label);
            info_container.addArrangedSubview(&button_container);
            info_container.addArrangedSubview(&synopsis_view);

            // Add poster and info to content container
            content_container.addArrangedSubview(&poster_image);
            content_container.addArrangedSubview(&info_container);

            // Add backdrop and content to main container
            container.addArrangedSubview(&backdrop_image);
            container.addArrangedSubview(&content_container);

            // Set scroll view document view
            scroll_view.setDocumentView(Some(&container));
        }

        // Setup constraints
        // TODO: Simplified constraints - removed builder pattern due to AutoLayout API structure
        let backdrop_constraints = vec![AutoLayout::height(&backdrop_image, BACKDROP_HEIGHT)];
        AutoLayout::activate(&backdrop_constraints);

        let poster_constraints = vec![
            AutoLayout::width(&poster_image, POSTER_WIDTH),
            AutoLayout::height(&poster_image, POSTER_HEIGHT),
        ];
        AutoLayout::activate(&poster_constraints);

        let synopsis_constraints = vec![AutoLayout::height(&synopsis_view, 100.0)]; // min_height -> height 
        AutoLayout::activate(&synopsis_constraints);

        let mut details_view = Self {
            scroll_view,
            container,
            backdrop_image,
            poster_image,
            title_label,
            metadata_label,
            synopsis_view,
            play_button,
            watchlist_button,
            view_model,
        };

        details_view.setup_bindings()?;
        details_view.setup_actions()?;

        Ok(details_view)
    }

    pub fn view(&self) -> &NSScrollView {
        &self.scroll_view
    }

    fn setup_bindings(&mut self) -> CocoaResult<()> {
        debug!("Setting up DetailsView bindings");

        // Subscribe to current item changes
        let mut subscriber = self
            .view_model
            .subscribe_to_property("current_item")
            .ok_or_else(|| {
                CocoaError::InvalidState("Could not subscribe to current_item".to_string())
            })?;

        let title_label = self.title_label.clone();
        let metadata_label = self.metadata_label.clone();
        let synopsis_view = self.synopsis_view.clone();
        let poster_image = self.poster_image.clone();
        let backdrop_image = self.backdrop_image.clone();

        tokio::spawn(async move {
            while subscriber.wait_for_change().await {
                // Get the current item from the view model
                // Note: We need to access the view model through the subscriber's context
                // For now, we'll need to refactor this to get the actual data
                Queue::main().exec_async(move || {
                    debug!("Details view item updated");
                    // Update UI elements here
                });
            }
        });

        // Subscribe to is_watched changes
        if let Some(mut watched_subscriber) = self.view_model.subscribe_to_property("is_watched") {
            let watchlist_button = self.watchlist_button.clone();

            tokio::spawn(async move {
                while watched_subscriber.wait_for_change().await {
                    Queue::main().exec_async(move || {
                        // Update watchlist button text based on watched state
                        debug!("Watched state updated");
                    });
                }
            });
        }

        Ok(())
    }

    fn setup_actions(&mut self) -> CocoaResult<()> {
        debug!("Setting up button actions");

        // Setup play button action
        let view_model = self.view_model.clone();
        let play_action = block2::StackBlock::new(move || {
            info!("Play button clicked");
            // TODO: UI navigation - implement play_item navigation from UI layer
            // if let Some(item) = view_model.current_item().try_get().flatten() {
            //     if let Some(url) = &item.playback_url {
            //         // Navigate to player with URL
            //     }
            // }
        });

        unsafe {
            let _: () = msg_send![&self.play_button, setTarget: &*play_action];
            let _: () = msg_send![&self.play_button, setAction: sel!(invoke)];
        }

        // Setup watchlist button action
        let _view_model = self.view_model.clone();
        let watchlist_action = block2::StackBlock::new(move || {
            info!("Watchlist button clicked");
            // TODO: UI action - implement watchlist toggle from UI layer
            // This should trigger a UI action, not a ViewModel method
        });

        unsafe {
            let _: () = msg_send![&self.watchlist_button, setTarget: &*watchlist_action];
            let _: () = msg_send![&self.watchlist_button, setAction: sel!(invoke)];
        }

        Ok(())
    }

    fn format_metadata(item: &MediaItem) -> String {
        let mut parts = Vec::new();

        if let Some(year) = item.year() {
            parts.push(year.to_string());
        }

        if let Some(rating) = item.content_rating() {
            parts.push(rating.to_string());
        }

        if let Some(duration) = item.duration_millis() {
            let minutes = duration / 60000;
            let hours = minutes / 60;
            let mins = minutes % 60;
            if hours > 0 {
                parts.push(format!("{}h {}m", hours, mins));
            } else {
                parts.push(format!("{}m", mins));
            }
        }

        if let Some(rating) = item.rating() {
            parts.push(format!("⭐ {:.1}", rating));
        }

        parts.join(" • ")
    }

    fn load_image_async(_image_view: Retained<NSImageView>, url: String) {
        // TODO: Implement proper async image loading
        // The image_view cannot be moved into async context directly
        // Need to use a different approach, possibly with callbacks
        tokio::spawn(async move {
            match reqwest::get(&url).await {
                Ok(response) => {
                    if let Ok(bytes) = response.bytes().await {
                        let _data = bytes.to_vec();
                        // TODO: Set image on main thread
                        debug!("Image data loaded from {}", url);
                    }
                }
                Err(e) => {
                    error!("Failed to load image from {}: {}", url, e);
                }
            }
        });
    }

    pub fn load_item(&self, _item_id: String) {
        // TODO: UI navigation - load item should be handled at UI layer
        // The ViewModel should receive the loaded item, not fetch it
    }
}
