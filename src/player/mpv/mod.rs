pub mod gl_render;

use libmpv2::Mpv;
use tracing::{error, info};

#[allow(dead_code)]
pub struct MpvBackend {
    pub mpv: Mpv,
}

impl MpvBackend {
    pub fn new() -> Result<Self, libmpv2::Error> {
        // MPV requires LC_NUMERIC=C
        unsafe {
            let c_locale = std::ffi::CString::new("C").unwrap();
            libc::setlocale(libc::LC_NUMERIC, c_locale.as_ptr());
        }

        let mpv = Mpv::new()?;

        mpv.set_property("vo", "libmpv")?;
        mpv.set_property("hwdec", "auto-safe")?;
        mpv.set_property("keep-open", "yes")?;
        mpv.set_property("idle", "yes")?;
        mpv.set_property("input-default-bindings", false)?;
        mpv.set_property("input-vo-keyboard", false)?;
        mpv.set_property("osc", false)?;
        mpv.set_property("ytdl", false)?;
        mpv.set_property("video-sync", "audio")?;

        if let Ok(version) = mpv.get_property::<String>("mpv-version") {
            info!("mpv version: {}", version);
        }

        Ok(Self { mpv })
    }

    #[allow(dead_code)]
    pub fn load_file(&self, uri: &str) {
        if let Err(e) = self.mpv.command("loadfile", &[uri, "replace"]) {
            error!("Failed to load file: {:?}", e);
        }
    }

    #[allow(dead_code)]
    pub fn toggle_pause(&self) {
        if let Err(e) = self.mpv.command("cycle", &["pause"]) {
            error!("Failed to toggle pause: {:?}", e);
        }
    }

    #[allow(dead_code)]
    pub fn position(&self) -> Option<f64> {
        self.mpv.get_property("playback-time").ok()
    }

    #[allow(dead_code)]
    pub fn duration(&self) -> Option<f64> {
        self.mpv.get_property("duration").ok()
    }

    #[allow(dead_code)]
    pub fn is_paused(&self) -> bool {
        self.mpv.get_property("pause").unwrap_or(true)
    }
}
