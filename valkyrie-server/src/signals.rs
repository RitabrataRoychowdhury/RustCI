//! Signal handling for graceful shutdown

use anyhow::Result;
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook_tokio::Signals;
use futures::stream::StreamExt;
use tracing::{debug, info};

/// Wait for shutdown signals (SIGINT, SIGTERM)
pub async fn wait_for_shutdown() -> Result<()> {
    let mut signals = Signals::new(&[SIGINT, SIGTERM])?;
    
    info!("Signal handler installed, waiting for shutdown signal...");
    
    while let Some(signal) = signals.next().await {
        match signal {
            SIGINT => {
                info!("Received SIGINT (Ctrl+C)");
                break;
            }
            SIGTERM => {
                info!("Received SIGTERM");
                break;
            }
            _ => {
                debug!("Received unknown signal: {}", signal);
            }
        }
    }
    
    Ok(())
}