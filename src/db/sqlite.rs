use anyhow::Result;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::fs::File;
use std::path::Path;

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
    
    // TODO: Add methods for pool tracking, transaction history, etc.
}

/// Initialize an in-memory database for testing
#[cfg(test)]
pub async fn init_test_db() -> Result<Database> {
    Database::new("sqlite::memory:").await
} 