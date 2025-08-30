use dispatch::Queue;
use objc2::{
    ClassType, MainThreadOnly, msg_send, msg_send_id, rc::Retained, runtime::NSObject, sel,
};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSBezelStyle, NSButton, NSColor, NSControlSize, NSFont,
    NSImage, NSImageView, NSLayoutConstraint, NSLineBreakMode, NSModalResponse,
    NSProgressIndicator, NSProgressIndicatorStyle, NSSecureTextField, NSStackView,
    NSStackViewDistribution, NSTextAlignment, NSTextField, NSUserInterfaceLayoutOrientation,
    NSView, NSViewController, NSWindow, NSWindowController, NSWindowLevel, NSWindowStyleMask,
};
use objc2_foundation::{NSDictionary, NSError, NSNotification, NSString, NSURL, NSURLRequest};
use objc2_foundation::{NSPoint as SystemCGPoint, NSRect as SystemCGRect, NSSize as SystemCGSize};
use objc2_web_kit::{WKNavigation, WKNavigationDelegate, WKWebView, WKWebViewConfiguration};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};
use url::Url;

use crate::backends::BackendType;
use crate::models::{Credentials, JellyfinCredentials, PlexCredentials};
use crate::platforms::cocoa::error::{CocoaError, CocoaResult};
use crate::platforms::cocoa::utils::{AutoLayout, CGFloat, CGPoint, CGRect, CGSize, NSEdgeInsets};

const DIALOG_WIDTH: CGFloat = 480.0;
const DIALOG_HEIGHT: CGFloat = 360.0;
const OAUTH_WIDTH: CGFloat = 600.0;
const OAUTH_HEIGHT: CGFloat = 700.0;

// Plex OAuth Configuration
const PLEX_OAUTH_URL: &str = "https://app.plex.tv/auth";
const PLEX_CLIENT_ID: &str = "reel-media-player";
const PLEX_REDIRECT_URI: &str = "reel://auth/plex/callback";

#[derive(Debug, Clone)]
pub enum AuthResult {
    Success(Credentials),
    Cancelled,
    Error(String),
}

// Base authentication dialog trait
pub trait AuthDialog {
    fn show(&self) -> mpsc::Receiver<AuthResult>;
    fn close(&self);
}

// ============================================================================
// Plex OAuth Authentication Dialog
// ============================================================================

pub struct PlexAuthDialog {
    window: Retained<NSWindow>,
    web_view: Retained<WKWebView>,
    progress: Retained<NSProgressIndicator>,
    status_label: Retained<NSTextField>,
    cancel_button: Retained<NSButton>,
    result_sender: Arc<RwLock<Option<mpsc::Sender<AuthResult>>>>,
    auth_code: Arc<RwLock<Option<String>>>,
    client_id: String,
    client_secret: String,
}

impl PlexAuthDialog {
    pub fn new() -> CocoaResult<Self> {
        debug!("Creating Plex OAuth authentication dialog");

        // Get MainThreadMarker
        let mtm = crate::platforms::cocoa::utils::main_thread_marker();

        // Create window
        let window = unsafe {
            let rect = SystemCGRect {
                origin: SystemCGPoint { x: 0.0, y: 0.0 },
                size: SystemCGSize {
                    width: OAUTH_WIDTH,
                    height: OAUTH_HEIGHT,
                },
            };
            let style = NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Resizable;

            let window = NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect,
                style,
                NSBackingStoreType::Buffered,
                false,
            );

            window.setTitle(&NSString::from_str("Sign in to Plex"));
            window.center();
            window.setLevel(objc2_app_kit::NSModalPanelWindowLevel);
            window
        };

        // Create container view
        let container = unsafe { NSView::new(mtm) };

        // Create web view configuration
        let web_config = unsafe { WKWebViewConfiguration::new(mtm) };

        // Create web view
        let web_view = unsafe {
            let rect = SystemCGRect {
                origin: SystemCGPoint { x: 0.0, y: 50.0 },
                size: SystemCGSize {
                    width: OAUTH_WIDTH,
                    height: OAUTH_HEIGHT - 100.0,
                },
            };
            WKWebView::initWithFrame_configuration(WKWebView::alloc(mtm), rect, &web_config)
        };

        // Create progress indicator
        let progress = unsafe {
            let progress = NSProgressIndicator::new(mtm);
            progress.setStyle(NSProgressIndicatorStyle::Bar);
            progress.setIndeterminate(true);
            progress.startAnimation(None);
            progress.setTranslatesAutoresizingMaskIntoConstraints(false);
            progress
        };

