# tempo-x402-client

Client SDK for making paid API requests using the x402 payment protocol on Tempo blockchain.

## Quick Start

```rust
use alloy::signers::local::PrivateKeySigner;
use x402_client::{X402Client, TempoSchemeClient};

#[tokio::main]
async fn main() {
    let signer: PrivateKeySigner = "0xYOUR_PRIVATE_KEY".parse().unwrap();
    let client = X402Client::new(TempoSchemeClient::new(signer));

    let (response, settlement) = client
        .fetch("https://api.example.com/paid-endpoint", reqwest::Method::GET)
        .await
        .unwrap();

    if let Some(s) = settlement {
        println!("Paid via tx: {}", s.transaction);
    }
}
```

## How It Works

1. Make a request to an x402-protected endpoint
2. Receive a 402 response with payment requirements
3. Client automatically signs an EIP-712 payment authorization
4. Retry with `PAYMENT-SIGNATURE` header
5. Receive content + settlement confirmation in `PAYMENT-RESPONSE` header

## CLI Usage

```bash
# Set environment variables
export EVM_PRIVATE_KEY=0x...
export RESOURCE_SERVER_URL=https://x402-server.example.com

# Run the client
cargo run --bin x402-client
```

## Features

- Automatic 402 handling with payment retry
- EIP-712 signature generation
- Tempo blockchain support (chain ID 42431)
- pathUSD token transfers (TIP-20)
- Custom chain configuration support

## License

MIT
