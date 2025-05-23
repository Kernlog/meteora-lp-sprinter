use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

#[cfg(feature = "telegram")]
use crate::monitoring::telegram::TelegramConfig;

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
    /// Path to the database file
    pub database_path: String,
    /// Whether to enable debug logging
    pub debug_logging: bool,
    /// Telegram monitoring configuration
    #[cfg(feature = "telegram")]
    pub telegram: Option<TelegramConfig>,
    #[cfg(not(feature = "telegram"))]
    pub telegram: Option<DummyTelegramConfig>,
}

/// Dummy structure for when the telegram feature is disabled
#[cfg(not(feature = "telegram"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DummyTelegramConfig {}

impl Default for Config {
    fn default() -> Self {
        Self {
            rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
            keypair_path: "keypair.json".to_string(),
            max_sol_per_position: 0.1,
            position_duration_seconds: 180, // 3 minutes
            fee_claim_interval_seconds: 60,
            database_path: "meteora_sprinter.db".to_string(),
            debug_logging: false,
            #[cfg(feature = "telegram")]
            telegram: None,
            #[cfg(not(feature = "telegram"))]
            telegram: None,
        }
    }
}

/// Loads configuration with the following priority:
/// 1. Environment variables (highest priority)
/// 2. JSON config file
/// 3. Default values (lowest priority)
pub fn load_config() -> Result<Config> {
    // Start with default configuration
    let mut config = Config::default();
    
    // Try to load from config file if it exists
    if let Some(file_config) = load_from_file()? {
        config = file_config;
    }
    
    // Override with environment variables (highest priority)
    apply_env_overrides(&mut config);
    
    Ok(config)
}

/// Loads configuration from a JSON file if available
fn load_from_file() -> Result<Option<Config>> {
    // Check for config file paths in order of preference
    let config_paths = [
        env::var("CONFIG_FILE").unwrap_or_else(|_| "config.json".to_string()),
        "./config.json".to_string(),
        format!("{}/.config/meteora-lp-sprinter/config.json", env::var("HOME").unwrap_or_else(|_| ".".to_string())),
    ];
    
    for path in &config_paths {
        let config_path = Path::new(path);
        if config_path.exists() {
            let file = File::open(config_path)
                .with_context(|| format!("Failed to open config file: {}", path))?;
            let reader = BufReader::new(file);
            let config = serde_json::from_reader(reader)
                .with_context(|| format!("Failed to parse config file: {}", path))?;
            return Ok(Some(config));
        }
    }
    
    // No config file found
    Ok(None)
}

/// Applies environment variable overrides to the configuration
fn apply_env_overrides(config: &mut Config) {
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
    
    if let Ok(db_path) = env::var("DATABASE_URL") {
        config.database_path = db_path;
    }
    
    if let Ok(debug) = env::var("DEBUG_LOGGING") {
        config.debug_logging = debug.to_lowercase() == "true" || debug == "1";
    }
    
    // Apply Telegram configuration from environment variables
    #[cfg(feature = "telegram")]
    apply_telegram_env_overrides(config);
}

/// Applies Telegram-specific environment variables to the configuration
#[cfg(feature = "telegram")]
fn apply_telegram_env_overrides(config: &mut Config) {
    let mut telegram_config = config.telegram.clone().unwrap_or_default();
    
    if let Ok(api_id) = env::var("TELEGRAM_API_ID") {
        if let Ok(value) = api_id.parse::<i32>() {
            telegram_config.api_id = value;
        }
    }
    
    if let Ok(api_hash) = env::var("TELEGRAM_API_HASH") {
        telegram_config.api_hash = api_hash;
    }
    
    if let Ok(phone) = env::var("TELEGRAM_PHONE") {
        telegram_config.phone_number = phone;
    }
    
    if let Ok(session_path) = env::var("TELEGRAM_SESSION_PATH") {
        telegram_config.session_path = session_path;
    }
    
    if let Ok(channels) = env::var("TELEGRAM_CHANNELS") {
        telegram_config.channels = channels.split(',')
            .map(|s| s.trim().to_string())
            .collect();
    }
    
    // Only set telegram config if we have the minimum required fields
    if telegram_config.api_id != 0 && !telegram_config.api_hash.is_empty() && !telegram_config.phone_number.is_empty() {
        config.telegram = Some(telegram_config);
    }
} 