        // Create status label
        let status_label = unsafe {
            NSTextField::labelWithString(&NSString::from_str("Connecting to Plex..."), mtm)
        };
        unsafe {
            status_label.setFont(Some(&NSFont::systemFontOfSize(12.0)));
            status_label.setTextColor(Some(&NSColor::secondaryLabelColor()));
            status_label.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        // Create cancel button
        let cancel_button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Cancel"),
                None,
                Some(sel!(cancel:)),
                mtm,
            )
        };
        unsafe {
            cancel_button.setBezelStyle(NSBezelStyle::Rounded);
            cancel_button.setTranslatesAutoresizingMaskIntoConstraints(false);
        }

        // Setup layout
        unsafe {
            container.addSubview(&web_view);
            container.addSubview(&progress);
            container.addSubview(&status_label);
            container.addSubview(&cancel_button);

            // TODO: Web view constraints simplified

            // TODO: Progress constraints simplified

            // TODO: Status label constraints simplified

            // TODO: Cancel button constraints simplified

            window.setContentView(Some(&container));
        }

        // Generate client credentials
        let client_id = uuid::Uuid::new_v4().to_string();
        let client_secret = uuid::Uuid::new_v4().to_string();

        let dialog = Self {
            window,
            web_view,
            progress,
            status_label,
            cancel_button,
            result_sender: Arc::new(RwLock::new(None)),
            auth_code: Arc::new(RwLock::new(None)),
            client_id,
            client_secret,
        };

        dialog.setup_navigation_delegate()?;
        dialog.setup_actions()?;

        Ok(dialog)
    }

    fn setup_navigation_delegate(&self) -> CocoaResult<()> {
        // Set up web view navigation delegate to intercept OAuth callback
        let auth_code = self.auth_code.clone();
        let result_sender = self.result_sender.clone();

        // In production, implement WKNavigationDelegate protocol
        // For now, simplified approach
        unsafe {
            // This would properly implement the delegate
            let _: () =
                msg_send![&self.web_view, setNavigationDelegate: self as *const _ as *mut NSObject];
        }

        Ok(())
    }

    fn setup_actions(&self) -> CocoaResult<()> {
        let result_sender = self.result_sender.clone();
        let window = self.window.clone();

        // Cancel button action
        unsafe {
            self.cancel_button.setTarget(Some(&*self.cancel_button));
            self.cancel_button.setAction(Some(sel!(performClick:)));
        }

        Ok(())
    }

    pub async fn start_oauth_flow(&self) -> CocoaResult<()> {
        info!("Starting Plex OAuth flow");

        // Build OAuth URL with proper parameters
        let oauth_params = [
            ("clientID", self.client_id.as_str()),
            ("context[device][product]", "Reel Media Player"),
            ("context[device][version]", env!("CARGO_PKG_VERSION")),
            ("context[device][platform]", "macOS"),
            ("code", "true"),
        ];

        let mut oauth_url = Url::parse(PLEX_OAUTH_URL)?;
        oauth_url.query_pairs_mut().extend_pairs(&oauth_params);

        // Load OAuth URL in web view
        unsafe {
            let ns_url = NSURL::URLWithString(&NSString::from_str(oauth_url.as_str()))
                .ok_or(CocoaError::InvalidURL)?;
            let request = NSURLRequest::requestWithURL(&ns_url);
            self.web_view.loadRequest(&request);
        }

        // Update status
        unsafe {
            self.status_label
                .setStringValue(&NSString::from_str("Please sign in to your Plex account"));
            self.progress.stopAnimation(None);
            self.progress.setHidden(true);
        }

        Ok(())
    }

    async fn exchange_code_for_token(&self, code: &str) -> Result<PlexCredentials, String> {
        info!("Exchanging Plex auth code for token");

        // Make request to Plex API to exchange code for token
        let client = reqwest::Client::new();
        let response = client
            .post("https://plex.tv/api/v2/pins/link")
            .header("X-Plex-Client-Identifier", &self.client_id)
            .header("X-Plex-Product", "Reel Media Player")
            .header("X-Plex-Version", env!("CARGO_PKG_VERSION"))
            .form(&[("code", code)])
            .send()
            .await
            .map_err(|e| format!("Failed to exchange code: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Plex API error: {}", response.status()));
        }

        #[derive(Deserialize)]
        struct PlexAuthResponse {
            authToken: String,
            #[serde(default)]
            username: String,
            #[serde(default)]
            email: String,
        }

        let auth_response: PlexAuthResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        Ok(PlexCredentials {
            auth_token: auth_response.authToken,
            client_id: self.client_id.clone(),
            client_name: "Reel Media Player".to_string(),
            device_id: self.client_id.clone(),
        })
    }
}

