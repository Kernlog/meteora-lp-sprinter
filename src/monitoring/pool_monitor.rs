use anyhow::Result;
use crate::models::Pool;
use tokio::sync::mpsc;

/// Trait for pool monitors
pub trait PoolMonitor {
    /// Start monitoring for new pools
    async fn start_monitoring(&mut self, tx: mpsc::Sender<Pool>) -> Result<()>;
    
    /// Stop monitoring
    async fn stop(&mut self) -> Result<()>;
} 