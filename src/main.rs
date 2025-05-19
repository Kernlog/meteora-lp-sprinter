use log::{info, error};
use anyhow::{Result, Context};
use dotenv::dotenv;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use chrono::Utc;
use solana_sdk::pubkey::Pubkey;

mod config;
mod solana;
mod db;
mod models;
mod monitoring;
mod strategy;
mod meteora;
mod utils;

use crate::monitoring::PoolMonitor;
// Temporarily comment out for testing build
// use monitoring::telegram::TelegramMonitor;
use monitoring::websocket::MeteoraPoolMonitor;
use models::pool::{Pool, TokenInfo};
use strategy::analysis::{PoolAnalyzer, PoolCriteria};

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
    let db = db::Database::new(&config.database_path).await?;
    info!("Database initialized");
    
    // Initialize the pool analyzer with default criteria
    let pool_analyzer = PoolAnalyzer::new(solana_client.clone());
    let pool_criteria = PoolCriteria::default();
    info!("Pool analyzer initialized with min score: {}", pool_criteria.min_score);
    
    // Create a channel for pool discovery
    let (pool_tx, mut pool_rx) = mpsc::channel::<Pool>(100);
    
    // Temporarily comment out Telegram monitoring initialization for testing build
    /*
    // Initialize the TelegramMonitor if configured
    if let Some(telegram_config) = config.telegram.clone() {
        info!("Initializing Telegram monitoring...");
        let mut telegram_monitor = match TelegramMonitor::new(telegram_config) {
            Ok(monitor) => monitor,
            Err(e) => {
                error!("Failed to initialize Telegram monitor: {}", e);
                error!("If this is an authentication issue, run the telegram_auth binary first.");
                return Err(anyhow::anyhow!("Failed to initialize Telegram monitor"));
            }
        };
        
        // Start the monitor
        match telegram_monitor.start_monitoring(pool_tx.clone()).await {
            Ok(_) => info!("Telegram monitoring started successfully"),
            Err(e) => {
                error!("Failed to start Telegram monitoring: {}", e);
                error!("If this is an authentication issue, run the telegram_auth binary first.");
                return Err(anyhow::anyhow!("Failed to start Telegram monitoring"));
            }
        }
    } else {
        info!("Telegram monitoring disabled (no configuration found)");
    }
    */
    info!("Telegram monitoring temporarily disabled for testing build");
    
    // Initialize and start Meteora websocket monitoring
    info!("Initializing Meteora websocket monitoring...");
    let mut meteora_monitor = MeteoraPoolMonitor::new(config.rpc_url.clone());
    match meteora_monitor.start_monitoring(pool_tx.clone()).await {
        Ok(_) => info!("Meteora websocket monitoring started successfully"),
        Err(e) => {
            error!("Failed to start Meteora websocket monitoring: {}", e);
            return Err(anyhow::anyhow!("Failed to start Meteora websocket monitoring"));
        }
    }
    
    // Process discovered pools
    let pool_analyzer_clone = pool_analyzer;
    let pool_criteria_clone = pool_criteria;
    let db_clone = db.clone();
    
    let process_pools_handle = tokio::spawn(async move {
        info!("Starting pool processing loop");
        
        while let Some(mut pool) = pool_rx.recv().await {
            info!("New pool discovered: {}", pool.address);
            
            // Save the pool to the database
            match db.save_pool(&pool).await {
                Ok(_) => info!("Saved pool {} to database", pool.address),
                Err(e) => error!("Failed to save pool to database: {}", e),
            }
            
            // Analyze the pool
            info!("Analyzing pool {}", pool.address);
            match pool_analyzer_clone.analyze_pool(&mut pool).await {
                Ok(score) => {
                    info!("Pool {} analyzed, score: {:.2}", pool.address, score);
                    
                    // Update the pool in the database with analysis results
                    if let Err(e) = db_clone.save_pool(&pool).await {
                        error!("Failed to update pool analysis in database: {}", e);
                    }
                    
                    // Check if the pool meets our criteria for liquidity provision
                    if pool_analyzer_clone.meets_criteria(&pool, &pool_criteria_clone) {
                        info!("Pool {} meets criteria for liquidity provision!", pool.address);
                        // TODO: Implement liquidity provision strategy
                    } else {
                        info!("Pool {} does not meet criteria for liquidity provision", pool.address);
                    }
                },
                Err(e) => {
                    error!("Failed to analyze pool {}: {}", pool.address, e);
                }
            }
        }
    });
    
    // Wait for Ctrl+C signal
    tokio::signal::ctrl_c().await?;
    info!("Shutdown signal received");
    
    // Stop the Meteora websocket monitoring
    if let Err(e) = meteora_monitor.stop().await {
        error!("Error stopping Meteora websocket monitoring: {}", e);
    }
    
    // Close the pool channel to terminate the processing loop
    drop(pool_tx);
    
    // Wait for the processing to complete
    let _ = tokio::time::timeout(Duration::from_secs(5), process_pools_handle).await;
    
    info!("Shutting down...");
    Ok(())
}

fn init_logger() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or("RUST_LOG", "info")
    );
}
