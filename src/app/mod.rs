pub mod app;

use anyhow::Result;
use std::sync::Arc;
use tokio::runtime::Runtime;

pub struct AppPlatform;

impl AppPlatform {
    pub fn run_relm4(runtime: Arc<Runtime>) -> Result<()> {
        let app = app::ReelApp::new(runtime);
        app.run()?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn detect_and_run(runtime: Arc<Runtime>) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            Self::run_relm4(runtime)
        }

        #[cfg(target_os = "macos")]
        {
            Self::run_relm4(runtime)
        }

        #[cfg(target_os = "windows")]
        {
            Self::run_relm4(runtime)
        }
    }
}
