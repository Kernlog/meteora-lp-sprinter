use anyhow::{Result, anyhow, Context};
use log::{info, warn, error, debug};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tdlib::Tdlib;
use tdlib_types::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::models::pool::Pool;
use crate::monitoring::pool_monitor::PoolMonitor;

/// Config for the Telegram monitor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    /// API ID from https://my.telegram.org
    pub api_id: i32,
    /// API Hash from https://my.telegram.org
    pub api_hash: String,
    /// Phone number in international format
    pub phone_number: String,
    /// List of channel usernames to monitor
    pub channels: Vec<String>,
    /// Path to store the Telegram session files
    pub session_path: String,
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            api_id: 0,
            api_hash: String::new(),
            phone_number: String::new(),
            channels: vec![
                "fluxbot_pool_sniper".to_string(),
                "BONKbotNewTokenAlerts".to_string(),
            ],
            session_path: "telegram_session".to_string(),
        }
    }
}

/// Structure for extracting pool information from messages
struct PoolExtractor {
    /// Regex patterns for identifying Meteora pools
    pool_patterns: Vec<Regex>,
    /// Memory of recently seen pools to avoid duplicates
    recent_pools: HashMap<String, Instant>,
}

impl PoolExtractor {
    fn new() -> Self {
        let pool_patterns = vec![
            // FluxBot pattern
            Regex::new(r"Pool Address: ([a-zA-Z0-9]{32,44})").unwrap(),
            // BONK Bot pattern
            Regex::new(r"Pool: ([a-zA-Z0-9]{32,44})").unwrap(),
            // Generic pattern for Solana addresses
            Regex::new(r"(?i)meteora pool[:\s]+([a-zA-Z0-9]{32,44})").unwrap(),
            // Alternative format with "LP Pool"
            Regex::new(r"(?i)lp pool[:\s]+([a-zA-Z0-9]{32,44})").unwrap(),
        ];
        
        Self {
            pool_patterns,
            recent_pools: HashMap::new(),
        }
    }
    
    /// Extract pool addresses from a message
    fn extract_pools(&mut self, message: &str) -> Vec<String> {
        let mut pools = Vec::new();
        
        for pattern in &self.pool_patterns {
            for cap in pattern.captures_iter(message) {
                if let Some(pool_address) = cap.get(1) {
                    let pool_address = pool_address.as_str().to_string();
                    
                    // Check if we've seen this pool recently (last 30 minutes)
                    let now = Instant::now();
                    let is_new = self.recent_pools
                        .get(&pool_address)
                        .map_or(true, |last_seen| now.duration_since(*last_seen) > Duration::from_secs(1800));
                    
                    if is_new {
                        pools.push(pool_address.clone());
                        self.recent_pools.insert(pool_address, now);
                    }
                }
            }
        }
        
        pools
    }
}

/// Telegram client that connects to channels and monitors for new pools
pub struct TelegramMonitor {
    /// TDLib client
    client: Arc<Mutex<Tdlib>>,
    /// Configuration
    config: TelegramConfig,
    /// Pool extractor
    extractor: Arc<Mutex<PoolExtractor>>,
    /// Channel for sending discovered pools
    pool_sender: Option<mpsc::Sender<Pool>>,
    /// Map of channel usernames to their chat IDs
    channel_ids: Arc<Mutex<HashMap<String, i64>>>,
    /// Running flag
    running: Arc<Mutex<bool>>,
}

impl TelegramMonitor {
    /// Create a new Telegram monitor
    pub fn new(config: TelegramConfig) -> Result<Self> {
        let tdlib_path = std::env::var("TDLIB_PATH").unwrap_or_else(|_| "tdlib".to_string());
        let client = Tdlib::new(tdlib_path);
        
        Ok(Self {
            client: Arc::new(Mutex::new(client)),
            config,
            extractor: Arc::new(Mutex::new(PoolExtractor::new())),
            pool_sender: None,
            channel_ids: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(Mutex::new(false)),
        })
    }
    
