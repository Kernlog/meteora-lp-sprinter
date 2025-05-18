pub mod client;
pub mod wallet;
pub mod connection;
pub mod rpc_helpers;
pub mod wallet_manager;

pub use client::SolanaClient;
pub use client::RetryConfig;
pub use wallet::Wallet;
pub use wallet_manager::WalletManager;
pub use connection::ConnectionPool;
pub use connection::ConnectionPoolConfig;
pub use connection::ConnectionStatus;
pub use rpc_helpers::*;

use crate::config::Config;
use anyhow::{Result, Context};
use std::path::PathBuf;

/// Create a Solana client from the application configuration
pub fn create_client_from_config(config: &Config) -> SolanaClient {
    SolanaClient::new(&config.rpc_url)
}

/// Create a connection pool from the application configuration
pub fn create_pool_from_config(config: &Config, fallback_urls: Option<Vec<String>>) -> ConnectionPool {
    // Start with the primary URL from config
    let mut urls = vec![config.rpc_url.clone()];
    
    // Add any fallback URLs if provided
    if let Some(fallbacks) = fallback_urls {
        urls.extend(fallbacks);
    }
    
    let pool_config = ConnectionPoolConfig {
        min_connections: 1,
        max_connections: 3,
        health_check_interval_secs: 60,
        rpc_urls: urls,
        retry_config: RetryConfig::default(),
    };
    
    ConnectionPool::new(pool_config)
}

/// Create a wallet manager from the application configuration
pub fn create_wallet_manager_from_config(config: &Config, wallet_path: &str) -> Result<WalletManager> {
    let client = create_client_from_config(config);
    
    WalletManager::from_file(PathBuf::from(wallet_path), client)
        .with_context(|| format!("Failed to create wallet manager from {}", wallet_path))
} 