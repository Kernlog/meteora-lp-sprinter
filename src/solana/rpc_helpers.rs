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

use crate::solana::client::SolanaClient;

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