#[cfg(feature = "telegram")]
use anyhow::{Result, Context};
#[cfg(feature = "telegram")]
use dotenv::dotenv;
#[cfg(feature = "telegram")]
use log::{info, error};
#[cfg(feature = "telegram")]
use std::io::{self, Write};
#[cfg(feature = "telegram")]
use tdlib::Tdlib;
#[cfg(feature = "telegram")]
use tdlib_types::*;

#[cfg(feature = "telegram")]
// Import the config module from the parent crate
use meteora_lp_sprinter::config;
#[cfg(feature = "telegram")]
use meteora_lp_sprinter::monitoring::telegram::TelegramConfig;

#[cfg(feature = "telegram")]
fn init_logger() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or("RUST_LOG", "info")
    );
}

#[cfg(feature = "telegram")]
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize environment variables
    dotenv().ok();
    
    // Initialize logging
    init_logger();
    
    info!("Telegram Authentication Tool");
    
    // Load configuration
    let app_config = config::load_config()?;
    let telegram_config: TelegramConfig = app_config.telegram.clone().context("Telegram configuration not found")?;
    
    info!("Telegram config loaded - connecting to API");
    
    // Initialize TDLib
    let tdlib_path = std::env::var("TDLIB_PATH").unwrap_or_else(|_| "tdlib".to_string());
    let mut client = Tdlib::new(tdlib_path);
    
    // Create the TDLib parameters
    let parameters = TdlibParameters {
        use_test_dc: false,
        database_directory: telegram_config.session_path.clone(),
        files_directory: telegram_config.session_path.clone(),
        api_id: telegram_config.api_id,
        api_hash: telegram_config.api_hash.clone(),
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
    
    // Process authentication
    let mut authenticated = false;
    
    while !authenticated {
        // Check authentication state
        let update = client.receive(10.0).await?;
        
        match update {
            TdType::UpdateAuthorizationState(state) => {
                match state.authorization_state {
                    TdType::AuthorizationStateWaitTdlibParameters(_) => {
                        info!("Sending TDLib parameters");
                        client.send(SetTdlibParameters {
                            parameters: parameters.clone(),
                            extra: String::new(),
                            ..Default::default()
                        }).await?;
                    },
                    TdType::AuthorizationStateWaitEncryptionKey(_) => {
                        info!("Sending encryption key");
                        client.send(CheckDatabaseEncryptionKey {
                            encryption_key: String::new(),
                            extra: String::new(),
                            ..Default::default()
                        }).await?;
                    },
                    TdType::AuthorizationStateWaitPhoneNumber(_) => {
                        info!("Sending phone number: {}", telegram_config.phone_number);
                        client.send(SetAuthenticationPhoneNumber {
                            phone_number: telegram_config.phone_number.clone(),
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
                    },
                    TdType::AuthorizationStateWaitCode(_) => {
                        print!("Enter the verification code sent to your device: ");
                        io::stdout().flush()?;
                        
                        let mut code = String::new();
                        io::stdin().read_line(&mut code)?;
                        let code = code.trim();
                        
                        info!("Sending verification code");
                        client.send(CheckAuthenticationCode {
                            code: code.to_string(),
                            extra: String::new(),
                            ..Default::default()
                        }).await?;
                    },
                    TdType::AuthorizationStateWaitPassword(_) => {
                        print!("Enter your 2FA password: ");
                        io::stdout().flush()?;
                        
                        let mut password = String::new();
                        io::stdin().read_line(&mut password)?;
                        let password = password.trim();
                        
                        info!("Sending 2FA password");
                        client.send(CheckAuthenticationPassword {
                            password: password.to_string(),
                            extra: String::new(),
                            ..Default::default()
                        }).await?;
                    },
                    TdType::AuthorizationStateReady(_) => {
                        info!("✅ Successfully authenticated with Telegram!");
                        authenticated = true;
                    },
                    TdType::AuthorizationStateLoggingOut(_) => {
                        info!("Logging out...");
                    },
                    TdType::AuthorizationStateClosing(_) => {
                        info!("Closing...");
                    },
                    TdType::AuthorizationStateClosed(_) => {
                        error!("TDLib instance closed");
                        return Err(anyhow::anyhow!("TDLib instance closed"));
                    },
                    _ => {
                        error!("Unexpected authorization state: {:?}", state.authorization_state);
                    }
                }
            },
            _ => {}
        }
    }
    
    // Get and display user info
    let me = client.send(GetMe {
        extra: String::new(),
        ..Default::default()
    }).await?;
    
    match me {
        TdType::User(user) => {
            info!("Logged in as: {} {} (@{})", 
                user.first_name, 
                user.last_name.unwrap_or_default(), 
                user.username.unwrap_or_default());
        },
        _ => {
            error!("Failed to get user info");
        }
    }
    
    info!("✅ Authentication complete! You can now run the main application.");
    Ok(())
}

#[cfg(not(feature = "telegram"))]
fn main() {
    println!("Telegram support is not enabled. Compile with --features telegram to use this binary.");
} 