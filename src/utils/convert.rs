use anyhow::Result;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

/// Convert a string to a Pubkey
pub fn pubkey_from_str(pubkey_str: &str) -> Result<Pubkey> {
    Ok(Pubkey::from_str(pubkey_str)?)
}

/// Convert lamports to SOL
pub fn lamports_to_sol(lamports: u64) -> f64 {
    lamports as f64 / 1_000_000_000.0
}

/// Convert SOL to lamports
pub fn sol_to_lamports(sol: f64) -> u64 {
    (sol * 1_000_000_000.0) as u64
} 