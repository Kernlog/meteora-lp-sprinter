use anyhow::{Result, anyhow, Context};
use log::{debug, warn};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_request::RpcRequest;
use solana_client::rpc_config::{RpcProgramAccountsConfig, RpcAccountInfoConfig};
use solana_client::rpc_filter::{RpcFilterType, Memcmp, MemcmpEncodedBytes};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::account::Account;
use solana_sdk::commitment_config::CommitmentConfig;
use std::str::FromStr;
use borsh::BorshDeserialize;
use base64;
use bs58;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;

use crate::solana::client::SolanaClient;
use crate::models::pool::TokenInfo;

// SPL Token Program ID
pub const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

// Token Metadata Program ID - Metaplex
pub const TOKEN_METADATA_PROGRAM_ID: &str = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";

/// Find program accounts with specific offset and data
pub async fn find_program_accounts_by_data(
    client: &SolanaClient,
    program_id: &Pubkey,
    offset: usize,
    data: Vec<u8>,
) -> Result<Vec<(Pubkey, Account)>> {
    let memcmp = Memcmp::new(
        offset,
        MemcmpEncodedBytes::Base58(bs58::encode(&data).into_string()),
    );
    
    let _config = RpcProgramAccountsConfig {
        filters: Some(vec![RpcFilterType::Memcmp(memcmp)]),
        account_config: RpcAccountInfoConfig {
            encoding: None,
            data_slice: None,
            commitment: Some(CommitmentConfig::confirmed()),
            min_context_slot: None,
        },
        with_context: None,
    };
    
    let accounts = client.get_program_accounts(program_id)?;
    debug!("Found {} accounts for program {}", accounts.len(), program_id);
    
    Ok(accounts)
}

/// Get an account's data as a specific type by deserializing with borsh
pub fn get_account_data<T: BorshDeserialize>(account: &Account) -> Result<T> {
    T::try_from_slice(&account.data)
        .with_context(|| format!("Failed to deserialize account data"))
}

/// Parse pubkey from string with helpful error message
pub fn parse_pubkey(pubkey_str: &str) -> Result<Pubkey> {
    Pubkey::from_str(pubkey_str)
        .with_context(|| format!("Failed to parse pubkey: {}", pubkey_str))
}

/// Make a direct raw JSON RPC request to the Solana node
pub fn make_raw_rpc_request(
    rpc_client: &RpcClient,
    method_name: &'static str,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let request = RpcRequest::Custom { method: method_name };
    let response = rpc_client.send(request, params)
        .map_err(|e| anyhow!("RPC request '{}' failed: {}", method_name, e))?;
    
    Ok(response)
}

/// Calculate the program-derived address (PDA)
pub fn find_pda(
    seeds: &[&[u8]],
    program_id: &Pubkey,
) -> Result<(Pubkey, u8)> {
    let result = Pubkey::find_program_address(seeds, program_id);
    Ok(result)
}

/// Get multiple accounts and handle missing accounts
pub fn get_multiple_accounts_with_handling(
    client: &SolanaClient,
    pubkeys: &[Pubkey],
) -> Result<Vec<Option<Account>>> {
    let accounts = client.get_multiple_accounts(pubkeys)?;
    
    // Log warning for any missing accounts
    for (i, account) in accounts.iter().enumerate() {
        if account.is_none() {
            warn!("Account not found: {}", pubkeys[i]);
        }
    }
    
    Ok(accounts)
}

/// SPL Token account layout
#[derive(Debug, Clone)]
pub struct TokenAccountInfo {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub amount: u64,
    pub decimals: u8,
}

/// Simple token metadata structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMetadata {
    pub name: String,
    pub symbol: String,
    pub uri: String,
}

