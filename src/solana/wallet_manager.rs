use anyhow::{Result, Context, anyhow};
use log::{info, warn, debug};
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::system_instruction;
use solana_sdk::transaction::Transaction;
use std::path::Path;
use std::time::{Duration, Instant};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tokio::time;

use crate::solana::client::SolanaClient;
use crate::solana::wallet::Wallet;

const LAMPORTS_PER_SOL: u64 = 1_000_000_000;
const MIN_BALANCE_LAMPORTS: u64 = 5_000_000; // 0.005 SOL minimum balance

/// A higher-level wallet manager that provides balance checking and SOL transfer utilities
pub struct WalletManager {
    /// The underlying wallet
    wallet: Wallet,
    /// The Solana client to use for RPC calls
    client: Arc<SolanaClient>,
    /// Cached balance (in lamports)
    cached_balance: AtomicU64,
    /// Last time the balance was updated
    last_balance_check: Mutex<Option<Instant>>,
}

impl WalletManager {
    /// Create a new wallet manager with the given wallet and client
    pub fn new(wallet: Wallet, client: SolanaClient) -> Self {
        Self {
            wallet,
            client: Arc::new(client),
            cached_balance: AtomicU64::new(0),
            last_balance_check: Mutex::new(None),
        }
    }
    
    /// Load a wallet from a keypair file and create a wallet manager
    pub fn from_file<P: AsRef<Path>>(path: P, client: SolanaClient) -> Result<Self> {
        let wallet = Wallet::from_file(path)?;
        Ok(Self::new(wallet, client))
    }
    
    /// Get the public key of the wallet
    pub fn pubkey(&self) -> Pubkey {
        self.wallet.pubkey()
    }
    
    /// Get the wallet's balance in lamports
    pub async fn get_balance(&self) -> Result<u64> {
        let balance = self.client.get_balance(&self.wallet.pubkey())?;
        
        // Update cached balance
        self.cached_balance.store(balance, Ordering::SeqCst);
        *self.last_balance_check.lock().unwrap() = Some(Instant::now());
        
        Ok(balance)
    }
    
    /// Get the wallet's balance in SOL
    pub async fn get_balance_sol(&self) -> Result<f64> {
        let balance = self.get_balance().await?;
        Ok(balance as f64 / LAMPORTS_PER_SOL as f64)
    }
    
    /// Check if the wallet has enough SOL for the given amount plus fees
    pub async fn has_sufficient_balance(&self, amount_sol: f64, include_fees: bool) -> Result<bool> {
        let amount_lamports = (amount_sol * LAMPORTS_PER_SOL as f64) as u64;
        let fee_estimate = if include_fees { 10_000 } else { 0 }; // Estimate 0.00001 SOL for fees
        
        let balance = self.get_balance().await?;
        
        Ok(balance >= amount_lamports.saturating_add(fee_estimate).saturating_add(MIN_BALANCE_LAMPORTS))
    }
    
    /// Create and send a SOL transfer transaction
    pub async fn transfer_sol(&self, recipient: &Pubkey, amount_sol: f64) -> Result<String> {
        let amount_lamports = (amount_sol * LAMPORTS_PER_SOL as f64) as u64;
        
        // Check balance
        let balance = self.get_balance().await?;
        if balance < amount_lamports.saturating_add(MIN_BALANCE_LAMPORTS) {
            return Err(anyhow!("Insufficient balance for transfer: {} SOL (min: {} SOL)",
                amount_sol, (MIN_BALANCE_LAMPORTS as f64) / (LAMPORTS_PER_SOL as f64)));
        }
        
        // Get recent blockhash
        let blockhash = self.client.get_latest_blockhash()?;
        
        // Create transfer instruction
        let instruction = system_instruction::transfer(
            &self.wallet.pubkey(),
            recipient,
            amount_lamports,
        );
        
        // Create and sign transaction
        let transaction = self.wallet.create_and_sign_transaction(
            vec![instruction],
            blockhash,
            None,
        )?;
        
        // Send transaction
        let signature = self.client.send_and_confirm_transaction(&transaction)?;
        
        // Update cached balance
        let new_balance = balance.saturating_sub(amount_lamports);
        self.cached_balance.store(new_balance, Ordering::SeqCst);
        
        info!("Transferred {} SOL to {}", amount_sol, recipient);
        Ok(signature)
    }
    
    /// Start a background task to periodically refresh the wallet balance
    pub async fn start_balance_monitoring(&self, interval_secs: u64) {
        // Create a cloneable shared state
        let wallet_pubkey = self.wallet.pubkey();
        let client = self.client.clone();
        
        // Create weak reference to self
        let balance_ref = Arc::new(AtomicU64::new(self.cached_balance.load(Ordering::SeqCst)));
        let time_ref = Arc::new(Mutex::new(None::<Instant>));
        
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(interval_secs));
            
            loop {
                interval.tick().await;
                
                match client.get_balance(&wallet_pubkey) {
                    Ok(balance) => {
                        // Update stored balance
                        let old_balance = balance_ref.load(Ordering::SeqCst);
                        balance_ref.store(balance, Ordering::SeqCst);
                        
                        if old_balance != balance {
                            debug!("Wallet balance updated: {} SOL", balance as f64 / LAMPORTS_PER_SOL as f64);
                        }
                        
                        // Update last check time
                        let mut lock = time_ref.lock().unwrap();
                        *lock = Some(Instant::now());
                    },
                    Err(e) => {
                        warn!("Failed to refresh wallet balance: {}", e);
                    }
                }
            }
        });
    }
    
    /// Generate a random keypair and create a wallet from it
    pub fn generate_random() -> Self {
        let wallet = Wallet::new();
        let client = SolanaClient::new("https://api.mainnet-beta.solana.com");
        
        Self::new(wallet, client)
    }
    
    /// Get the underlying wallet
    pub fn wallet(&self) -> &Wallet {
        &self.wallet
    }
    
    /// Get the underlying Solana client
    pub fn client(&self) -> Arc<SolanaClient> {
        self.client.clone()
    }
    
    /// Check if the cached balance is stale (older than the given duration)
    pub fn is_balance_stale(&self, stale_threshold: Duration) -> bool {
        let last_check = self.last_balance_check.lock().unwrap();
        
        match *last_check {
            Some(time) => time.elapsed() > stale_threshold,
            None => true,
        }
    }
    
    /// Refresh the balance if it's stale
    pub async fn refresh_balance_if_stale(&self, stale_threshold: Duration) -> Result<u64> {
        if self.is_balance_stale(stale_threshold) {
            self.get_balance().await
        } else {
            Ok(self.cached_balance.load(Ordering::SeqCst))
        }
    }
    
    /// Create an airdrop request for devnet/testnet
    pub async fn request_airdrop(&self, amount_sol: f64) -> Result<String> {
        let amount_lamports = (amount_sol * LAMPORTS_PER_SOL as f64) as u64;
        
        // Send airdrop request
        let signature = self.client.rpc_client()
            .request_airdrop(&self.wallet.pubkey(), amount_lamports)
            .with_context(|| format!("Failed to request airdrop of {} SOL", amount_sol))?;
        
        // Wait for confirmation
        self.client.rpc_client()
            .confirm_transaction_with_commitment(&signature, CommitmentConfig::confirmed())
            .with_context(|| format!("Failed to confirm airdrop transaction"))?;
            
        // Update cached balance
        let _ = self.get_balance().await;
        
        info!("Received airdrop of {} SOL", amount_sol);
        Ok(signature.to_string())
    }
} 