//! Wallet connection utilities supporting MetaMask, demo key, and embedded wallets.
//!
//! Embedded wallets are persisted to localStorage. The private key survives
//! page refreshes and disconnects. Users can export (reveal key, download JSON)
//! and import (paste key) to manage their wallet across devices.
//!
//! Keys are encrypted with AES-GCM-256 using a password derived via PBKDF2
//! before being written to localStorage.

#![allow(dead_code, deprecated)]

use crate::WalletState;
use wasm_bindgen::prelude::*;

use crate::wallet_crypto;
pub use crate::WalletMode;

const STORAGE_KEY: &str = "x402_embedded_wallet";

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window, js_name = ethereum)]
    static ETHEREUM: JsValue;

    #[wasm_bindgen(catch, js_namespace = ["window", "ethereum"], js_name = request)]
    async fn ethereum_request(args: &JsValue) -> Result<JsValue, JsValue>;
}

// --- localStorage helpers ---

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn storage_get(key: &str) -> Option<String> {
    local_storage()?.get_item(key).ok()?
}

fn storage_set(key: &str, value: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(key, value);
    }
}

fn storage_remove(key: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.remove_item(key);
    }
}

// --- Wallet operations ---

/// Connect to browser wallet (MetaMask, etc.)
pub async fn connect_wallet() -> Result<WalletState, String> {
    if ETHEREUM.is_undefined() || ETHEREUM.is_null() {
        return Err("No Web3 wallet detected. Please install MetaMask.".to_string());
    }

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

/// Check if an embedded wallet exists in localStorage.
pub fn has_stored_wallet() -> bool {
    storage_get(STORAGE_KEY).is_some()
}

/// Check if the stored wallet is encrypted (needs password to unlock).
pub fn is_wallet_encrypted() -> bool {
    storage_get(STORAGE_KEY)
        .map(|v| wallet_crypto::is_encrypted(&v))
        .unwrap_or(false)
}

/// Load an encrypted embedded wallet by decrypting from localStorage.
///
/// If the stored key is in legacy plaintext format, it is transparently
/// re-encrypted with the provided password.
pub async fn load_encrypted_wallet(password: &str) -> Result<WalletState, String> {
    let stored = storage_get(STORAGE_KEY).ok_or("No stored wallet found")?;

    let key_hex = if wallet_crypto::is_encrypted(&stored) {
        // Decrypt with password
        wallet_crypto::decrypt_key(password, &stored).await?
    } else {
        // Legacy plaintext — migrate to encrypted format
        let key = stored.clone();
        let encrypted = wallet_crypto::encrypt_key(password, &key).await?;
        storage_set(STORAGE_KEY, &encrypted);
        key
    };

    let signer = x402_wallet::WalletSigner::new(&key_hex)?;
    Ok(WalletState {
        connected: true,
        address: Some(signer.address_string()),
        chain_id: Some(format!("0x{:x}", x402_wallet::TEMPO_CHAIN_ID)),
        mode: WalletMode::Embedded,
        private_key: Some(key_hex),
    })
}

/// Create a new embedded wallet, encrypt with password, and save to localStorage.
///
/// Returns `(wallet_state, is_new)` — caller should fund new wallets.
pub async fn create_embedded_wallet(password: &str) -> Result<(WalletState, bool), String> {
    if password.is_empty() {
        return Err("Password is required for wallet encryption".to_string());
    }

    // If wallet already exists, try to load it
    if has_stored_wallet() {
        let state = load_encrypted_wallet(password).await?;
        return Ok((state, false));
    }

    let key_hex = x402_wallet::generate_random_key();
    let signer = x402_wallet::WalletSigner::new(&key_hex)?;
    let address = signer.address_string();

    // Encrypt and store
    let encrypted = wallet_crypto::encrypt_key(password, &key_hex).await?;
    storage_set(STORAGE_KEY, &encrypted);

    Ok((
        WalletState {
            connected: true,
            address: Some(address),
            chain_id: Some(format!("0x{:x}", x402_wallet::TEMPO_CHAIN_ID)),
            mode: WalletMode::Embedded,
            private_key: Some(key_hex),
        },
        true,
    ))
}

/// Load or create an embedded wallet (legacy unencrypted path, kept for backward compat).
///
/// If a key exists in localStorage, restores that wallet.
/// Otherwise generates a new random keypair and persists it.
/// Returns `(wallet_state, is_new)` — caller should fund new wallets.
pub fn load_or_create_embedded_wallet() -> Result<(WalletState, bool), String> {
    if let Some(key_hex) = storage_get(STORAGE_KEY) {
        // If encrypted, can't load without password
        if wallet_crypto::is_encrypted(&key_hex) {
            return Err("Wallet is encrypted — enter your password to unlock".to_string());
        }

        let signer = x402_wallet::WalletSigner::new(&key_hex)?;
        return Ok((
            WalletState {
                connected: true,
                address: Some(signer.address_string()),
                chain_id: Some(format!("0x{:x}", x402_wallet::TEMPO_CHAIN_ID)),
                mode: WalletMode::Embedded,
                private_key: Some(key_hex),
            },
            false,
        ));
    }

    let key_hex = x402_wallet::generate_random_key();
    let signer = x402_wallet::WalletSigner::new(&key_hex)?;
    let address = signer.address_string();

    storage_set(STORAGE_KEY, &key_hex);

    Ok((
        WalletState {
            connected: true,
            address: Some(address),
            chain_id: Some(format!("0x{:x}", x402_wallet::TEMPO_CHAIN_ID)),
            mode: WalletMode::Embedded,
            private_key: Some(key_hex),
        },
        true,
    ))
}

/// Import a private key as an embedded wallet with password encryption.
pub async fn import_embedded_wallet_encrypted(
    key_hex: &str,
    password: &str,
) -> Result<WalletState, String> {
    let trimmed = key_hex.trim();
    if trimmed.is_empty() {
        return Err("Empty private key".to_string());
    }
    if password.is_empty() {
        return Err("Password is required for wallet encryption".to_string());
    }

    let signer = x402_wallet::WalletSigner::new(trimmed)?;
    let address = signer.address_string();

    let encrypted = wallet_crypto::encrypt_key(password, trimmed).await?;
    storage_set(STORAGE_KEY, &encrypted);

    Ok(WalletState {
        connected: true,
        address: Some(address),
        chain_id: Some(format!("0x{:x}", x402_wallet::TEMPO_CHAIN_ID)),
        mode: WalletMode::Embedded,
        private_key: Some(trimmed.to_string()),
    })
}

/// Import a private key as an embedded wallet (legacy unencrypted).
pub fn import_embedded_wallet(key_hex: &str) -> Result<WalletState, String> {
    let trimmed = key_hex.trim();
    if trimmed.is_empty() {
        return Err("Empty private key".to_string());
    }

    let signer = x402_wallet::WalletSigner::new(trimmed)?;
    let address = signer.address_string();

    storage_set(STORAGE_KEY, trimmed);

    Ok(WalletState {
        connected: true,
        address: Some(address),
        chain_id: Some(format!("0x{:x}", x402_wallet::TEMPO_CHAIN_ID)),
        mode: WalletMode::Embedded,
        private_key: Some(trimmed.to_string()),
    })
}

/// Get the stored private key for export (without connecting).
pub fn get_stored_key() -> Option<String> {
    storage_get(STORAGE_KEY)
}

/// Delete the embedded wallet from localStorage. Destructive — key is gone.
pub fn delete_embedded_wallet() {
    storage_remove(STORAGE_KEY);
}

/// Build a JSON keystore export for download.
pub fn export_wallet_json(key_hex: &str, address: &str) -> String {
    let export = serde_json::json!({
        "version": 1,
        "type": "x402-embedded-wallet",
        "network": "eip155:42431",
        "chain": "Tempo Moderato",
        "address": address,
        "privateKey": key_hex,
        "warning": "This file contains your private key in plaintext. Anyone with this key can control your funds. Store it securely."
    });
    serde_json::to_string_pretty(&export).unwrap_or_default()
}

/// Trigger a browser file download with the given content.
pub fn trigger_download(filename: &str, content: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;

    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(content));

    let mut opts = web_sys::BlobPropertyBag::new();
    opts.type_("application/json");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts)
        .map_err(|e| format!("Blob error: {:?}", e))?;

    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|e| format!("URL error: {:?}", e))?;

    let a: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(|e| format!("Element error: {:?}", e))?
        .dyn_into()
        .map_err(|_| "Not an anchor element".to_string())?;

    a.set_href(&url);
    a.set_download(filename);
    a.click();

    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

/// Copy text to clipboard.
pub fn copy_to_clipboard(text: &str) {
    if let Some(window) = web_sys::window() {
        let clipboard = window.navigator().clipboard();
        let _ = clipboard.write_text(text);
    }
}

/// Fund an address via the Tempo `tempo_fundAddress` RPC method.
pub async fn fund_address(address: &str) -> Result<(), String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tempo_fundAddress",
        "params": [address],
        "id": 1
    });

    let resp = gloo_net::http::Request::post("https://rpc.moderato.tempo.xyz")
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&body).map_err(|e| format!("Failed to serialize: {}", e))?)
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
