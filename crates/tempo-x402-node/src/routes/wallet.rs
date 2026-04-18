//! Wallet setup endpoint — fund and approve for x402 payments.
//!
//! `POST /wallet/setup` accepts a private key, calls the Tempo faucet
//! to fund the address, and submits an ERC-20 `approve()` transaction
//! so the embedded facilitator can settle payments on behalf of the wallet.

use actix_web::{web, HttpResponse};
use alloy::primitives::{Address, U256};
use alloy::signers::local::PrivateKeySigner;
use x402_gateway::error::GatewayError;

use crate::state::NodeState;

#[derive(serde::Deserialize)]
pub struct SetupRequest {
    /// Wallet private key (hex, with or without 0x prefix)
    pub private_key: String,
}

/// POST /wallet/setup — fund wallet via faucet + approve facilitator for pathUSD
pub async fn setup_wallet(
    body: web::Json<SetupRequest>,
    node: web::Data<NodeState>,
) -> Result<HttpResponse, GatewayError> {
    // Parse the private key
    let signer: PrivateKeySigner = body
        .private_key
        .parse()
        .map_err(|e| GatewayError::Internal(format!("invalid private key: {e}")))?;
    let wallet_address = signer.address();

    // Get the facilitator address from the embedded facilitator state
    let facilitator_address: Address = node
        .gateway
        .facilitator
        .as_ref()
        .map(|f| f.facilitator.facilitator_address())
        .ok_or_else(|| GatewayError::Internal("no embedded facilitator configured".to_string()))?;

    let rpc_url = node.gateway.config.rpc_url.clone();
    let token = x402::constants::DEFAULT_TOKEN;

    // 1. Fund via faucet (best-effort)
    let funded = match x402_identity::request_faucet_funds(&rpc_url, wallet_address).await {
        Ok(()) => {
            tracing::info!(address = %wallet_address, "Wallet funded via faucet");
            true
        }
        Err(e) => {
            tracing::warn!(address = %wallet_address, error = %e, "Faucet funding failed");
            false
        }
    };

    // 2. Check current allowance
    let provider = alloy::providers::ProviderBuilder::new()
        .wallet(alloy::network::EthereumWallet::from(signer))
        .connect_http(
            rpc_url
                .parse()
                .map_err(|e| GatewayError::Internal(format!("invalid RPC URL: {e}")))?,
        );

    let current_allowance =
        x402::tip20::allowance(&provider, token, wallet_address, facilitator_address)
            .await
            .unwrap_or(U256::ZERO);

    // If allowance is already sufficient (>= 1B pathUSD = 1e15 units), skip approve
    let needs_approve = current_allowance < U256::from(1_000_000_000_000_000u64);

    let approved = if needs_approve {
        // Approve MAX for convenience (testnet only)
        match x402::tip20::approve(&provider, token, facilitator_address, U256::MAX).await {
            Ok(tx_hash) => {
                tracing::info!(
                    address = %wallet_address,
                    facilitator = %facilitator_address,
                    tx = %tx_hash,
                    "Facilitator approved for pathUSD"
                );
                true
            }
            Err(e) => {
                tracing::warn!(
                    address = %wallet_address,
                    error = %e,
                    "Approval failed (wallet may lack gas — retry after faucet)"
                );
                false
            }
        }
    } else {
        true // already approved
    };

    // 3. Check pathUSD balance
    let balance = x402::tip20::balance_of(&provider, token, wallet_address)
        .await
        .unwrap_or(U256::ZERO);

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "address": format!("{:#x}", wallet_address),
        "facilitator": format!("{:#x}", facilitator_address),
        "funded": funded,
        "approved": approved,
        "balance": balance.to_string(),
        "allowance": if needs_approve { "max".to_string() } else { current_allowance.to_string() },
    })))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/wallet/setup", web::post().to(setup_wallet));
}
