use relm4::gtk;

/// Load an image from a URL and create a GDK texture
pub async fn load_image_from_url(
    url: &str,
    _width: i32,
    _height: i32,
) -> Result<gtk::gdk::Texture, String> {
    // Download the image
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("Failed to download image: {}", e))?;

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read bytes: {}", e))?;

    // Create texture from bytes
    let glib_bytes = gtk::glib::Bytes::from(&bytes[..]);
    let texture = gtk::gdk::Texture::from_bytes(&glib_bytes)
        .map_err(|e| format!("Failed to create texture: {}", e))?;

    // If width and height are specified (not -1), we could resize here
    // For now, just return the texture as is
    Ok(texture)
}
