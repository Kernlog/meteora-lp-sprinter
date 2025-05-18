use anyhow::Result;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() -> Result<()> {
    // Create a random keypair
    let keypair = Keypair::new();
    
    // Get the bytes of the keypair
    let keypair_bytes = keypair.to_bytes();
    
    // Convert bytes to base58 for easier copy/paste if needed
    let keypair_bs58 = bs58::encode(&keypair_bytes).into_string();
    
    // Path to save the keypair
    let path = PathBuf::from("wallet-keypair.json");
    
    // Write the keypair bytes as JSON array
    let mut file = File::create(&path)?;
    file.write_all(serde_json::to_string(&keypair_bytes.to_vec())?.as_bytes())?;
    
    println!("Generated new random keypair:");
    println!("Path: {:?}", path);
    println!("Pubkey: {}", keypair.pubkey());
    println!("Base58: {}", keypair_bs58);
    
    Ok(())
} 