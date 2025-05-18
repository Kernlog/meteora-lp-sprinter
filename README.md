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

Configuration can be set in the .env file or via environment variables:

- `RPC_URL`: Solana RPC endpoint URL
- `DATABASE_URL`: SQLite database file path
- `MAX_SOL_PER_POSITION`: Maximum SOL to allocate per liquidity position
- `POSITION_DURATION_SECONDS`: How long to hold a position (default: 180 seconds)
- `FEE_CLAIM_INTERVAL_SECONDS`: How often to claim fees (default: 60 seconds)

## License

MIT
