# tempo-x402

Pay-per-request APIs on the Tempo blockchain.

**[Live Demo](https://tempo-x402-demo.vercel.app)** · **[Documentation](https://tempo-x402-demo.vercel.app/docs)** · **[crates.io](https://crates.io/crates/tempo-x402)**

## Install

```bash
cargo add tempo-x402
```

## Usage

```rust
use alloy::signers::local::PrivateKeySigner;
use x402::{TempoSchemeClient, X402Client};

#[tokio::main]
async fn main() {
    let signer: PrivateKeySigner = "0xYOUR_KEY".parse().unwrap();
    let client = X402Client::new(TempoSchemeClient::new(signer));

    let (response, settlement) = client
        .fetch("https://api.example.com/paid-endpoint", reqwest::Method::GET)
        .await
        .unwrap();
}
```

## License

MIT
