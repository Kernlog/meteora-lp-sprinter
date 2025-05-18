use anyhow::{Result, anyhow};
use log::{info, debug, error};
use solana_client::nonblocking::pubsub_client::PubsubClient;
use solana_client::rpc_config::{RpcTransactionLogsFilter, RpcTransactionLogsConfig};
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use futures::StreamExt; // Import StreamExt for the next() method
use chrono::Utc;
use regex::Regex;

use crate::models::Pool;
use crate::models::pool::TokenInfo;
use crate::monitoring::pool_monitor::PoolMonitor;

// Meteora DAMM v2 program ID
const METEORA_PROGRAM_ID: &str = "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG";

/// Monitors Solana directly via websocket for Meteora pool creation
pub struct MeteoraPoolMonitor {
    /// RPC endpoint for websocket connection
    rpc_url: String,
    /// Commitment level to use
    commitment: CommitmentConfig,
    /// Current subscription (if active)
    subscription: Arc<Mutex<Option<WebsocketSubscription>>>,
}

/// Wrapper for the websocket subscription
struct WebsocketSubscription {
    /// Channel to request cancellation
    cancel_sender: oneshot::Sender<()>,
    /// The background task handle
    task_handle: JoinHandle<()>,
}

impl MeteoraPoolMonitor {
    /// Create a new Meteora pool monitor
    pub fn new(rpc_url: String) -> Self {
        // Convert HTTP URL to WebSocket URL if needed
        let ws_url = if rpc_url.starts_with("http") {
            rpc_url.replace("http", "ws")
        } else {
            rpc_url
        };

        Self {
            rpc_url: ws_url,
            commitment: CommitmentConfig::confirmed(),
            subscription: Arc::new(Mutex::new(None)),
        }
    }

    /// Extract pool information from transaction logs
    fn extract_pool_info(&self, log_messages: &[String]) -> Option<Pool> {
        // Regex patterns to extract pool information
        let pool_created_pattern = Regex::new(r"Program log: Pool created: ([1-9A-HJ-NP-Za-km-z]{32,})").ok()?;
        let token_a_pattern = Regex::new(r"Token A: ([1-9A-HJ-NP-Za-km-z]{32,})").ok()?;
        let token_b_pattern = Regex::new(r"Token B: ([1-9A-HJ-NP-Za-km-z]{32,})").ok()?;

        let mut pool_address = None;
        let mut token_a_mint = None;
        let mut token_b_mint = None;

        // Parse log messages for relevant information
        for message in log_messages {
            if let Some(cap) = pool_created_pattern.captures(message) {
                if let Some(address_match) = cap.get(1) {
                    if let Ok(pubkey) = Pubkey::from_str(address_match.as_str()) {
                        pool_address = Some(pubkey);
                    }
                }
            } else if let Some(cap) = token_a_pattern.captures(message) {
                if let Some(mint_match) = cap.get(1) {
                    if let Ok(pubkey) = Pubkey::from_str(mint_match.as_str()) {
                        token_a_mint = Some(pubkey);
                    }
                }
            } else if let Some(cap) = token_b_pattern.captures(message) {
                if let Some(mint_match) = cap.get(1) {
                    if let Ok(pubkey) = Pubkey::from_str(mint_match.as_str()) {
                        token_b_mint = Some(pubkey);
                    }
                }
            }
        }

        // If we have all required information, create a Pool object
        if let (Some(address), Some(token_a), Some(token_b)) = (pool_address, token_a_mint, token_b_mint) {
            Some(Pool {
                address,
                token_a: TokenInfo {
                    mint: token_a,
                    name: None,
                    symbol: None,
                    decimals: None,
                },
                token_b: TokenInfo {
                    mint: token_b,
                    name: None,
                    symbol: None,
                    decimals: None,
                },
                discovered_at: Utc::now(),
                analyzed: false,
                score: None,
            })
        } else {
            None
        }
    }
}