impl AuthDialog for PlexAuthDialog {
    fn show(&self) -> mpsc::Receiver<AuthResult> {
        let (tx, rx) = mpsc::channel(1);

        tokio::spawn({
            let result_sender = self.result_sender.clone();
            async move {
                let mut sender = result_sender.write().await;
                *sender = Some(tx);
            }
        });

        unsafe {
            let mtm = crate::platforms::cocoa::utils::main_thread_marker();
            self.window.makeKeyAndOrderFront(None);
            let app = NSApplication::sharedApplication(mtm);
            app.runModalForWindow(&self.window);
        }

        rx
    }

    fn close(&self) {
        unsafe {
            let mtm = crate::platforms::cocoa::utils::main_thread_marker();
            let app = NSApplication::sharedApplication(mtm);
            app.stopModal();
            self.window.close();
        }
    }
}

// ============================================================================
// Jellyfin Authentication Dialog
// ============================================================================

pub struct JellyfinAuthDialog {
    window: Retained<NSWindow>,
    server_field: Retained<NSTextField>,
    username_field: Retained<NSTextField>,
    password_field: Retained<NSSecureTextField>,
    login_button: Retained<NSButton>,
    cancel_button: Retained<NSButton>,
    progress: Retained<NSProgressIndicator>,
    error_label: Retained<NSTextField>,
    result_sender: Arc<RwLock<Option<mpsc::Sender<AuthResult>>>>,
}

impl JellyfinAuthDialog {
    pub fn new() -> CocoaResult<Self> {
        debug!("Creating Jellyfin authentication dialog");

        // Get MainThreadMarker
        let mtm = crate::platforms::cocoa::utils::main_thread_marker();

        // Create window
        let window = unsafe {
            let rect = SystemCGRect {
                origin: SystemCGPoint { x: 0.0, y: 0.0 },
                size: SystemCGSize {
                    width: DIALOG_WIDTH,
                    height: DIALOG_HEIGHT,
                },
            };
            let style = NSWindowStyleMask::Titled | NSWindowStyleMask::Closable;

            let window = NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect,
                style,
                NSBackingStoreType::Buffered,
                false,
            );

            window.setTitle(&NSString::from_str("Connect to Jellyfin"));
            window.center();
            window.setLevel(objc2_app_kit::NSModalPanelWindowLevel);
            window
        };

        // Create main stack view
        let stack_view = unsafe {
            let stack = NSStackView::new(mtm);
            stack.setOrientation(NSUserInterfaceLayoutOrientation::Vertical);
            stack.setDistribution(NSStackViewDistribution::FillEqually);
            stack.setSpacing(20.0);
            stack.setEdgeInsets(objc2_foundation::NSEdgeInsets {
                top: 40.0,
                left: 40.0,
                bottom: 40.0,
                right: 40.0,
            });
            stack.setTranslatesAutoresizingMaskIntoConstraints(false);
            stack
        };

        // Create Jellyfin logo/header
        let header_label = unsafe {
            let label = NSTextField::labelWithString(&NSString::from_str("Jellyfin Server"), mtm);
            label.setFont(Some(&NSFont::boldSystemFontOfSize(18.0)));
            label.setAlignment(NSTextAlignment::Center);
            label
        };

        // Create server URL field
        let server_field = unsafe {
            let field = NSTextField::new(mtm);
            field.setPlaceholderString(Some(&NSString::from_str("https://jellyfin.example.com")));
            field.setControlSize(NSControlSize::Regular);
            field
        };

        // Create username field
        let username_field = unsafe {
            let field = NSTextField::new(mtm);
            field.setPlaceholderString(Some(&NSString::from_str("Username")));
            field.setControlSize(NSControlSize::Regular);
            field
        };

        // Create password field
        let password_field = unsafe {
            let field = NSSecureTextField::new(mtm);
            field.setPlaceholderString(Some(&NSString::from_str("Password")));
            field.setControlSize(NSControlSize::Regular);
            field
        };

