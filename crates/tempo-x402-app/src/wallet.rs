//! Wallet connection utilities supporting MetaMask, demo key, and embedded wallets.

#![allow(dead_code, deprecated)]

use crate::WalletState;
use wasm_bindgen::prelude::*;

pub use crate::WalletMode;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window, js_name = ethereum)]
    static ETHEREUM: JsValue;

    #[wasm_bindgen(catch, js_namespace = ["window", "ethereum"], js_name = request)]
    async fn ethereum_request(args: &JsValue) -> Result<JsValue, JsValue>;
}

/// Connect to browser wallet (MetaMask, etc.)
pub async fn connect_wallet() -> Result<WalletState, String> {
    if ETHEREUM.is_undefined() || ETHEREUM.is_null() {
        return Err("No Web3 wallet detected. Please install MetaMask.".to_string());
    }

    // Request accounts
    let request = js_sys::Object::new();
    js_sys::Reflect::set(&request, &"method".into(), &"eth_requestAccounts".into())
        .map_err(|e| format!("Failed to build request: {:?}", e))?;

    let accounts = ethereum_request(&request)
        .await
        .map_err(|e| format!("Wallet connection failed: {:?}", e))?;

    let accounts_array = js_sys::Array::from(&accounts);
    if accounts_array.length() == 0 {
        return Err("No accounts found".to_string());
    }

    let address = accounts_array.get(0).as_string().ok_or("Invalid address")?;

    // Get chain ID
    let chain_request = js_sys::Object::new();
    js_sys::Reflect::set(&chain_request, &"method".into(), &"eth_chainId".into())
        .map_err(|e| format!("Failed to build chain request: {:?}", e))?;

    let chain_id = ethereum_request(&chain_request)
        .await
        .map_err(|e| format!("Failed to get chain ID: {:?}", e))?
        .as_string();

    Ok(WalletState {
        connected: true,
        address: Some(address),
        chain_id,
        mode: WalletMode::MetaMask,
        private_key: None,
    })
}

/// Use the pre-funded demo key for testnet demos.
pub fn use_demo_key() -> Result<WalletState, String> {
    let signer = x402_wallet::WalletSigner::new(x402_wallet::DEMO_PRIVATE_KEY)?;
    Ok(WalletState {
        connected: true,
        address: Some(signer.address_string()),
        chain_id: Some(format!("0x{:x}", x402_wallet::TEMPO_CHAIN_ID)),
        mode: WalletMode::DemoKey,
        private_key: Some(x402_wallet::DEMO_PRIVATE_KEY.to_string()),
    })
}

/// Create a new embedded wallet with a random keypair.
///
/// After creation, the wallet needs to be funded via `tempo_fundAddress` RPC
/// and given facilitator token allowance.
pub fn create_embedded_wallet() -> Result<WalletState, String> {
    let key_hex = x402_wallet::generate_random_key();
    let signer = x402_wallet::WalletSigner::new(&key_hex)?;
    let address = signer.address_string();

    Ok(WalletState {
        connected: true,
        address: Some(address),
        chain_id: Some(format!("0x{:x}", x402_wallet::TEMPO_CHAIN_ID)),
        mode: WalletMode::Embedded,
        private_key: Some(key_hex),
    })
}

/// Fund an address via the Tempo `tempo_fundAddress` RPC method.
/// This is a WASM-compatible fetch call.
pub async fn fund_address(address: &str) -> Result<(), String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tempo_fundAddress",
        "params": [address],
        "id": 1
    });

    let resp = gloo_net::http::Request::post("https://rpc.moderato.tempo.xyz")
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&body).unwrap())
        .map_err(|e| format!("Failed to build request: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Fund request failed: {}", e))?;

    if !resp.ok() {
        let err = resp.text().await.unwrap_or_default();
        return Err(format!("Funding failed: {}", err));
    }

    Ok(())
}

/// Sign EIP-712 typed data using the connected MetaMask wallet.
pub async fn sign_typed_data(
    address: &str,
    domain: &serde_json::Value,
    types: &serde_json::Value,
    message: &serde_json::Value,
) -> Result<String, String> {
    if ETHEREUM.is_undefined() || ETHEREUM.is_null() {
        return Err("No wallet connected".to_string());
    }

    let typed_data = serde_json::json!({
        "types": types,
        "primaryType": "PaymentAuthorization",
        "domain": domain,
        "message": message
    });

    let typed_data_str = serde_json::to_string(&typed_data)
        .map_err(|e| format!("Failed to serialize typed data: {}", e))?;

    let request = js_sys::Object::new();
    js_sys::Reflect::set(&request, &"method".into(), &"eth_signTypedData_v4".into())
        .map_err(|e| format!("Failed to build request: {:?}", e))?;

    let params = js_sys::Array::new();
    params.push(&address.into());
    params.push(&typed_data_str.into());
    js_sys::Reflect::set(&request, &"params".into(), &params)
        .map_err(|e| format!("Failed to set params: {:?}", e))?;

    let signature = ethereum_request(&request)
        .await
        .map_err(|e| format!("Signing failed: {:?}", e))?
        .as_string()
        .ok_or("Invalid signature")?;

    Ok(signature)
}

/// Simple hex encoding (no external dep needed)
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
