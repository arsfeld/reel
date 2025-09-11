use gtk4::{glib, prelude::*, subclass::prelude::*};
use libadwaita::{self as adw, prelude::*};
use std::cell::RefCell;
use std::sync::Arc;

use super::super::main_window::ReelMainWindow;
use crate::core::viewmodels::property::Property;
use crate::core::viewmodels::sidebar_view_model::{LibraryInfo, SidebarViewModel, SourceInfo};
use crate::events::{DatabaseEvent, EventPayload, EventType};
use crate::platforms::gtk::ui::reactive::bindings::{
    BindingHandle, bind_image_icon_to_property, bind_spinner_to_property, bind_text_to_property,
    bind_visibility_to_property,
};

mod imp {
    use super::*;
    use gtk4::CompositeTemplate;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/dev/arsfeld/Reel/sidebar.ui")]
    pub struct Sidebar {
        #[template_child]
        pub welcome_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub connect_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub home_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub home_list: TemplateChild<gtk4::ListBox>,
        #[template_child]
        pub sources_container: TemplateChild<gtk4::Box>,
        #[template_child]
        pub status_container: TemplateChild<gtk4::Box>,
        #[template_child]
        pub status_icon: TemplateChild<gtk4::Image>,
        #[template_child]
        pub status_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub sync_spinner: TemplateChild<gtk4::Spinner>,
        #[template_child]
        pub sources_button: TemplateChild<gtk4::Button>,

        // Reactive properties
        pub sidebar_viewmodel: RefCell<Option<Arc<SidebarViewModel>>>,

        // Main window reference for sidebar navigation
        pub main_window: RefCell<Option<glib::WeakRef<ReelMainWindow>>>,