    /// Initialize TDLib
    async fn initialize(&self) -> Result<()> {
        let mut client = self.client.lock().await;
        
        // Create the TDLib parameters
        let parameters = TdlibParameters {
            use_test_dc: false,
            database_directory: self.config.session_path.clone(),
            files_directory: self.config.session_path.clone(),
            api_id: self.config.api_id,
            api_hash: self.config.api_hash.clone(),
            system_language_code: "en".to_string(),
            device_model: "Desktop".to_string(),
            application_version: "1.0".to_string(),
            enable_storage_optimizer: true,
            use_message_database: true,
            ..Default::default()
        };
        
        // Send the TDLib parameters
        client.send(SetTdlibParameters {
            parameters: parameters.clone(),
            extra: String::new(),
            ..Default::default()
        }).await?;
        
        // Check authentication state
        let auth_state = client.send(GetAuthorizationState {
            extra: String::new(),
            ..Default::default()
        }).await?;
        
        match auth_state {
            TdType::AuthorizationStateWaitPhoneNumber(_) => {
                // Send phone number
                client.send(SetAuthenticationPhoneNumber {
                    phone_number: self.config.phone_number.clone(),
                    settings: PhoneNumberAuthenticationSettings {
                        allow_flash_call: false,
                        allow_missed_call: false,
                        is_current_phone_number: true,
                        allow_sms_retriever_api: false,
                        ..Default::default()
                    },
                    extra: String::new(),
                    ..Default::default()
                }).await?;
                
                info!("Sent phone number, please check your device for verification code");
                
                // Wait for the verification code
                loop {
                    let update = client.receive(10.0).await?;
                    
                    match update {
                        TdType::UpdateAuthorizationState(state) => {
                            match state.authorization_state {
                                TdType::AuthorizationStateWaitCode(_) => {
                                    info!("Please enter the verification code sent to your device");
                                    return Err(anyhow!("Manual verification required: Please run the telegram_auth binary and enter the code"));
                                },
                                TdType::AuthorizationStateReady(_) => {
                                    info!("Successfully authenticated with Telegram");
                                    break;
                                },
                                _ => continue,
                            }
                        },
                        _ => continue,
                    }
                }
            },
            TdType::AuthorizationStateReady(_) => {
                info!("Already authenticated with Telegram");
            },
            _ => {
                return Err(anyhow!("Unexpected authentication state: {:?}", auth_state));
            }
        }
        
        Ok(())
    }
    
    /// Resolve channel usernames to chat IDs
    async fn resolve_channels(&self) -> Result<()> {
        let client = self.client.lock().await;
        let mut channel_ids = self.channel_ids.lock().await;
        
        for channel in &self.config.channels {
            // Resolve the channel username
            let channel_chat = client.send(SearchPublicChat {
                username: channel.clone(),
                extra: String::new(),
                ..Default::default()
            }).await?;
            
            match channel_chat {
                TdType::Chat(chat) => {
                    info!("Resolved channel {} to chat ID {}", channel, chat.id);
                    channel_ids.insert(channel.clone(), chat.id);
                },
                _ => {
                    return Err(anyhow!("Failed to resolve channel: {}", channel));
                }
            }
        }
        
        Ok(())
    }
    
    /// Process a new message
    async fn process_message(&self, chat_id: i64, message: String) -> Result<()> {
        debug!("Processing message from chat {}: {}", chat_id, message);
        
        // Extract pools from the message
        let mut extractor = self.extractor.lock().await;
        let pool_addresses = extractor.extract_pools(&message);
        
        if !pool_addresses.is_empty() {
            info!("Found {} potential pools in message", pool_addresses.len());
            
            // Check if we have a channel to send the pool info
            if let Some(pool_sender) = &self.pool_sender {
                let client = self.client.lock().await;
                
                for address_str in pool_addresses {
                    // Try to parse the pool address
                    match address_str.parse::<solana_sdk::pubkey::Pubkey>() {
                        Ok(address) => {
                            info!("Successfully parsed pool address: {}", address);
                            
                            // Create a new Pool object
                            // For now, we just know the address, and will need to fetch token info later
                            let pool = crate::models::pool::Pool {
                                address,
                                token_a: crate::models::pool::TokenInfo {
                                    mint: solana_sdk::pubkey::Pubkey::default(), // Will be fetched later
                                    name: None,
                                    symbol: None,
                                    decimals: None,
                                },
                                token_b: crate::models::pool::TokenInfo {
                                    mint: solana_sdk::pubkey::Pubkey::default(), // Will be fetched later
                                    name: None,
                                    symbol: None,
                                    decimals: None,
                                },
                                discovered_at: chrono::Utc::now(),
                                analyzed: false,
                                score: None,
                            };
                            
                            // Send the pool to the channel
                            if let Err(e) = pool_sender.send(pool.clone()).await {
                                error!("Failed to send pool to channel: {}", e);
                            } else {
                                info!("Sent pool {} to channel for processing", address);
                            }
                        },
                        Err(e) => {
                            warn!("Failed to parse pool address {}: {}", address_str, e);
                        }
                    }
                }
            } else {
                warn!("Found pools but no channel is configured to receive them");
            }
        }
        
        Ok(())
    }
    
