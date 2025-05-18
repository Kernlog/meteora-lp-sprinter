# Meteora LP Sprinter

A Rust-based bot for automatically providing liquidity to new Solana memecoin pools on Meteora DAMM v2, harvesting fees, and exiting rapidly to maximize returns.

## Features

- Monitors for new Meteora DAMM v2 memecoin pools
- Analyzes pools for profitability potential
- Provides liquidity to promising pools
- Claims fees at optimal intervals
- Exits positions within configurable timeframes (default: 3 minutes)
- Tracks performance and profitability

## Setup

1. Clone the repository
2. Copy `.env.example` to `.env` and update configuration values
3. Create your Solana keypair file (or use an existing one)
4. Install Rust if not already installed (https://rustup.rs/)
5. Build the project: `cargo build --release`

## Usage

Run the bot with default settings:

```bash
cargo run --release
```

For the Telegram monitoring feature, use:

```bash
cargo run --release --features telegram
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

### JSON Configuration

You can also use a JSON configuration file. The application will look for the config file in the following locations:

1. Path specified in the `CONFIG_FILE` environment variable
2. `./config.json` in the current directory
3. `~/.config/meteora-lp-sprinter/config.json`

Example config.json:
```json
{
  "rpc_url": "https://api.mainnet-beta.solana.com",
  "keypair_path": "/path/to/your/keypair.json",
  "max_sol_per_position": 0.1,
  "position_duration_seconds": 180,
  "fee_claim_interval_seconds": 60,
  "database_path": "meteora_sprinter.db",
  "debug_logging": false
}
```

## License

MIT
