use gtk4::CompositeTemplate;
use gtk4::{self, glib, subclass::prelude::*};

mod imp {
    use super::*;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/dev/arsfeld/Reel/player.ui")]
    pub struct ReelPlayerOverlayHost {
        #[template_child]
        pub player_overlay: TemplateChild<gtk4::Overlay>,
        #[template_child]
        pub video_container: TemplateChild<gtk4::Box>,
        #[template_child]
        pub controls_container: TemplateChild<gtk4::Box>,
        #[template_child]
        pub top_left_osd: TemplateChild<gtk4::Box>,
        #[template_child]
        pub top_right_osd: TemplateChild<gtk4::Box>,
        #[template_child]
        pub back_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub close_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub skip_intro_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub skip_credits_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub auto_play_overlay: TemplateChild<gtk4::Box>,
        #[template_child]
        pub pip_container: TemplateChild<gtk4::Box>,
        #[template_child]
        pub play_now_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub cancel_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub loading_overlay: TemplateChild<gtk4::Box>,
        #[template_child]
        pub loading_spinner: TemplateChild<gtk4::Spinner>,
        #[template_child]
        pub loading_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub error_overlay: TemplateChild<gtk4::Box>,
        #[template_child]
        pub error_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub retry_button: TemplateChild<gtk4::Button>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ReelPlayerOverlayHost {
        const NAME: &'static str = "ReelPlayerOverlayHost";
        type Type = super::ReelPlayerOverlayHost;
        type ParentType = gtk4::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ReelPlayerOverlayHost {}
    impl WidgetImpl for ReelPlayerOverlayHost {}
    impl BoxImpl for ReelPlayerOverlayHost {}
}

glib::wrapper! {
    pub struct ReelPlayerOverlayHost(ObjectSubclass<imp::ReelPlayerOverlayHost>)
        @extends gtk4::Widget, gtk4::Box,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Orientable;
}

impl Default for ReelPlayerOverlayHost {
    fn default() -> Self {
        Self::new()
    }
}

impl ReelPlayerOverlayHost {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn overlay(&self) -> &gtk4::Overlay {
        &self.imp().player_overlay
    }
    pub fn video_container(&self) -> &gtk4::Box {
        &self.imp().video_container
    }
    pub fn controls_container(&self) -> &gtk4::Box {
        &self.imp().controls_container
    }
    pub fn top_left_osd(&self) -> &gtk4::Box {
        &self.imp().top_left_osd
    }
    pub fn top_right_osd(&self) -> &gtk4::Box {
        &self.imp().top_right_osd
    }
    pub fn back_button(&self) -> &gtk4::Button {
        &self.imp().back_button
    }
    pub fn close_button(&self) -> &gtk4::Button {
        &self.imp().close_button
    }
    pub fn skip_intro_button(&self) -> &gtk4::Button {
        &self.imp().skip_intro_button
    }
    pub fn skip_credits_button(&self) -> &gtk4::Button {
        &self.imp().skip_credits_button
    }
    pub fn auto_play_overlay(&self) -> &gtk4::Box {
        &self.imp().auto_play_overlay
    }
    pub fn pip_container(&self) -> &gtk4::Box {
        &self.imp().pip_container
    }
    pub fn play_now_button(&self) -> &gtk4::Button {
        &self.imp().play_now_button
    }
    pub fn cancel_button(&self) -> &gtk4::Button {
        &self.imp().cancel_button
    }
    pub fn loading_overlay(&self) -> &gtk4::Box {
        &self.imp().loading_overlay
    }
    pub fn loading_spinner(&self) -> &gtk4::Spinner {
        &self.imp().loading_spinner
    }
    pub fn loading_label(&self) -> &gtk4::Label {
        &self.imp().loading_label
    }
    pub fn error_overlay(&self) -> &gtk4::Box {
        &self.imp().error_overlay
    }
    pub fn error_label(&self) -> &gtk4::Label {
        &self.imp().error_label
    }
    pub fn retry_button(&self) -> &gtk4::Button {
        &self.imp().retry_button
    }
}