impl PoolMonitor for MeteoraPoolMonitor {
    async fn start_monitoring(&mut self, tx: mpsc::Sender<Pool>) -> Result<()> {
        info!("Starting Meteora pool monitoring via Solana websocket...");

        // Ensure we don't have an active subscription
        if self.subscription.lock().unwrap().is_some() {
            return Err(anyhow!("Websocket monitoring already active"));
        }

        // Parse the program ID
        let program_id = Pubkey::from_str(METEORA_PROGRAM_ID)
            .map_err(|e| anyhow!("Invalid program ID: {}", e))?;

        // Set up cancellation channel
        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();

        // Clone necessary data for the background task
        let pool_tx = tx.clone();
        let subscription_arc = self.subscription.clone();
        let rpc_url = self.rpc_url.clone();
        let commitment = self.commitment;

        // Spawn background task to process log messages
        let task_handle = tokio::spawn(async move {
            info!("Attempting to connect to Solana websocket at {}", rpc_url);

            // First, create a PubsubClient instance
            match PubsubClient::new(&rpc_url).await {
                Ok(pubsub_client) => {
                    // Then use the client to subscribe to logs
                    match pubsub_client.logs_subscribe(
                        RpcTransactionLogsFilter::Mentions(vec![program_id.to_string()]),
                        RpcTransactionLogsConfig {
                            commitment: Some(commitment),
                        },
                    ).await {
                        Ok((mut logs_receiver, unsubscribe_fn)) => {
                            info!("Websocket subscription active, listening for Meteora pool creation events");

                            // Handle log messages until cancelled
                            tokio::select! {
                                _ = async {
                                    while let Some(log_entry) = logs_receiver.next().await {
                                        debug!("Received log entry: {:?}", log_entry);

                                        // Get the logs which are already a Vec<String>, not an Option
                                        let logs = log_entry.value.logs;
                                        
                                        // Check if this is a pool creation transaction
                                        let is_pool_creation = logs.iter().any(|msg| 
                                            msg.contains("Pool created")
                                        );

                                        if is_pool_creation {
                                            // Extract pool information from logs
                                            let monitor = MeteoraPoolMonitor::new(String::new());
                                            if let Some(pool) = monitor.extract_pool_info(&logs) {
                                                info!("Discovered new Meteora pool: {}", pool.address);
                                                
                                                // Send the pool to the processor
                                                if let Err(e) = pool_tx.send(pool).await {
                                                    error!("Failed to send discovered pool: {}", e);
                                                }
                                            }
                                        }
                                    }
                                } => {
                                    info!("Websocket connection closed");
                                },
                                _ = cancel_rx => {
                                    info!("Websocket subscription cancelled");
                                    let _ = unsubscribe_fn().await;
                                }
                            }
                        },
                        Err(e) => {
                            error!("Failed to subscribe to logs: {:?}", e);
                        }
                    }
                },
                Err(e) => {
                    error!("Failed to create PubsubClient: {:?}", e);
                }
            }

            // Clear the subscription when done
            let mut subscription = subscription_arc.lock().unwrap();
            *subscription = None;
        });

        // Store the subscription
        let subscription = WebsocketSubscription {
            cancel_sender: cancel_tx,
            task_handle,
        };

        // Store the subscription in our state
        *self.subscription.lock().unwrap() = Some(subscription);

        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        info!("Stopping Meteora pool monitoring...");
        
        // Get the current subscription
        let mut subscription_guard = self.subscription.lock().unwrap();
        if let Some(subscription) = subscription_guard.take() {
            // Send the cancel signal
            if let Err(_) = subscription.cancel_sender.send(()) {
                // The task may have already completed, which is fine
            }

            // Await the task to complete (with timeout)
            tokio::select! {
                _ = subscription.task_handle => {
                    info!("Websocket task completed successfully");
                },
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(5)) => {
                    info!("Websocket task shutdown timed out, continuing anyway");
                }
            }
        }

        Ok(())
    }
} 