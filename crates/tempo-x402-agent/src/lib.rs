//! Agent orchestration for x402 self-replicating instances.
//!
//! Provides Railway GraphQL API integration and clone orchestration logic.
//! Depends on `x402_identity` for the identity model.

pub mod clone;
pub mod railway;

pub use clone::{CloneConfig, CloneOrchestrator, CloneResult};
pub use railway::RailwayClient;
