use anyhow::{Result, anyhow};
use log::{info, warn, error, debug};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time;

use crate::solana::client::{SolanaClient, RetryConfig};
use solana_sdk::commitment_config::CommitmentConfig;

/// Status of a connection in the pool
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Connection is healthy and available
    Healthy,
    /// Connection is currently being used
    InUse,
    /// Connection has failed and is being reconnected
    Reconnecting,
    /// Connection has failed permanently
    Failed,
}

/// Configuration for connection pooling
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Minimum number of connections to keep in the pool
    pub min_connections: usize,
    /// Maximum number of connections to allow in the pool
    pub max_connections: usize,
    /// How often to check connection health
    pub health_check_interval_secs: u64,
    /// RPC URLs to use
    pub rpc_urls: Vec<String>,
    /// Retry configuration for RPC clients
    pub retry_config: RetryConfig,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 1,
            max_connections: 3,
            health_check_interval_secs: 60,
            rpc_urls: vec!["https://api.mainnet-beta.solana.com".to_string()],
            retry_config: RetryConfig::default(),
        }
    }
}

/// A connection pool for managing multiple Solana RPC clients
pub struct ConnectionPool {
    clients: Arc<Mutex<HashMap<String, (SolanaClient, ConnectionStatus, Instant)>>>,
    config: ConnectionPoolConfig,
}

impl ConnectionPool {
    /// Create a new connection pool with the given configuration
    pub fn new(config: ConnectionPoolConfig) -> Self {
        let clients = Arc::new(Mutex::new(HashMap::new()));
        let pool = Self { clients, config };
        
        // Initialize the pool with min_connections
        pool.initialize();
        
        pool
    }
    
    /// Initialize the connection pool with the minimum number of connections
    fn initialize(&self) {
        let mut clients = self.clients.lock().unwrap();
        
        // Ensure we have at least min_connections
        for i in 0..self.config.min_connections {
            if clients.len() >= self.config.rpc_urls.len() {
                break;
            }
            
            let url_index = i % self.config.rpc_urls.len();
            let url = &self.config.rpc_urls[url_index];
            
            if !clients.contains_key(url) {
                let client = SolanaClient::new_with_config(
                    url, 
                    CommitmentConfig::confirmed(),
                    self.config.retry_config.clone()
                );
                
                clients.insert(
                    url.clone(),
                    (client, ConnectionStatus::Healthy, Instant::now())
                );
                
                info!("Initialized Solana RPC connection to {}", url);
            }
        }
    }
    
    /// Get an available client from the pool
    pub fn get_client(&self) -> Result<Arc<SolanaClient>> {
        let mut clients = self.clients.lock().unwrap();
        
        // Try to find a healthy client
        for (url, (client, status, last_used)) in clients.iter_mut() {
            if *status == ConnectionStatus::Healthy {
                *status = ConnectionStatus::InUse;
                *last_used = Instant::now();
                debug!("Using Solana RPC connection to {}", url);
                return Ok(Arc::new(client.clone()));
            }
        }
        
        // If no healthy client is available, try to create a new one if we haven't reached max_connections
        if clients.len() < self.config.max_connections && clients.len() < self.config.rpc_urls.len() {
            // Find a URL that isn't already in use
            for url in &self.config.rpc_urls {
                if !clients.contains_key(url) {
                    let client = SolanaClient::new_with_config(
                        url, 
                        CommitmentConfig::confirmed(),
                        self.config.retry_config.clone()
                    );
                    
                    // Check if the new client is healthy
                    if client.is_healthy() {
                        let client_arc = Arc::new(client.clone());
                        clients.insert(
                            url.clone(),
                            (client, ConnectionStatus::InUse, Instant::now())
                        );
                        
                        info!("Created new Solana RPC connection to {}", url);
                        return Ok(client_arc);
                    } else {
                        warn!("Failed to establish healthy connection to {}", url);
                    }
                }
            }
        }
        
        // If we still don't have a client, return an error
        Err(anyhow!("No available Solana RPC connections"))
    }
    
    /// Release a client back to the pool
    pub fn release_client(&self, url: &str) {
        let mut clients = self.clients.lock().unwrap();
        
        if let Some((_, status, _)) = clients.get_mut(url) {
            *status = ConnectionStatus::Healthy;
            debug!("Released Solana RPC connection to {}", url);
        }
    }
    
    /// Start a background task to periodically check connection health
    pub async fn start_health_check_task(&self) {
        let clients = self.clients.clone();
        let config = self.config.clone();
        
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(config.health_check_interval_secs));
            
            loop {
                interval.tick().await;
                
                let mut clients_lock = clients.lock().unwrap();
                
                // Check health of each connection
                for (url, (client, status, _)) in clients_lock.iter_mut() {
                    // Skip checking if the client is currently in use
                    if *status == ConnectionStatus::InUse {
                        continue;
                    }
                    
                    if client.is_healthy() {
                        if *status == ConnectionStatus::Reconnecting {
                            info!("Solana RPC connection to {} has been restored", url);
                        }
                        *status = ConnectionStatus::Healthy;
                    } else {
                        if *status == ConnectionStatus::Healthy {
                            warn!("Solana RPC connection to {} is unhealthy, marking for reconnection", url);
                        }
                        *status = ConnectionStatus::Reconnecting;
                    }
                }
                
                // Try to reconnect any clients in Reconnecting state
                for (url, (client, status, _)) in clients_lock.iter_mut() {
                    if *status == ConnectionStatus::Reconnecting {
                        let new_client = SolanaClient::new_with_config(
                            url, 
                            CommitmentConfig::confirmed(),
                            config.retry_config.clone()
                        );
                        
                        if new_client.is_healthy() {
                            *client = new_client;
                            *status = ConnectionStatus::Healthy;
                            info!("Successfully reconnected to Solana RPC at {}", url);
                        } else {
                            error!("Failed to reconnect to Solana RPC at {}", url);
                        }
                    }
                }
            }
        });
    }
} 