        // Binding handles to manage reactive subscriptions
        pub binding_handles: RefCell<Vec<BindingHandle>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Sidebar {
        const NAME: &'static str = "ReelSidebar";
        type Type = super::Sidebar;
        type ParentType = gtk4::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Sidebar {
        fn constructed(&self) {
            self.parent_constructed();

            // Set orientation to vertical by default
            let obj = self.obj();
            obj.set_orientation(gtk4::Orientation::Vertical);
        }
    }

    impl WidgetImpl for Sidebar {}
    impl BoxImpl for Sidebar {}
}

glib::wrapper! {
    pub struct Sidebar(ObjectSubclass<imp::Sidebar>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Orientable;
}

impl Sidebar {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn set_viewmodel(&self, viewmodel: Arc<SidebarViewModel>) {
        let imp = self.imp();
        imp.sidebar_viewmodel.replace(Some(viewmodel.clone()));

        // Setup reactive bindings
        self.setup_reactive_bindings(viewmodel);
    }

    pub fn set_main_window(&self, main_window: &ReelMainWindow) {
        let imp = self.imp();
        imp.main_window.replace(Some(main_window.downgrade()));
        // Navigation will be enabled when rows are created in reactive bindings
    }

    fn setup_reactive_bindings(&self, viewmodel: Arc<SidebarViewModel>) {
        let imp = self.imp();
        let mut handles = Vec::new();

        // Bind visibility properties
        handles.push(bind_visibility_to_property(
            &*imp.welcome_page,
            viewmodel.is_connected().clone(),
            |connected| !connected,
        ));

        handles.push(bind_visibility_to_property(
            &*imp.home_group,
            viewmodel.is_connected().clone(),
            |connected| *connected,
        ));

        handles.push(bind_visibility_to_property(
            &*imp.sources_container,
            viewmodel.sources().clone(),
            |sources| !sources.is_empty(),
        ));

        handles.push(bind_visibility_to_property(
            &*imp.status_container,
            viewmodel.is_connected().clone(),
            |connected| *connected,
        ));

        // Bind text properties
        handles.push(bind_text_to_property(
            &imp.status_label,
            viewmodel.status_text().clone(),
            |text| text.clone(),
        ));

        // Bind icon property
        handles.push(bind_image_icon_to_property(
            &imp.status_icon,
            viewmodel.status_icon().clone(),
            |icon| icon.clone(),
        ));

        // Bind spinner visibility and spinning
        handles.push(bind_visibility_to_property(
            &*imp.sync_spinner,
            viewmodel.show_spinner().clone(),
            |show| *show,
        ));

        // Custom binding for spinner spinning state
        handles.push(bind_spinner_to_property(
            &imp.sync_spinner,
            viewmodel.show_spinner().clone(),
            |show| *show,
        ));

        // Bind sources container to sources collection with navigation setup
        let sidebar_weak_for_sources = self.downgrade();
        handles.push(Self::setup_sources_binding(
            &imp.sources_container,
            viewmodel.sources().clone(),
            sidebar_weak_for_sources,
        ));

        // Bind home list to sources (for the unified home row)
        let sidebar_weak_2 = self.downgrade();
        handles.push(Self::setup_home_list_binding(
            &imp.home_list,
            viewmodel.sources().clone(),
            sidebar_weak_2,
        ));

        // Store binding handles
        imp.binding_handles.replace(handles);
    }

    /// Create a source group widget for a given SourceInfo
    fn create_source_group_widget(source: &SourceInfo) -> adw::PreferencesGroup {
        // Create a preferences group for this source
        let source_group = adw::PreferencesGroup::builder().title(&source.name).build();

        // Add edit/refresh buttons in the header suffix
        let header_buttons = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .build();

        let edit_button = gtk4::Button::builder()
            .icon_name("document-edit-symbolic")
            .valign(gtk4::Align::Center)
            .tooltip_text("Edit Libraries")
            .css_classes(vec!["flat"])
            .build();

        let refresh_button = gtk4::Button::builder()
            .icon_name("view-refresh-symbolic")
            .valign(gtk4::Align::Center)
            .tooltip_text("Refresh")
            .css_classes(vec!["flat"])
            .build();

        header_buttons.append(&edit_button);
        header_buttons.append(&refresh_button);
        source_group.set_header_suffix(Some(&header_buttons));

        // Connect refresh button for this specific source
        let source_id_clone = source.id.clone();
        refresh_button.connect_clicked(move |_| {
            // TODO: Implement refresh functionality
            eprintln!("Refresh requested for source: {}", source_id_clone);
        });

        // Create list box for libraries
        let libraries_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(vec!["boxed-list"])
            .build();

        // Add libraries for this source
        for library in &source.libraries {
            let row = Self::create_library_row(library, &source.id);
            libraries_list.append(&row);
        }

        // Navigation callbacks will be set up after widget creation

        source_group.add(&libraries_list);
        source_group
    }

    /// Create a library row widget for a given LibraryInfo
    fn create_library_row(library: &LibraryInfo, source_id: &str) -> adw::ActionRow {
        let row = adw::ActionRow::builder()
            .title(&library.title)
            .subtitle(format!("{} items", library.item_count))
            .activatable(true)
            .build();

        // Add icon based on library type
        let icon_name = match library.library_type.as_str() {
            "movies" => "video-x-generic-symbolic",
            "shows" => "video-display-symbolic",
            "music" => "audio-x-generic-symbolic",
            "photos" => "image-x-generic-symbolic",
            _ => "folder-symbolic",
        };

        let prefix_icon = gtk4::Image::from_icon_name(icon_name);
        row.add_prefix(&prefix_icon);

        let arrow = gtk4::Image::from_icon_name("go-next-symbolic");
        row.add_suffix(&arrow);

        // Store source_id:library_id in widget name for navigation
        row.set_widget_name(&format!("{}:{}", source_id, library.id));

        row
    }

    /// Setup reactive binding for home list (unified home row)
    fn setup_home_list_binding(
        home_list: &gtk4::ListBox,
        sources_property: Property<Vec<SourceInfo>>,
        sidebar_weak: glib::WeakRef<Self>,
    ) -> BindingHandle {
        let home_list_weak = home_list.downgrade();
        let mut subscriber = sources_property.subscribe();

        // Set initial home row
        if let Some(home_list) = home_list_weak.upgrade() {
            let sources = sources_property.get_sync();
            Self::update_home_list(&home_list, &sources, &sidebar_weak);
        }

        let handle = glib::spawn_future_local(async move {
            while subscriber.wait_for_change().await {
                if let Some(home_list) = home_list_weak.upgrade() {
                    let sources = sources_property.get().await;
                    Self::update_home_list(&home_list, &sources, &sidebar_weak);
                } else {
                    break; // Widget destroyed, exit loop
                }
            }
        });

        BindingHandle::new(handle)
    }

    /// Setup reactive binding for sources container with navigation
    fn setup_sources_binding(
        container: &gtk4::Box,
        sources_property: Property<Vec<SourceInfo>>,
        sidebar_weak: glib::WeakRef<Self>,
    ) -> BindingHandle {
        let container_weak = container.downgrade();
        let mut subscriber = sources_property.subscribe();

        // Set initial sources
        if let Some(container) = container_weak.upgrade() {
            let sources = sources_property.get_sync();
            Self::update_sources_container(&container, &sources, &sidebar_weak);
        }

        let handle = glib::spawn_future_local(async move {
            while subscriber.wait_for_change().await {
                if let Some(container) = container_weak.upgrade() {
                    let sources = sources_property.get().await;
                    Self::update_sources_container(&container, &sources, &sidebar_weak);
                } else {
                    break; // Widget destroyed, exit loop
                }
            }
        });

        BindingHandle::new(handle)
    }

    /// Update sources container with source groups and navigation callbacks
    fn update_sources_container(
        container: &gtk4::Box,
        sources: &[SourceInfo],
        sidebar_weak: &glib::WeakRef<Self>,
    ) {
        // Clear existing children
        while let Some(child) = container.first_child() {
            container.remove(&child);
        }

        // Add new source groups
        for source in sources {
            let source_group = Self::create_source_group_widget(source);
            container.append(&source_group);

            // Setup navigation for this source group
            Self::setup_source_group_navigation(&source_group, sidebar_weak);
        }
    }

    /// Setup navigation callbacks for a source group
    fn setup_source_group_navigation(
        source_group: &adw::PreferencesGroup,
        sidebar_weak: &glib::WeakRef<Self>,
    ) {
        // Find all ListBox widgets in the source group
        let mut current_widget = source_group.first_child();
        while let Some(widget) = current_widget {
            if let Some(list_box) = widget.downcast_ref::<gtk4::ListBox>() {
                let sidebar_weak_clone = sidebar_weak.clone();
                list_box.connect_row_activated(move |_, row| {
                    if let Some(action_row) = row.downcast_ref::<adw::ActionRow>()
                        && let Some(_sidebar) = sidebar_weak_clone.upgrade()
                    {
                        let widget_name = action_row.widget_name();
                        let parts: Vec<&str> = widget_name.split(':').collect();

                        if parts.len() == 2 {
                            let source_id = parts[0].to_string();
                            let library_id = parts[1].to_string();
                            let library_title = action_row.title().to_string();

                            // Find the actual library_type from SidebarViewModel data
                            let library_type = if let Some(viewmodel) = _sidebar.get_viewmodel() {
                                let sources = viewmodel.sources().get_sync();
                                sources.iter()
                                    .find(|source| source.id == source_id)
                                    .and_then(|source| source.libraries.iter().find(|lib| lib.id == library_id))
                                    .map(|lib| lib.library_type.clone())
                                    .unwrap_or_else(|| {
                                        eprintln!("Warning: Could not find library type for {}:{}, using 'movies' as fallback", source_id, library_id);
                                        "movies".to_string()
                                    })
                            } else {
                                eprintln!("Warning: SidebarViewModel not available, using 'movies' as fallback");
                                "movies".to_string()
                            };

                            // Emit library navigation event instead of direct MainWindow call
                            let sidebar_clone = _sidebar.clone();
                            glib::spawn_future_local(async move {
                                sidebar_clone.emit_library_navigation_event(
                                    source_id,
                                    library_id,
                                    library_title,
                                    library_type,
                                ).await;
                            });
                        }
                    }
                });
                break; // Only need to setup navigation once per source group
            }
            current_widget = widget.next_sibling();
        }
    }

    /// Update home list with unified home row
    fn update_home_list(
        home_list: &gtk4::ListBox,
        sources: &[SourceInfo],
        sidebar_weak: &glib::WeakRef<Self>,
    ) {
        // Clear existing rows
        while let Some(child) = home_list.first_child() {
            home_list.remove(&child);
        }

        // Only add home row if we have sources
        if !sources.is_empty() {
            let home_row = adw::ActionRow::builder()
                .title("Home")
                .subtitle("Recently added from all sources")
                .activatable(true)
                .build();

            let home_icon = gtk4::Image::from_icon_name("user-home-symbolic");
            home_row.add_prefix(&home_icon);

            let home_arrow = gtk4::Image::from_icon_name("go-next-symbolic");
            home_row.add_suffix(&home_arrow);

            home_row.set_widget_name("__home__");

            // Setup navigation for home row
            let sidebar_weak_clone = sidebar_weak.clone();
            home_row.connect_activated(move |_| {
                if let Some(sidebar) = sidebar_weak_clone.upgrade() {
                    // Emit home navigation event instead of direct MainWindow call
                    let sidebar_clone = sidebar.clone();
                    glib::spawn_future_local(async move {
                        sidebar_clone.emit_home_navigation_event(None).await;
                    });
                }
            });

            home_list.append(&home_row);
        }
    }

    pub fn get_viewmodel(&self) -> Option<Arc<SidebarViewModel>> {
        self.imp().sidebar_viewmodel.borrow().as_ref().cloned()
    }

    /// Emit library navigation event through SidebarViewModel's EventBus
    async fn emit_library_navigation_event(
        &self,
        source_id: String,
        library_id: String,
        library_title: String,
        library_type: String,
    ) {
        if let Some(viewmodel) = self.get_viewmodel() {
            let event = DatabaseEvent::new(
                EventType::LibraryNavigationRequested,
                EventPayload::LibraryNavigation {
                    source_id,
                    library_id,
                    library_title,
                    library_type,
                },
            );
            if let Err(e) = viewmodel.emit_event(event).await {
                eprintln!("Failed to emit library navigation event: {}", e);
            }
        }
    }

    /// Emit home navigation event through SidebarViewModel's EventBus
    async fn emit_home_navigation_event(&self, source_id: Option<String>) {
        if let Some(viewmodel) = self.get_viewmodel() {
            let event = DatabaseEvent::new(
                EventType::HomeNavigationRequested,
                EventPayload::HomeNavigation { source_id },
            );
            if let Err(e) = viewmodel.emit_event(event).await {
                eprintln!("Failed to emit home navigation event: {}", e);
            }
        }
    }

    // Accessors for template children (useful for reactive bindings)
    pub fn welcome_page(&self) -> &adw::StatusPage {
        &self.imp().welcome_page
    }

    pub fn connect_button(&self) -> &gtk4::Button {
        &self.imp().connect_button
    }

    pub fn home_group(&self) -> &adw::PreferencesGroup {
        &self.imp().home_group
    }

    pub fn home_list(&self) -> &gtk4::ListBox {
        &self.imp().home_list
    }

    pub fn sources_container(&self) -> &gtk4::Box {
        &self.imp().sources_container
    }

    pub fn status_container(&self) -> &gtk4::Box {
        &self.imp().status_container
    }

    pub fn status_icon(&self) -> &gtk4::Image {
        &self.imp().status_icon
    }

    pub fn status_label(&self) -> &gtk4::Label {
        &self.imp().status_label
    }

    pub fn sync_spinner(&self) -> &gtk4::Spinner {
        &self.imp().sync_spinner
    }

    pub fn sources_button(&self) -> &gtk4::Button {
        &self.imp().sources_button
    }
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new()
    }
}