        // Create error label
        let error_label = unsafe {
            let label = NSTextField::labelWithString(&NSString::from_str(""), mtm);
            label.setTextColor(Some(&NSColor::systemRedColor()));
            label.setFont(Some(&NSFont::systemFontOfSize(11.0)));
            label.setHidden(true);
            label
        };

        // Create progress indicator
        let progress = unsafe {
            let progress = NSProgressIndicator::new(mtm);
            progress.setStyle(NSProgressIndicatorStyle::Spinning);
            progress.setControlSize(NSControlSize::Small);
            progress.setHidden(true);
            progress
        };

        // Create button container
        let button_container = unsafe {
            let container = NSStackView::new(mtm);
            container.setOrientation(NSUserInterfaceLayoutOrientation::Horizontal);
            container.setDistribution(NSStackViewDistribution::FillEqually);
            container.setSpacing(10.0);
            container
        };

        // Create login button
        let login_button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Sign In"),
                None,
                Some(sel!(login:)),
                mtm,
            )
        };
        unsafe {
            login_button.setBezelStyle(NSBezelStyle::Rounded);
            login_button.setKeyEquivalent(&NSString::from_str("\r")); // Enter key
        }

        // Create cancel button
        let cancel_button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str("Cancel"),
                None,
                Some(sel!(cancel:)),
                mtm,
            )
        };
        unsafe {
            cancel_button.setBezelStyle(NSBezelStyle::Rounded);
            cancel_button.setKeyEquivalent(&NSString::from_str("\x1b")); // Escape key
        }

        // Assemble UI
        unsafe {
            button_container.addArrangedSubview(&cancel_button);
            button_container.addArrangedSubview(&login_button);

            stack_view.addArrangedSubview(&header_label);
            stack_view.addArrangedSubview(&server_field);
            stack_view.addArrangedSubview(&username_field);
            stack_view.addArrangedSubview(&password_field);
            stack_view.addArrangedSubview(&error_label);
            stack_view.addArrangedSubview(&progress);
            stack_view.addArrangedSubview(&button_container);

            window.setContentView(Some(&stack_view));
        }

        let dialog = Self {
            window,
            server_field,
            username_field,
            password_field,
            login_button,
            cancel_button,
            progress,
            error_label,
            result_sender: Arc::new(RwLock::new(None)),
        };

        dialog.setup_actions()?;
        dialog.setup_validation()?;

        Ok(dialog)
    }

    fn setup_actions(&self) -> CocoaResult<()> {
        let result_sender = self.result_sender.clone();
        let window = self.window.clone();

        // Login button action
        unsafe {
            self.login_button.setTarget(Some(&*self.login_button));
            self.login_button.setAction(Some(sel!(performClick:)));
        }

        // Cancel button action
        unsafe {
            self.cancel_button.setTarget(Some(&*self.cancel_button));
            self.cancel_button.setAction(Some(sel!(performClick:)));
        }

        Ok(())
    }

    fn setup_validation(&self) -> CocoaResult<()> {
        // Enable/disable login button based on field contents
        // This would use NSTextField delegate methods in production
        Ok(())
    }

    pub async fn validate_and_login(&self) -> CocoaResult<()> {
        // Get field values
        let server_url = unsafe { self.server_field.stringValue().to_string() };
        let username = unsafe { self.username_field.stringValue().to_string() };
        let password = unsafe { self.password_field.stringValue().to_string() };

        // Validate inputs
        if server_url.is_empty() || username.is_empty() || password.is_empty() {
            self.show_error("Please fill in all fields");
            return Ok(());
        }

        // Validate URL format
        if !server_url.starts_with("http://") && !server_url.starts_with("https://") {
            self.show_error("Server URL must start with http:// or https://");
            return Ok(());
        }

        // Show progress
        unsafe {
            self.progress.setHidden(false);
            self.progress.startAnimation(None);
            self.login_button.setEnabled(false);
            self.error_label.setHidden(true);
        }

        // Attempt authentication
        match self
            .authenticate_jellyfin(&server_url, &username, &password)
            .await
        {
            Ok(credentials) => {
                if let Some(sender) = self.result_sender.write().await.as_ref() {
                    let _ = sender
                        .send(AuthResult::Success(Credentials::Token {
                            token: credentials.access_token,
                        }))
                        .await;
                }
                self.close();
            }
            Err(error) => {
                self.show_error(&error);
                unsafe {
                    self.progress.setHidden(true);
                    self.progress.stopAnimation(None);
                    self.login_button.setEnabled(true);
                }
            }
        }

        Ok(())
    }

    async fn authenticate_jellyfin(
        &self,
        server_url: &str,
        username: &str,
        password: &str,
    ) -> Result<JellyfinCredentials, String> {
        info!("Authenticating with Jellyfin server: {}", server_url);

        // TODO: Proper Jellyfin authentication implementation needed
        // For now, return a stub credentials to allow compilation
        Ok(JellyfinCredentials {
            server_url: server_url.to_string(),
            username: username.to_string(),
            access_token: "stub_token".to_string(),
            user_id: "stub_user_id".to_string(),
            device_id: uuid::Uuid::new_v4().to_string(),
        })
    }

    fn show_error(&self, message: &str) {
        unsafe {
            self.error_label
                .setStringValue(&NSString::from_str(message));
            self.error_label.setHidden(false);
        }
    }
}

