//! # tempo-x402-cartridge
//!
//! WASM cartridge runtime for x402 nodes. Sandboxed app execution with payment rails.
//!
//! Cartridges are precompiled `.wasm` binaries that run inside a wasmtime sandbox.
//! The host exposes a minimal ABI: HTTP client, KV store, logging, payment info.
//! Each cartridge handles HTTP requests and returns responses — instant deployment,
//! no node restart required.
//!
//! ## Quick Start
//!
//! ```ignore
//! use x402_cartridge::{CartridgeEngine, CartridgeRequest};
//!
//! let engine = CartridgeEngine::new("/data/cartridges")?;
//! engine.load_module("hello", Path::new("/data/cartridges/hello/bin/hello.wasm"))?;
//!
//! let request = CartridgeRequest {
//!     method: "GET".into(),
//!     path: "/".into(),
//!     body: String::new(),
//!     headers: Default::default(),
//!     payment: None,
//! };
//!
//! let result = engine.execute("hello", &request, Default::default(), 30)?;
//! println!("Status: {}, Body: {}", result.status, result.body);
//! ```

pub mod abi;
pub mod compiler;
pub mod engine;
pub mod error;
pub mod manifest;

pub use engine::CartridgeEngine;
pub use error::CartridgeError;
pub use manifest::{
    CartridgeManifest, CartridgeRequest, CartridgeResult, PaymentContext, ABI_VERSION,
};
