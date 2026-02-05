//! Wallet connection utilities using browser's Web3 provider (MetaMask, etc.)

#![allow(dead_code, deprecated)]

use crate::WalletState;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window, js_name = ethereum)]
    static ETHEREUM: JsValue;

    #[wasm_bindgen(catch, js_namespace = ["window", "ethereum"], js_name = request)]
    async fn ethereum_request(args: &JsValue) -> Result<JsValue, JsValue>;
}

/// Connect to browser wallet (MetaMask, etc.)
pub async fn connect_wallet() -> Result<WalletState, String> {
    // Check if ethereum provider exists
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
    })
}

/// Sign a message using the connected wallet
pub async fn sign_typed_data(
    address: &str,
    domain: &serde_json::Value,
    types: &serde_json::Value,
    message: &serde_json::Value,
) -> Result<String, String> {
    if ETHEREUM.is_undefined() || ETHEREUM.is_null() {
        return Err("No wallet connected".to_string());
    }

    // Build EIP-712 typed data
    let typed_data = serde_json::json!({
        "types": types,
        "primaryType": "PaymentAuthorization",
        "domain": domain,
        "message": message
    });

    let typed_data_str = serde_json::to_string(&typed_data)
        .map_err(|e| format!("Failed to serialize typed data: {}", e))?;

    // Build request
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

/// Get the current connected address
pub fn get_address() -> Option<String> {
    // This would need to check local state or make another request
    // For now, return None - the app state tracks this
    None
}
