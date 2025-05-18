use anyhow::{Result, Context};
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::transaction::Transaction;
use solana_sdk::message::Message;
use solana_sdk::instruction::Instruction;
use std::path::Path;
use std::fs::File;
use std::io::Read;
use bs58;

/// Handles wallet operations and transaction signing
pub struct Wallet {
    keypair: Keypair,
}

impl Wallet {
    /// Create a new wallet with a random keypair
    pub fn new() -> Self {
        Self { keypair: Keypair::new() }
    }

    /// Load a wallet from a keypair file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(&path)
            .with_context(|| format!("Failed to open keypair file at {:?}", path.as_ref()))?;
            
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .with_context(|| format!("Failed to read keypair file at {:?}", path.as_ref()))?;
            
        // Try to deserialize as a JSON string containing byte array
        match serde_json::from_slice::<Vec<u8>>(&bytes) {
            Ok(keypair_bytes) if keypair_bytes.len() == 64 => {
                let mut array = [0u8; 64];
                array.copy_from_slice(&keypair_bytes);
                return Ok(Self { keypair: Keypair::from_bytes(&array)? });
            },
            _ => {}
        }
        
        // Try to deserialize as a base58 encoded keypair
        let bytes_str = String::from_utf8_lossy(&bytes).trim().to_string();
        match bs58::decode(&bytes_str).into_vec() {
            Ok(keypair_bytes) if keypair_bytes.len() == 64 => {
                let mut array = [0u8; 64];
                array.copy_from_slice(&keypair_bytes);
                return Ok(Self { keypair: Keypair::from_bytes(&array)? });
            },
            _ => {}
        }
        
        // If we get here, we couldn't parse the keypair
        Err(anyhow::anyhow!("Failed to parse keypair file"))
    }
    
    /// Create wallet from a seed phrase (mnemonic)
    pub fn from_seed_phrase(mnemonic: &str, passphrase: Option<&str>) -> Result<Self> {
        // This is a simplified implementation
        // In a production environment, use a proper HD wallet derivation
        let seed = format!("{}{}", mnemonic, passphrase.unwrap_or(""));
        let hash = solana_sdk::hash::hash(seed.as_bytes());
        let bytes = hash.to_bytes();
        
        // Use first 32 bytes as seed for keypair
        let mut keypair_bytes = [0u8; 64];
        keypair_bytes[..32].copy_from_slice(&bytes);
        
        let keypair = Keypair::from_bytes(&keypair_bytes)?;
        Ok(Self { keypair })
    }
    
    /// Get the public key of the wallet
    pub fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }
    
    /// Sign a transaction
    pub fn sign_transaction(&self, message: Message) -> Result<Transaction> {
        let mut transaction = Transaction::new_unsigned(message);
        transaction.sign(&[&self.keypair], transaction.message.recent_blockhash);
        Ok(transaction)
    }
    
    /// Create and sign a transaction in one step
    pub fn create_and_sign_transaction(
        &self,
        instructions: Vec<Instruction>,
        recent_blockhash: solana_sdk::hash::Hash,
        fee_payer: Option<Pubkey>,
    ) -> Result<Transaction> {
        let fee_payer = fee_payer.unwrap_or_else(|| self.pubkey());
        
        let message = Message::new_with_blockhash(
            &instructions,
            Some(&fee_payer),
            &recent_blockhash,
        );
        
        self.sign_transaction(message)
    }
    
    /// Sign a buffer of data
    pub fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>> {
        let signature = self.keypair.sign_message(message);
        Ok(signature.as_ref().to_vec())
    }
    
    /// Export keypair as bytes (for backup purposes)
    pub fn export_keypair(&self) -> Vec<u8> {
        self.keypair.to_bytes().to_vec()
    }
    
    /// Get the underlying keypair
    pub fn keypair(&self) -> &Keypair {
        &self.keypair
    }
}

impl AsRef<Keypair> for Wallet {
    fn as_ref(&self) -> &Keypair {
        &self.keypair
    }
} 