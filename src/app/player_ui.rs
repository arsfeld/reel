use adw::prelude::*;

use crate::models::media::MediaItem;

pub fn player_title_for_item(item: Option<&MediaItem>, fallback: &str) -> String {
    if let Some(item) = item {
        return item.display_title();
    }
    let path = fallback.strip_prefix("file://").unwrap_or(fallback);
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Reel")
        .to_string()
}

pub fn enter_player_mode(
    root: &adw::ApplicationWindow,
    chrome: &mut gtk::Revealer,
    window_title: &adw::WindowTitle,
    title: &str,
) {
    window_title.set_title(title);
    root.set_title(Some(title));
    chrome.set_reveal_child(true);
}

pub fn leave_player_mode(root: &adw::ApplicationWindow, chrome: &mut gtk::Revealer) {
    chrome.set_reveal_child(false);
    root.set_title(Some("Reel"));
}
