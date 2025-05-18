use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use chrono::{DateTime, Utc};

/// Represents a Meteora V2 pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pool {
    pub address: Pubkey,
    pub token_a: TokenInfo,
    pub token_b: TokenInfo,
    pub discovered_at: DateTime<Utc>,
    pub analyzed: bool,
    pub score: Option<f64>,
}

/// Information about a token in a pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub mint: Pubkey,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
} 