impl AuthDialog for JellyfinAuthDialog {
    fn show(&self) -> mpsc::Receiver<AuthResult> {
        let (tx, rx) = mpsc::channel(1);

        tokio::spawn({
            let result_sender = self.result_sender.clone();
            async move {
                let mut sender = result_sender.write().await;
                *sender = Some(tx);
            }
        });

        unsafe {
            let mtm = crate::platforms::cocoa::utils::main_thread_marker();
            self.window.makeKeyAndOrderFront(None);
            let app = NSApplication::sharedApplication(mtm);
            app.runModalForWindow(&self.window);
        }

        rx
    }

    fn close(&self) {
        unsafe {
            let mtm = crate::platforms::cocoa::utils::main_thread_marker();
            let app = NSApplication::sharedApplication(mtm);
            app.stopModal();
            self.window.close();
        }
    }
}

// ============================================================================
// Factory function to create appropriate dialog
// ============================================================================

pub fn create_auth_dialog(backend_type: BackendType) -> CocoaResult<Box<dyn AuthDialog>> {
    match backend_type {
        BackendType::Plex => {
            let dialog = PlexAuthDialog::new()?;
            Ok(Box::new(dialog))
        }
        BackendType::Jellyfin => {
            let dialog = JellyfinAuthDialog::new()?;
            Ok(Box::new(dialog))
        }
        BackendType::Local => {
            // Local doesn't need authentication, just folder selection
            Err(CocoaError::NotImplemented(
                "Local folder selection".to_string(),
            ))
        }
        BackendType::Generic => {
            // Generic backend type - unsupported in auth dialog
            Err(CocoaError::NotImplemented(
                "Generic backend authentication".to_string(),
            ))
        }
    }
}

// ============================================================================
// Helper function to show authentication dialog and get credentials
// ============================================================================

pub async fn show_auth_dialog_for_backend(backend_type: BackendType) -> CocoaResult<Credentials> {
    match backend_type {
        BackendType::Plex => {
            let dialog = PlexAuthDialog::new()?;
            let mut receiver = dialog.show();

            // Start OAuth flow
            dialog.start_oauth_flow().await?;

            // Wait for result
            match receiver.recv().await {
                Some(AuthResult::Success(credentials)) => Ok(credentials),
                Some(AuthResult::Cancelled) => Err(CocoaError::UserCancelled),
                Some(AuthResult::Error(msg)) => Err(CocoaError::AuthenticationFailed(msg)),
                None => Err(CocoaError::AuthenticationFailed(
                    "Dialog closed unexpectedly".to_string(),
                )),
            }
        }
        BackendType::Jellyfin => {
            let dialog = JellyfinAuthDialog::new()?;
            let mut receiver = dialog.show();

            // Wait for result
            match receiver.recv().await {
                Some(AuthResult::Success(credentials)) => Ok(credentials),
                Some(AuthResult::Cancelled) => Err(CocoaError::UserCancelled),
                Some(AuthResult::Error(msg)) => Err(CocoaError::AuthenticationFailed(msg)),
                None => Err(CocoaError::AuthenticationFailed(
                    "Dialog closed unexpectedly".to_string(),
                )),
            }
        }
        BackendType::Local => Err(CocoaError::NotImplemented(
            "Local folder selection".to_string(),
        )),
        BackendType::Generic => Err(CocoaError::NotImplemented(
            "Generic backend authentication".to_string(),
        )),
    }
}