    /// Message polling loop
    async fn poll_messages(&self) -> Result<()> {
        let client = self.client.lock().await;
        let channel_ids = self.channel_ids.lock().await;
        
        // Create a vector of chat IDs we're monitoring
        let chat_ids: Vec<i64> = channel_ids.values().cloned().collect();
        
        // Get latest messages for each chat
        for &chat_id in &chat_ids {
            // Get chat history
            let messages = client.send(GetChatHistory {
                chat_id,
                from_message_id: 0,
                offset: 0,
                limit: 10,
                only_local: false,
                extra: String::new(),
                ..Default::default()
            }).await?;
            
            match messages {
                TdType::Messages(msgs) => {
                    for message in msgs.messages {
                        if let TdType::Message(msg) = message {
                            if let TdType::MessageContent(content) = msg.content {
                                if let TdType::MessageText(text) = content {
                                    self.process_message(chat_id, text.text.text).await?;
                                }
                            }
                        }
                    }
                },
                _ => {
                    warn!("Unexpected response when getting chat history: {:?}", messages);
                }
            }
        }
        
        Ok(())
    }
    
    /// Listen for new updates from Telegram
    async fn listen_updates(&self) -> Result<()> {
        let client_arc = self.client.clone();
        let running_arc = self.running.clone();
        let channel_ids_arc = self.channel_ids.clone();
        let extractor_arc = self.extractor.clone();
        let sender_arc = self.pool_sender.clone();
        
        tokio::spawn(async move {
            loop {
                // Check if we should stop
                if !*running_arc.lock().await {
                    break;
                }
                
                // Receive an update from TDLib
                let client = client_arc.lock().await;
                let update = match client.receive(1.0).await {
                    Ok(update) => update,
                    Err(e) => {
                        error!("Error receiving update: {}", e);
                        continue;
                    }
                };
                
                // Process the update
                match update {
                    TdType::UpdateNewMessage(update) => {
                        if let TdType::Message(msg) = update.message {
                            // Check if this message is from a channel we're monitoring
                            let channel_ids = channel_ids_arc.lock().await;
                            if channel_ids.values().any(|&id| id == msg.chat_id) {
                                if let TdType::MessageContent(content) = msg.content {
                                    if let TdType::MessageText(text) = content {
                                        // Extract pools from the message
                                        let mut extractor = extractor_arc.lock().await;
                                        let pools = extractor.extract_pools(&text.text.text);
                                        
                                        if !pools.is_empty() {
                                            debug!("Found {} pool addresses in message", pools.len());
                                            
                                            // Send pools to the channel if we have a sender
                                            if let Some(sender) = &sender_arc {
                                                for pool_address in pools {
                                                    info!("Found new pool: {}", pool_address);
                                                    
                                                    // Create a Pool object
                                                    let pool = Pool {
                                                        address: pool_address,
                                                        token_a: "".to_string(),
                                                        token_b: "".to_string(),
                                                        source: "telegram".to_string(),
                                                        discovery_time: chrono::Utc::now(),
                                                    };
                                                    
                                                    // Send the pool to the channel
                                                    if let Err(e) = sender.send(pool.clone()).await {
                                                        error!("Failed to send pool to channel: {}", e);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    _ => {
                        // Ignore other updates
                    }
                }
            }
        });
        
        Ok(())
    }
}

#[async_trait::async_trait]
impl PoolMonitor for TelegramMonitor {
    /// Start monitoring Telegram channels for new pools
    async fn start_monitoring(&mut self, tx: mpsc::Sender<Pool>) -> Result<()> {
        info!("Starting Telegram monitor");
        
        // Set pool sender
        self.pool_sender = Some(tx);
        
        // Initialize TDLib
        if let Err(e) = self.initialize().await {
            error!("Failed to initialize Telegram client: {}", e);
            info!("Please run the telegram_auth binary to authenticate with Telegram");
            return Err(e);
        }
        
        // Resolve channels
        if let Err(e) = self.resolve_channels().await {
            error!("Failed to resolve channels: {}", e);
            return Err(e);
        }
        
        // Mark as running
        *self.running.lock().await = true;
        
        // Start listening for updates
        self.listen_updates().await?;
        
        // Poll for initial messages
        self.poll_messages().await?;
        
        info!("Telegram monitor started successfully");
        
        Ok(())
    }
    
    /// Stop monitoring Telegram channels
    async fn stop(&mut self) -> Result<()> {
        info!("Stopping Telegram monitor");
        
        // Mark as not running
        *self.running.lock().await = false;
        
        // Clear pool sender
        self.pool_sender = None;
        
        Ok(())
    }
} 