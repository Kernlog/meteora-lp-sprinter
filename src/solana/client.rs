use anyhow::{Result, Context, anyhow};
use log::{debug, info, warn, error};
use solana_client::rpc_client::RpcClient;
use solana_client::client_error::ClientError;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::transaction::Transaction;
use solana_sdk::account::Account;
use solana_sdk::clock::Slot;
use solana_sdk::hash::Hash;
use std::time::{Duration, Instant};
use std::str::FromStr;
use std::sync::Arc;
use std::thread;

/// Configuration for client retries
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retries for a request
    pub max_retries: u32,
    /// Base delay between retries (will be increased exponentially)
    pub base_delay_ms: u64,
    /// Maximum delay between retries
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            base_delay_ms: 500,
            max_delay_ms: 20000,
        }
    }
}

/// Wrapper around Solana RPC client with retry logic and error handling
pub struct SolanaClient {
    rpc_client: RpcClient,
    retry_config: RetryConfig,
}

// Manual implementation of Clone since RpcClient doesn't implement Clone
impl Clone for SolanaClient {
    fn clone(&self) -> Self {
        // Create a new RPC client with the same configuration
        let commitment = self.rpc_client.commitment();
        let url = self.rpc_client.url().to_string();
        
        Self {
            rpc_client: RpcClient::new_with_commitment(url, commitment),
            retry_config: self.retry_config.clone(),
        }
    }
}

impl SolanaClient {
    /// Create a new Solana client with the given RPC URL
    pub fn new(rpc_url: &str) -> Self {
        let commitment = CommitmentConfig::confirmed();
        let rpc_client = RpcClient::new_with_commitment(rpc_url.to_string(), commitment);
        
        Self { 
            rpc_client,
            retry_config: RetryConfig::default()
        }
    }
    
    /// Create a new Solana client with custom commitment and retry configuration
    pub fn new_with_config(rpc_url: &str, commitment: CommitmentConfig, retry_config: RetryConfig) -> Self {
        let rpc_client = RpcClient::new_with_commitment(rpc_url.to_string(), commitment);
        
        Self { 
            rpc_client,
            retry_config
        }
    }
    
    /// Get the current Solana slot with retries
    pub fn get_slot(&self) -> Result<Slot> {
        self.with_retry(|| {
            debug!("Getting current slot");
            self.rpc_client.get_slot()
        })
    }
    
    /// Get the recent blockhash with retries
    pub fn get_latest_blockhash(&self) -> Result<Hash> {
        self.with_retry(|| {
            debug!("Getting latest blockhash");
            self.rpc_client.get_latest_blockhash()
        })
    }
    
    /// Get an account with retries
    pub fn get_account(&self, pubkey: &Pubkey) -> Result<Account> {
        self.with_retry(|| {
            debug!("Getting account: {}", pubkey);
            self.rpc_client.get_account(pubkey)
        })
    }
    
    /// Check if an account exists
    pub fn account_exists(&self, pubkey: &Pubkey) -> Result<bool> {
        match self.get_account(pubkey) {
            Ok(_) => Ok(true),
            Err(e) => {
                // If the error is account not found, return false
                if e.to_string().contains("AccountNotFound") {
                    Ok(false)
                } else {
                    // Otherwise, propagate the error
                    Err(e)
                }
            }
        }
    }
    
    /// Get account balance with retries
    pub fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        self.with_retry(|| {
            debug!("Getting balance for account: {}", pubkey);
            self.rpc_client.get_balance(pubkey)
        })
    }
    
    /// Get multiple accounts with retries
    pub fn get_multiple_accounts(&self, pubkeys: &[Pubkey]) -> Result<Vec<Option<Account>>> {
        self.with_retry(|| {
            debug!("Getting {} accounts", pubkeys.len());
            self.rpc_client.get_multiple_accounts(pubkeys)
        })
    }
    
    /// Send and confirm transaction with retries
    pub fn send_and_confirm_transaction(&self, transaction: &Transaction) -> Result<String> {
        self.with_retry(|| {
            debug!("Sending and confirming transaction");
            let signature = self.rpc_client.send_and_confirm_transaction(transaction)?;
            Ok(signature.to_string())
        })
    }
    
    /// Get Solana program accounts with retries
    pub fn get_program_accounts(&self, program_id: &Pubkey) -> Result<Vec<(Pubkey, Account)>> {
        self.with_retry(|| {
            debug!("Getting program accounts for: {}", program_id);
            self.rpc_client.get_program_accounts(program_id)
        })
    }
    
    /// Get a transaction by signature
    pub fn get_transaction(&self, signature: &str) -> Result<solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta> {
        let signature_obj = match solana_sdk::signature::Signature::from_str(signature) {
            Ok(sig) => sig,
            Err(err) => return Err(anyhow!("Invalid signature format: {}", err)),
        };
            
        self.with_retry(|| {
            debug!("Getting transaction: {}", signature);
            self.rpc_client.get_transaction_with_config(
                &signature_obj,
                solana_client::rpc_config::RpcTransactionConfig {
                    encoding: Some(solana_transaction_status::UiTransactionEncoding::Json),
                    commitment: Some(self.rpc_client.commitment()),
                    max_supported_transaction_version: Some(0),
                }
            )
        }).map_err(|e| anyhow!("Failed to get transaction: {}", e))
    }
    
    /// Helper function to execute a function with retry logic
    fn with_retry<T, F>(&self, mut operation: F) -> Result<T> 
    where
        F: FnMut() -> std::result::Result<T, ClientError>,
    {
        let mut retries = 0;
        let start = Instant::now();
        
        loop {
            match operation() {
                Ok(value) => {
                    if retries > 0 {
                        debug!("Operation succeeded after {} retries in {:?}", retries, start.elapsed());
                    }
                    return Ok(value);
                }
                Err(err) => {
                    if retries >= self.retry_config.max_retries {
                        return Err(anyhow!("Operation failed after {} retries: {}", retries, err));
                    }
                    
                    // Calculate exponential backoff with jitter
                    let backoff_ms = self.calculate_backoff(retries);
                    
                    warn!("RPC request failed (retry {}/{}), backing off for {}ms: {}",
                         retries + 1, self.retry_config.max_retries, backoff_ms, err);
                    
                    thread::sleep(Duration::from_millis(backoff_ms));
                    retries += 1;
                }
            }
        }
    }
    
    /// Calculate backoff duration with exponential increase and jitter
    fn calculate_backoff(&self, retry: u32) -> u64 {
        let base = self.retry_config.base_delay_ms;
        let max = self.retry_config.max_delay_ms;
        
        // Exponential backoff: base * 2^retry (using bit shifting)
        let retry_power = 1u64 << retry.min(16);
        let exp_backoff = base.saturating_mul(retry_power);
        
        // Add jitter: +/- 25% of the calculated backoff
        let jitter_factor = (fastrand::f64() - 0.5) * 0.5 + 1.0;
        
        // Apply jitter and cap at max delay
        let with_jitter = (exp_backoff as f64 * jitter_factor) as u64;
        
        with_jitter.min(max)
    }
    
    /// Check if the connection is healthy
    pub fn is_healthy(&self) -> bool {
        match self.get_slot() {
            Ok(_) => true,
            Err(e) => {
                error!("RPC connection is unhealthy: {}", e);
                false
            }
        }
    }
    
    /// Get the underlying RPC client
    pub fn rpc_client(&self) -> &RpcClient {
        &self.rpc_client
    }
} 