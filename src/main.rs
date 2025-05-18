use log::info;
use anyhow::Result;
use dotenv::dotenv;

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
    
    // Initialize Solana client
    let _solana_client = solana::SolanaClient::new(&config.rpc_url);
    
    // Connect to database
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| String::from("meteora_sprinter.db"));
    let _db = db::Database::new(&db_url).await?;
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
