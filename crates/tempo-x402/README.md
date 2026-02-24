# tempo-x402

Core library for the x402 payment protocol on the Tempo blockchain.

## Installation

```toml
[dependencies]
tempo-x402 = "0.6"
```

The crate is published as `tempo-x402` but the library name is `x402`:

```rust
use x402::scheme::SchemeServer;
use x402::scheme_server::TempoSchemeServer;
```

## Features

- **HTTP Client**: Automatic 402 handling with payment signing
- **EIP-712 Signing**: Payment authorization signatures
- **TIP-20 Utilities**: Balance, allowance, transfer functions
- **Nonce Store**: Replay protection (in-memory or SQLite)
- **HMAC Auth**: Server-facilitator authentication

## Usage

```rust
use alloy::signers::local::PrivateKeySigner;
use x402_client::{TempoSchemeClient, X402Client};

#[tokio::main]
async fn main() {
    let signer: PrivateKeySigner = "0xYOUR_KEY".parse().unwrap();
    let client = X402Client::new(TempoSchemeClient::new(signer));

    let (response, settlement) = client
        .fetch("https://api.example.com/paid", reqwest::Method::GET)
        .await
        .unwrap();

    println!("{}", response.text().await.unwrap());
}
```

## Binaries

- `x402-client`: CLI for making paid requests
- `x402-approve`: CLI for approving the facilitator

## Documentation

See the [full documentation](https://docs.rs/tempo-x402) or [llms.txt](https://github.com/compusophy/tempo-x402/blob/main/llms.txt).

## License

MIT
