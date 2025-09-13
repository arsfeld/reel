pub mod app;
pub mod components;

use crate::core::frontend::Frontend;
use anyhow::Result;
use std::sync::Arc;
use tokio::runtime::Runtime;

pub struct Relm4Platform;

impl Relm4Platform {
    pub fn new() -> Self {
        Self
    }
}

impl Frontend for Relm4Platform {
    fn run(&self, runtime: Arc<Runtime>) -> Result<()> {
        let app = app::ReelApp::new(runtime);
        app.run()?;
        Ok(())
    }
}
