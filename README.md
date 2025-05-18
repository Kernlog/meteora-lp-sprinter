# Meteora LP Sprinter

A Rust-based bot for automatically providing liquidity to new Solana memecoin pools on Meteora DAMM v2, harvesting fees, and exiting rapidly to maximize returns.

## Features

- Monitors for new Meteora DAMM v2 memecoin pools
- Analyzes pools for profitability potential
- Provides liquidity to promising pools
- Claims fees at optimal intervals
- Exits positions within configurable timeframes (default: 3 minutes)
- Tracks performance and profitability
- Telegram monitoring for real-time pool discovery

## Setup

1. Clone the repository
2. Copy `.env.example` to `.env` and update configuration values
3. Create your Solana keypair file (or use an existing one)
4. Install Rust if not already installed (https://rustup.rs/)
5. Build the project: `cargo build --release`
6. For Telegram monitoring, install TDLib (https://tdlib.github.io/td/build.html)

## Usage

Run the bot with default settings:

```bash
cargo run --release
```

For first-time Telegram setup, you'll need to authenticate:

```bash
cargo run --release --bin telegram_auth
```

## Configuration

Configuration can be set using one of the following methods (in order of precedence):

1. **Environment variables** (highest priority)
2. **JSON configuration file**
3. **Default values** (lowest priority)

### Environment Variables

- `RPC_URL`: Solana RPC endpoint URL
- `KEYPAIR_PATH`: Path to your Solana keypair file
- `MAX_SOL_PER_POSITION`: Maximum SOL to allocate per liquidity position
- `POSITION_DURATION_SECONDS`: How long to hold a position (default: 180 seconds)
- `FEE_CLAIM_INTERVAL_SECONDS`: How often to claim fees (default: 60 seconds)
- `DATABASE_URL`: SQLite database file path
- `DEBUG_LOGGING`: Enable debug logging (true/false)
- `CONFIG_FILE`: Path to custom JSON config file

#### Telegram Configuration
- `TELEGRAM_API_ID`: API ID from https://my.telegram.org/apps
- `TELEGRAM_API_HASH`: API Hash from https://my.telegram.org/apps
- `TELEGRAM_PHONE`: Phone number in international format
- `TELEGRAM_SESSION_PATH`: Directory to store Telegram session data
- `TELEGRAM_CHANNELS`: Comma-separated list of channels to monitor
- `TDLIB_PATH`: Path to TDLib library (optional)

### JSON Configuration

You can also use a JSON configuration file. The application will look for the config file in the following locations:

1. Path specified in the `CONFIG_FILE` environment variable
2. `./config.json` in the current directory
3. `~/.config/meteora-lp-sprinter/config.json`

Example config.json:
```json
{
  "rpc_url": "https://api.mainnet-beta.solana.com",
  "keypair_path": "wallet-keypair.json",
  "max_sol_per_position": 0.1,
  "position_duration_seconds": 180,
  "fee_claim_interval_seconds": 60,
  "database_path": "meteora_sprinter.db",
  "debug_logging": true,
  "telegram": {
    "api_id": 12345,
    "api_hash": "abcdef1234567890abcdef1234567890",
    "phone_number": "+12345678900",
    "session_path": "telegram_session",
    "channels": [
      "fluxbot_pool_sniper",
      "BONKbotNewTokenAlerts"
    ]
  }
}
```

## Telegram Monitoring

The bot can monitor Telegram channels to discover new Meteora pools. It currently monitors:

- `@fluxbot_pool_sniper` - Fluxbeam's pool announcements
- `@BONKbotNewTokenAlerts` - BONK bot's new token alerts

To use this feature:
1. Create a Telegram API application at https://my.telegram.org/apps
2. Get your API ID and API Hash
3. Configure them in your `.env` file or config.json
4. Run the authentication tool: `cargo run --release --bin telegram_auth`
5. Enter the verification code sent to your Telegram app

After authentication, the main bot will automatically monitor configured Telegram channels for new pool announcements.

## License

MIT
