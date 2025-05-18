use anyhow::{Result, Context};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::fs::File;
use std::path::Path;
use chrono::{DateTime, Utc};
use solana_sdk::pubkey::Pubkey;

use crate::models::pool::{Pool, TokenInfo};

/// Database manager for handling SQLite operations
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Create a new database connection
    pub async fn new(database_url: &str) -> Result<Self> {
        // Ensure the database file exists
        if database_url != "sqlite::memory:" && !database_url.starts_with("sqlite:") {
            let db_path = Path::new(database_url);
            if !db_path.exists() {
                File::create(db_path)?;
            }
        }
        
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
            
        // Initialize schema if needed
        let db = Self { pool };
        db.init_schema().await?;
        
        Ok(db)
    }
    
    /// Initialize the database schema
    pub async fn init_schema(&self) -> Result<()> {
        // Create tables if they don't exist
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS pools (
                address TEXT PRIMARY KEY,
                token_a_mint TEXT NOT NULL,
                token_b_mint TEXT NOT NULL,
                token_a_name TEXT,
                token_b_name TEXT,
                token_a_symbol TEXT,
                token_b_symbol TEXT,
                token_a_decimals INTEGER,
                token_b_decimals INTEGER,
                discovered_at TIMESTAMP NOT NULL,
                analyzed BOOLEAN NOT NULL DEFAULT FALSE,
                score REAL
            )"
        )
        .execute(&self.pool)
        .await?;
        
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS positions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pool_address TEXT NOT NULL,
                created_at TIMESTAMP NOT NULL,
                closed_at TIMESTAMP,
                sol_invested REAL NOT NULL,
                fee_claimed REAL,
                profit_loss REAL,
                status TEXT NOT NULL,
                FOREIGN KEY (pool_address) REFERENCES pools (address)
            )"
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Save a pool to the database
    pub async fn save_pool(&self, pool: &Pool) -> Result<()> {
        // Convert DateTime to ISO-8601 string which SQLite understands
        let discovered_at_str = pool.discovered_at.to_rfc3339();
        
        // Insert or update the pool
        sqlx::query(
            "INSERT OR REPLACE INTO pools (
                address,
                token_a_mint, token_a_name, token_a_symbol, token_a_decimals,
                token_b_mint, token_b_name, token_b_symbol, token_b_decimals,
                discovered_at, analyzed, score
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(pool.address.to_string())
        .bind(pool.token_a.mint.to_string())
        .bind(&pool.token_a.name)
        .bind(&pool.token_a.symbol)
        .bind(pool.token_a.decimals)
        .bind(pool.token_b.mint.to_string())
        .bind(&pool.token_b.name)
        .bind(&pool.token_b.symbol)
        .bind(pool.token_b.decimals)
        .bind(discovered_at_str)
        .bind(pool.analyzed)
        .bind(pool.score)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Get a pool by its address
    pub async fn get_pool(&self, address: &Pubkey) -> Result<Option<Pool>> {
        let pool = sqlx::query(
            "SELECT 
                address, 
                token_a_mint, token_a_name, token_a_symbol, token_a_decimals,
                token_b_mint, token_b_name, token_b_symbol, token_b_decimals,
                discovered_at, analyzed, score
            FROM pools
            WHERE address = ?"
        )
        .bind(address.to_string())
        .fetch_optional(&self.pool)
        .await?;
        
        match pool {
            Some(row) => {
                let pool = Pool {
                    address: row.get::<String, _>(0).parse()?,
                    token_a: TokenInfo {
                        mint: row.get::<String, _>(1).parse()?,
                        name: row.get(2),
                        symbol: row.get(3),
                        decimals: row.get::<Option<i64>, _>(4).map(|d| d as u8),
                    },
                    token_b: TokenInfo {
                        mint: row.get::<String, _>(5).parse()?,
                        name: row.get(6),
                        symbol: row.get(7),
                        decimals: row.get::<Option<i64>, _>(8).map(|d| d as u8),
                    },
                    discovered_at: row.get::<String, _>(9).parse::<DateTime<Utc>>()?,
                    analyzed: row.get(10),
                    score: row.get(11),
                };
                
                Ok(Some(pool))
            },
            None => Ok(None),
        }
    }
    
    /// List all pools with optional filtering
    pub async fn list_pools(&self, limit: Option<i64>, analyzed_only: bool) -> Result<Vec<Pool>> {
        let mut query = String::from(
            "SELECT 
                address, 
                token_a_mint, token_a_name, token_a_symbol, token_a_decimals,
                token_b_mint, token_b_name, token_b_symbol, token_b_decimals,
                discovered_at, analyzed, score
            FROM pools
            "
        );
        
        if analyzed_only {
            query.push_str("WHERE analyzed = TRUE ");
        }
        
        query.push_str("ORDER BY discovered_at DESC ");
        
        if let Some(limit) = limit {
            query.push_str(&format!("LIMIT {}", limit));
        }
        
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await?;
        
        let mut pools = Vec::new();
        for row in rows {
            let address: String = row.get(0);
            let token_a_mint: String = row.get(1);
            let token_a_name: Option<String> = row.get(2);
            let token_a_symbol: Option<String> = row.get(3);
            let token_a_decimals: Option<i64> = row.get(4);
            let token_b_mint: String = row.get(5);
            let token_b_name: Option<String> = row.get(6);
            let token_b_symbol: Option<String> = row.get(7);
            let token_b_decimals: Option<i64> = row.get(8);
            let discovered_at: String = row.get(9);
            let analyzed: bool = row.get(10);
            let score: Option<f64> = row.get(11);
            
            let pool = Pool {
                address: address.parse().context("Invalid address format")?,
                token_a: TokenInfo {
                    mint: token_a_mint.parse().context("Invalid token_a_mint format")?,
                    name: token_a_name,
                    symbol: token_a_symbol,
                    decimals: token_a_decimals.map(|d| d as u8),
                },
                token_b: TokenInfo {
                    mint: token_b_mint.parse().context("Invalid token_b_mint format")?,
                    name: token_b_name,
                    symbol: token_b_symbol,
                    decimals: token_b_decimals.map(|d| d as u8),
                },
                discovered_at: discovered_at.parse().context("Invalid timestamp format")?,
                analyzed,
                score,
            };
            
            pools.push(pool);
        }
        
        Ok(pools)
    }
}

/// Initialize an in-memory database for testing
#[cfg(test)]
pub async fn init_test_db() -> Result<Database> {
    Database::new("sqlite::memory:").await
} 