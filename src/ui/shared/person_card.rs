use crate::models::Person;
use gtk::prelude::*;
use relm4::gtk;

pub fn create_person_card(person: &Person, texture: Option<&gtk::gdk::Texture>) -> gtk::Box {
    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .width_request(120)
        .css_classes(["cast-card-modern", "interactive-element"])
        .build();

    // Person image or placeholder
    let picture = gtk::Picture::builder()
        .width_request(120)
        .height_request(120)
        .content_fit(gtk::ContentFit::Cover)
        .build();

    // Use the provided texture if available
    if let Some(tex) = texture {
        picture.set_paintable(Some(tex));
    }

    // Info container with gradient background
    let info_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .css_classes(["cast-info"])
        .margin_start(8)
        .margin_end(8)
        .margin_top(8)
        .margin_bottom(8)
        .build();

    let name = gtk::Label::builder()
        .label(&person.name)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["caption-heading"])
        .xalign(0.0)
        .build();

    let role = gtk::Label::builder()
        .label(person.role.as_deref().unwrap_or(""))
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["dim-label", "caption"])
        .xalign(0.0)
        .build();

    info_box.append(&name);
    if person.role.is_some() {
        info_box.append(&role);
    }

    card.append(&picture);
    card.append(&info_box);

    card
}
