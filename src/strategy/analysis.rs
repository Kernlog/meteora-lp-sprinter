use anyhow::Result;
use log::{info, debug};
use crate::models::Pool;
use crate::solana::SolanaClient;
use crate::meteora::MeteoraClient;
use crate::meteora::PoolInfo;
use crate::solana::rpc_helpers;

/// Analyzes and scores pools for potential profitability
pub struct PoolAnalyzer {
    client: SolanaClient,
    meteora_client: MeteoraClient,
}

impl PoolAnalyzer {
    /// Create a new pool analyzer
    pub fn new(client: SolanaClient) -> Self {
        let meteora_client = MeteoraClient::new(client.clone());
        Self { client, meteora_client }
    }
    
    /// Analyze a pool and calculate a score
    pub async fn analyze_pool(&self, pool: &mut Pool) -> Result<f64> {
        debug!("Analyzing pool: {}", pool.address);
        
        // First, fetch token metadata if not already populated
        self.populate_token_metadata(pool).await?;
        
        // Get pool info from Meteora
        let pool_info = self.meteora_client.get_pool_info(&pool.address).await?;
        
        // Calculate liquidity factors
        let liquidity_score = self.analyze_liquidity(&pool_info).await?;
        
        // Calculate potential yield
        let yield_score = self.calculate_yield_potential(&pool_info).await?;
        
        // Calculate final score (0-1 range)
        // We weight the factors based on importance:
        // - Liquidity is important for execution (50%)
        // - Yield potential directly impacts profit (50%)
        let score = 0.5 * liquidity_score + 0.5 * yield_score;
        
        // Ensure score is between 0 and 1
        let clamped_score = score.max(0.0).min(1.0);
        
        // Store the score in the pool
        pool.score = Some(clamped_score);
        pool.analyzed = true;
        
        info!("Pool {} analyzed with score: {:.2}", pool.address, clamped_score);
        Ok(clamped_score)
    }
    
    /// Populate token metadata for the pool
    async fn populate_token_metadata(&self, pool: &mut Pool) -> Result<()> {
        // Only fetch if metadata is missing
        if pool.token_a.name.is_none() || pool.token_a.symbol.is_none() || pool.token_a.decimals.is_none() {
            debug!("Fetching metadata for token A: {}", pool.token_a.mint);
            let token_info = rpc_helpers::fetch_token_info(&self.client, &pool.token_a.mint).await?;
            pool.token_a.name = token_info.name;
            pool.token_a.symbol = token_info.symbol;
            pool.token_a.decimals = token_info.decimals;
        }
        
        if pool.token_b.name.is_none() || pool.token_b.symbol.is_none() || pool.token_b.decimals.is_none() {
            debug!("Fetching metadata for token B: {}", pool.token_b.mint);
            let token_info = rpc_helpers::fetch_token_info(&self.client, &pool.token_b.mint).await?;
            pool.token_b.name = token_info.name;
            pool.token_b.symbol = token_info.symbol;
            pool.token_b.decimals = token_info.decimals;
        }
        
        Ok(())
    }
    
    /// Analyze liquidity of the pool
    async fn analyze_liquidity(&self, pool_info: &PoolInfo) -> Result<f64> {
        // Get total value locked in SOL
        let tvl = self.meteora_client.get_pool_tvl(pool_info).await?;
        
        // Score based on TVL (0-1)
        // We want at least 100 SOL for maximum score, with diminishing returns after that
        // Less than 10 SOL is considered low liquidity
        let liquidity_score = if tvl <= 10.0 {
            tvl / 10.0 * 0.5  // Scale up to 0.5 for 0-10 SOL
        } else if tvl <= 100.0 {
            0.5 + (tvl - 10.0) / 90.0 * 0.5  // Scale from 0.5 to 1.0 for 10-100 SOL
        } else {
            1.0  // Maximum score for >100 SOL
        };
        
        // Check balance between token A and token B
        // Balanced pools are preferred for providing liquidity
        let a_fraction = pool_info.token_a_amount as f64 / 
            (pool_info.token_a_amount as f64 + pool_info.token_b_amount as f64);
        
        // Balance score (1.0 = perfectly balanced, 0.0 = all in one token)
        let balance_score = if a_fraction <= 0.5 {
            a_fraction * 2.0
        } else {
            (1.0 - a_fraction) * 2.0
        };
        
        // Combine liquidity and balance scores
        // We value liquidity more than perfect balance
        let combined_score = liquidity_score * 0.7 + balance_score * 0.3;
        
        debug!("Liquidity score for pool {}: {}", pool_info.address, combined_score);
        Ok(combined_score)
    }
    
    /// Calculate potential yield from providing liquidity
    async fn calculate_yield_potential(&self, pool_info: &PoolInfo) -> Result<f64> {
        // Calculate fee APY
        let fee_apy = self.meteora_client.calculate_fee_yield(pool_info).await?;
        
        // Score based on APY (0-1)
        // 100% APY or higher is maximum score
        // 0-10% is low score
        let yield_score = if fee_apy <= 0.1 {
            fee_apy * 10.0 * 0.3  // 0-10% APY maps to 0-0.3 score
        } else if fee_apy <= 0.5 {
            0.3 + (fee_apy - 0.1) * (0.6 / 0.4)  // 10-50% APY maps to 0.3-0.9 score
        } else if fee_apy <= 1.0 {
            0.9 + (fee_apy - 0.5) * 0.2  // 50-100% APY maps to 0.9-1.0 score
        } else {
            1.0  // >100% APY is maximum score
        };
        
        debug!("Yield score for pool {}: {}", pool_info.address, yield_score);
        Ok(yield_score)
    }
    
    /// Determine if a pool meets the given criteria
    pub fn meets_criteria(&self, pool: &Pool, criteria: &PoolCriteria) -> bool {
        // Check if the pool has been analyzed
        if !pool.analyzed || pool.score.is_none() {
            return false;
        }
        
        // Check minimum score
        let score = pool.score.unwrap();
        if score < criteria.min_score {
            return false;
        }
        
        // Additional checks would go here
        // such as checking min_liquidity and max_token_holders if we had that data
        
        true
    }
}

/// Criteria for pool selection
pub struct PoolCriteria {
    pub min_score: f64,
    pub min_liquidity: u64,
    pub max_token_holders: Option<u64>,
}

impl Default for PoolCriteria {
    fn default() -> Self {
        Self {
            min_score: 0.7,
            min_liquidity: 10_000_000, // 10 SOL in lamports
            max_token_holders: Some(100),
        }
    }
} 