use solana_sdk::pubkey::Pubkey;

/// Format a pubkey for display (shortened)
pub fn format_pubkey(pubkey: &Pubkey) -> String {
    let pubkey_str = pubkey.to_string();
    let len = pubkey_str.len();
    format!("{}...{}", &pubkey_str[0..4], &pubkey_str[len-4..len])
} 