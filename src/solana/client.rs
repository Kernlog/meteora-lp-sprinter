use anyhow::Result;
use log::debug;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;

/// Wrapper around Solana RPC client with retry logic and error handling
pub struct SolanaClient {
    rpc_client: RpcClient,
}

impl SolanaClient {
    /// Create a new Solana client with the given RPC URL
    pub fn new(rpc_url: &str) -> Self {
        let commitment = CommitmentConfig::confirmed();
        let rpc_client = RpcClient::new_with_commitment(rpc_url.to_string(), commitment);
        
        Self { rpc_client }
    }
    
    /// Get the current Solana slot
    pub fn get_slot(&self) -> Result<u64> {
        debug!("Getting current slot");
        let slot = self.rpc_client.get_slot()?;
        Ok(slot)
    }
    
    // TODO: Add more methods for common Solana RPC operations with retry logic
} 