/// Get SPL Token account information
pub async fn get_token_account_info(client: &SolanaClient, account: &Pubkey) -> Result<TokenAccountInfo> {
    let account_data = client.get_account(account)?;
    
    // Parsing token account data
    // Standard SPL token account layout has mint at bytes 0-32
    if account_data.data.len() < 40 {
        return Err(anyhow!("Account data too short to be a token account"));
    }
    
    // Explicitly specify types for conversion
    let mint_bytes: [u8; 32] = account_data.data[0..32].try_into().unwrap_or([0u8; 32]);
    let mint = Pubkey::new_from_array(mint_bytes);
    let owner_bytes: [u8; 32] = account_data.data[32..64].try_into().unwrap_or([0u8; 32]);
    let owner = Pubkey::new_from_array(owner_bytes);
    
    // Amount is stored at bytes 64-72 as a u64
    let amount = u64::from_le_bytes([
        account_data.data[64], account_data.data[65], 
        account_data.data[66], account_data.data[67],
        account_data.data[68], account_data.data[69], 
        account_data.data[70], account_data.data[71],
    ]);
    
    // Decimals might not be directly stored in token accounts
    // We'll need to fetch it from the mint account
    let decimals = get_token_decimals(client, &mint).await?;
    
    Ok(TokenAccountInfo {
        mint,
        owner,
        amount,
        decimals,
    })
}

/// Get token decimals from a mint account
pub async fn get_token_decimals(client: &SolanaClient, mint: &Pubkey) -> Result<u8> {
    let mint_account = client.get_account(mint)?;
    
    // SPL token mint accounts store decimals at byte 44
    if mint_account.data.len() < 45 {
        return Err(anyhow!("Mint account data too short"));
    }
    
    Ok(mint_account.data[44])
}

/// Fetch token metadata from Metaplex
pub async fn get_token_metadata(client: &SolanaClient, mint: &Pubkey) -> Result<TokenMetadata> {
    let token_metadata_program_id = parse_pubkey(TOKEN_METADATA_PROGRAM_ID)?;
    
    // Calculate metadata account PDA
    let metadata_seeds = &[
        "metadata".as_bytes(),
        token_metadata_program_id.as_ref(),
        mint.as_ref(),
    ];
    
    let (metadata_address, _) = find_pda(metadata_seeds, &token_metadata_program_id)?;
    
    // Attempt to get the metadata account
    match client.get_account(&metadata_address) {
        Ok(account) => {
            
            // Skip the first 1 + 32 + 32 + 4 bytes (header + update auth + mint + name length)
            let mut pos = 1 + 32 + 32 + 4;
            
            // Extract name
            let name_len = u32::from_le_bytes([
                account.data[pos-4], account.data[pos-3], 
                account.data[pos-2], account.data[pos-1]
            ]) as usize;
            let name = String::from_utf8_lossy(&account.data[pos..pos+name_len]).to_string();
            pos += name_len + 4; // Move past name and symbol length
            
            // Extract symbol
            let symbol_len = u32::from_le_bytes([
                account.data[pos-4], account.data[pos-3], 
                account.data[pos-2], account.data[pos-1]
            ]) as usize;
            let symbol = String::from_utf8_lossy(&account.data[pos..pos+symbol_len]).to_string();
            pos += symbol_len + 4; // Move past symbol and uri length
            
            // Extract URI
            let uri_len = u32::from_le_bytes([
                account.data[pos-4], account.data[pos-3], 
                account.data[pos-2], account.data[pos-1]
            ]) as usize;
            let uri = String::from_utf8_lossy(&account.data[pos..pos+uri_len]).to_string();
            
            Ok(TokenMetadata { name, symbol, uri })
        },
        Err(_) => {
            // If we can't find metadata, return placeholder values
            Ok(TokenMetadata {
                name: format!("Token {}", mint.to_string()[0..8].to_string()),
                symbol: format!("TOKEN"),
                uri: String::new(),
            })
        }
    }
}

/// Helper to fetch token info for a mint
pub async fn fetch_token_info(client: &SolanaClient, mint: &Pubkey) -> Result<TokenInfo> {
    // Get decimals
    let decimals = match get_token_decimals(client, mint).await {
        Ok(d) => Some(d),
        Err(_) => None,
    };
    
    // Try to get metadata
    let metadata = match get_token_metadata(client, mint).await {
        Ok(meta) => meta,
        Err(_) => TokenMetadata {
            name: format!("Unknown {}", mint.to_string()[0..8].to_string()),
            symbol: "UNKNOWN".to_string(),
            uri: String::new(),
        }
    };
    
    Ok(TokenInfo {
        mint: *mint,
        name: Some(metadata.name),
        symbol: Some(metadata.symbol),
        decimals,
    })
} 