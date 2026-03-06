//! x402 Client SDK for making paid API requests.
//!
//! Handles the HTTP 402 payment flow automatically: request -> 402 -> sign -> retry.
//!
//! # Quick Example
//!
//! ```no_run
//! use alloy::signers::local::PrivateKeySigner;
//! use x402::client::{X402Client, TempoSchemeClient};
//!
//! # #[tokio::main]
//! # async fn main() {
//! let signer: PrivateKeySigner = "0xYOUR_KEY".parse().unwrap();
//! let client = X402Client::new(TempoSchemeClient::new(signer));
//!
//! let (resp, settlement) = client
//!     .fetch("https://api.example.com/data", reqwest::Method::GET)
//!     .await
//!     .unwrap();
//!
//! if let Some(s) = settlement {
//!     println!("Paid via tx: {}", s.transaction.as_deref().unwrap_or("pending"));
//! }
//! # }
//! ```

mod http_client;
mod scheme_client;

pub use http_client::{decode_payment, encode_payment, X402Client};
pub use scheme_client::TempoSchemeClient;
