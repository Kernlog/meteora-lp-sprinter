use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use chrono::{DateTime, Utc};

/// Represents a liquidity position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub pool: Pubkey,
    pub created_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub sol_invested: f64,
    pub fee_claimed: Option<f64>,
    pub profit_loss: Option<f64>,
    pub status: PositionStatus,
}

/// Status of a liquidity position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionStatus {
    Created,
    Active,
    ClaimingFees,
    Exiting,
    Closed,
    Failed,
} 