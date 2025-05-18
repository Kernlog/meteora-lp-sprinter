use log::{info, error};
use anyhow::{Result, Context};
use dotenv::dotenv;
use std::path::PathBuf;
use std::time::Duration;

mod config;
mod solana;
mod db;
mod models;
mod monitoring;
mod strategy;
mod meteora;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize environment variables
    dotenv().ok();
    
    // Initialize logging
    init_logger();
    
    info!("Starting Meteora LP Sprinter...");
    
    // Load configuration
    let config = config::load_config()?;
    info!("Configuration loaded");
    
    // Initialize Solana infrastructure
    let solana_client = solana::create_client_from_config(&config);
    
    // Create connection pool with fallback RPCs
    let fallback_rpcs = vec![
        "https://api.mainnet-beta.solana.com".to_string(),
        "https://solana-api.projectserum.com".to_string(),
    ];
    let solana_pool = solana::create_pool_from_config(&config, Some(fallback_rpcs));
    
    // Start the connection health check task
    solana_pool.start_health_check_task().await;
    
    // Initialize wallet from keypair file
    let wallet_path = std::env::var("WALLET_KEYPAIR_PATH")
        .unwrap_or_else(|_| "wallet-keypair.json".to_string());
        
    let wallet_manager = solana::create_wallet_manager_from_config(&config, &wallet_path)
        .with_context(|| format!("Failed to load wallet from {}", wallet_path))?;
    
    info!("Wallet loaded with pubkey: {}", wallet_manager.pubkey());
    
    // Start balance monitoring in the background
    wallet_manager.start_balance_monitoring(60).await;
    info!("Started wallet balance monitoring");
    
    // Verify connection by getting current slot
    match solana_client.get_slot() {
        Ok(slot) => info!("Current Solana slot: {} - RPC connection established", slot),
        Err(e) => {
            error!("Failed to connect to Solana RPC: {}", e);
            return Err(anyhow::anyhow!("Could not establish Solana RPC connection"));
        }
    }
    
    // Check wallet balance
    let balance = wallet_manager.get_balance().await?;
    info!("Wallet balance: {} SOL", balance as f64 / 1_000_000_000.0);
    
    // Check if balance is sufficient for operation
    let min_balance = 0.001; // 0.001 SOL
    if !wallet_manager.has_sufficient_balance(0.0, true).await? {
        error!("Wallet balance too low for operation! Minimum: {} SOL", min_balance);
        return Err(anyhow::anyhow!("Insufficient wallet balance"));
    }
    
    // Connect to database
    let _db = db::Database::new(&config.database_path).await?;
    info!("Database initialized");
    
    // TODO: Implement main application logic
    
    info!("Shutting down...");
    Ok(())
}

fn init_logger() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or("RUST_LOG", "info")
    );
}
