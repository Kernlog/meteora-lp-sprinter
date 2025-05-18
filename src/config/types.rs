use serde::{Deserialize, Serialize};
use anyhow::Result;
use std::env;

/// Configuration for the Meteora LP Sprinter application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Solana RPC URL
    pub rpc_url: String,
    /// Wallet keypair path
    pub keypair_path: String,
    /// Maximum amount of SOL to use per position
    pub max_sol_per_position: f64,
    /// Number of seconds to hold a position before exiting
    pub position_duration_seconds: u64,
    /// How frequently to claim fees (in seconds)
    pub fee_claim_interval_seconds: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            keypair_path: "keypair.json".to_string(),
            max_sol_per_position: 0.1,
            position_duration_seconds: 180, // 3 minutes
            fee_claim_interval_seconds: 60,
        }
    }
}

/// Loads configuration from environment variables, falling back to default values
pub fn load_config() -> Result<Config> {
    let mut config = Config::default();
    
    // Load values from environment variables if available
    if let Ok(rpc_url) = env::var("RPC_URL") {
        config.rpc_url = rpc_url;
    }
    
    if let Ok(keypair_path) = env::var("KEYPAIR_PATH") {
        config.keypair_path = keypair_path;
    }
    
    if let Ok(max_sol) = env::var("MAX_SOL_PER_POSITION") {
        if let Ok(value) = max_sol.parse::<f64>() {
            config.max_sol_per_position = value;
        }
    }
    
    if let Ok(duration) = env::var("POSITION_DURATION_SECONDS") {
        if let Ok(value) = duration.parse::<u64>() {
            config.position_duration_seconds = value;
        }
    }
    
    if let Ok(interval) = env::var("FEE_CLAIM_INTERVAL_SECONDS") {
        if let Ok(value) = interval.parse::<u64>() {
            config.fee_claim_interval_seconds = value;
        }
    }
    
    Ok(config)
} 