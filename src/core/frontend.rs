use anyhow::Result;
use std::sync::Arc;
use tokio::runtime::Runtime;

pub trait Frontend {
    fn run(&self, runtime: Arc<Runtime>) -> Result<()>;
}
