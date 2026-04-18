//! WebCrypto-based encryption for wallet private keys.
//!
//! Uses PBKDF2 (100k iterations, SHA-256) → AES-GCM-256 to encrypt keys
//! before storing them in localStorage. This prevents trivial extraction
//! via XSS or devtools.
//!
//! Storage format: base64(version_byte || salt_16 || iv_12 || ciphertext_with_tag)

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

/// Current encryption format version
const CURRENT_VERSION: u8 = 1;
/// PBKDF2 iteration count
const PBKDF2_ITERATIONS: u32 = 100_000;
/// AES-GCM IV length in bytes
const IV_LEN: usize = 12;
/// PBKDF2 salt length in bytes
const SALT_LEN: usize = 16;

fn get_crypto() -> Result<web_sys::SubtleCrypto, String> {
    let window = web_sys::window().ok_or("No window")?;
    let crypto = window.crypto().map_err(|_| "No crypto API")?;
    Ok(crypto.subtle())
}

/// Derive an AES-GCM-256 key from a password using PBKDF2.
async fn derive_key(
    subtle: &web_sys::SubtleCrypto,
    password: &str,
    salt: &[u8],
) -> Result<web_sys::CryptoKey, String> {
    // Import password as raw key material
    let password_bytes = password.as_bytes();
    let key_data = js_sys::Uint8Array::from(password_bytes);

    let import_algo = js_sys::Object::new();
    js_sys::Reflect::set(&import_algo, &"name".into(), &"PBKDF2".into())
        .map_err(|e| format!("set import algo: {:?}", e))?;

    let import_usages = js_sys::Array::new();
    import_usages.push(&"deriveBits".into());
    import_usages.push(&"deriveKey".into());

    let base_key = JsFuture::from(
        subtle
            .import_key_with_object("raw", &key_data, &import_algo, false, &import_usages.into())
            .map_err(|e| format!("import_key failed: {:?}", e))?,
    )
    .await
    .map_err(|e| format!("import_key promise: {:?}", e))?;

    // Derive AES-GCM key via PBKDF2
    let salt_array = js_sys::Uint8Array::from(salt);
    let derive_params = js_sys::Object::new();
    js_sys::Reflect::set(&derive_params, &"name".into(), &"PBKDF2".into())
        .map_err(|e| format!("set derive name: {:?}", e))?;
    js_sys::Reflect::set(&derive_params, &"salt".into(), &salt_array.into())
        .map_err(|e| format!("set derive salt: {:?}", e))?;
    js_sys::Reflect::set(
        &derive_params,
        &"iterations".into(),
        &JsValue::from_f64(PBKDF2_ITERATIONS as f64),
    )
    .map_err(|e| format!("set derive iterations: {:?}", e))?;
    let hash_algo = js_sys::Object::new();
    js_sys::Reflect::set(&hash_algo, &"name".into(), &"SHA-256".into())
        .map_err(|e| format!("set hash name: {:?}", e))?;
    js_sys::Reflect::set(&derive_params, &"hash".into(), &hash_algo.into())
        .map_err(|e| format!("set derive hash: {:?}", e))?;

    let derived_algo = js_sys::Object::new();
    js_sys::Reflect::set(&derived_algo, &"name".into(), &"AES-GCM".into())
        .map_err(|e| format!("set derived name: {:?}", e))?;
    js_sys::Reflect::set(&derived_algo, &"length".into(), &JsValue::from_f64(256.0))
        .map_err(|e| format!("set derived length: {:?}", e))?;

    let derive_usages = js_sys::Array::new();
    derive_usages.push(&"encrypt".into());
    derive_usages.push(&"decrypt".into());

    let derived_key = JsFuture::from(
        subtle
            .derive_key_with_object_and_object(
                &derive_params,
                &base_key.into(),
                &derived_algo,
                false,
                &derive_usages.into(),
            )
            .map_err(|e| format!("derive_key failed: {:?}", e))?,
    )
    .await
    .map_err(|e| format!("derive_key promise: {:?}", e))?;

    Ok(derived_key.into())
}

/// Encrypt a private key with a user-chosen password.
///
/// Returns a base64 string: version(1) || salt(16) || iv(12) || ciphertext+tag
pub async fn encrypt_key(password: &str, plaintext_key: &str) -> Result<String, String> {
    if password.is_empty() {
        return Err("Password cannot be empty".to_string());
    }

    let subtle = get_crypto()?;

    // Generate random salt and IV
    let mut salt = [0u8; SALT_LEN];
    let mut iv = [0u8; IV_LEN];
    getrandom::fill(&mut salt).map_err(|e| format!("salt rng: {e}"))?;
    getrandom::fill(&mut iv).map_err(|e| format!("iv rng: {e}"))?;

    let key = derive_key(&subtle, password, &salt).await?;

    // Encrypt
    let plaintext_bytes = plaintext_key.as_bytes();
    let iv_array = js_sys::Uint8Array::from(&iv[..]);

    let encrypt_params = js_sys::Object::new();
    js_sys::Reflect::set(&encrypt_params, &"name".into(), &"AES-GCM".into())
        .map_err(|e| format!("set enc name: {:?}", e))?;
    js_sys::Reflect::set(&encrypt_params, &"iv".into(), &iv_array.into())
        .map_err(|e| format!("set enc iv: {:?}", e))?;

    let ciphertext = JsFuture::from(
        subtle
            .encrypt_with_object_and_u8_array(&encrypt_params, &key, plaintext_bytes)
            .map_err(|e| format!("encrypt failed: {:?}", e))?,
    )
    .await
    .map_err(|e| format!("encrypt promise: {:?}", e))?;

    let ciphertext_array = js_sys::Uint8Array::new(&ciphertext);
    let ciphertext_bytes = ciphertext_array.to_vec();

    // Pack: version || salt || iv || ciphertext+tag
    let mut packed = Vec::with_capacity(1 + SALT_LEN + IV_LEN + ciphertext_bytes.len());
    packed.push(CURRENT_VERSION);
    packed.extend_from_slice(&salt);
    packed.extend_from_slice(&iv);
    packed.extend_from_slice(&ciphertext_bytes);

    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &packed,
    ))
}

/// Decrypt a previously encrypted private key with the password.
pub async fn decrypt_key(password: &str, encrypted_blob: &str) -> Result<String, String> {
    let packed = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encrypted_blob)
        .map_err(|_| "Invalid encrypted data (base64 decode failed)".to_string())?;

    if packed.is_empty() {
        return Err("Empty encrypted data".to_string());
    }

    let version = packed[0];
    if version != CURRENT_VERSION {
        return Err(format!("Unsupported encryption version: {version}"));
    }

    let min_len = 1 + SALT_LEN + IV_LEN + 1; // at least 1 byte of ciphertext
    if packed.len() < min_len {
        return Err("Encrypted data too short".to_string());
    }

    let salt = &packed[1..1 + SALT_LEN];
    let iv = &packed[1 + SALT_LEN..1 + SALT_LEN + IV_LEN];
    let ciphertext = &packed[1 + SALT_LEN + IV_LEN..];

    let subtle = get_crypto()?;
    let key = derive_key(&subtle, password, salt).await?;

    let iv_array = js_sys::Uint8Array::from(iv);
    let decrypt_params = js_sys::Object::new();
    js_sys::Reflect::set(&decrypt_params, &"name".into(), &"AES-GCM".into())
        .map_err(|e| format!("set dec name: {:?}", e))?;
    js_sys::Reflect::set(&decrypt_params, &"iv".into(), &iv_array.into())
        .map_err(|e| format!("set dec iv: {:?}", e))?;

    let ciphertext_array = js_sys::Uint8Array::from(ciphertext);

    let plaintext = JsFuture::from(
        subtle
            .decrypt_with_object_and_buffer_source(&decrypt_params, &key, &ciphertext_array)
            .map_err(|_| "Decryption failed — wrong password or corrupted data".to_string())?,
    )
    .await
    .map_err(|_| "Decryption failed — wrong password or corrupted data".to_string())?;

    let plaintext_array = js_sys::Uint8Array::new(&plaintext);
    let plaintext_bytes = plaintext_array.to_vec();

    String::from_utf8(plaintext_bytes).map_err(|_| "Decrypted data is not valid UTF-8".to_string())
}

/// Check if a stored value looks like it's encrypted (base64 with version prefix).
///
/// Legacy (unencrypted) values start with "0x" (hex private key).
/// Encrypted values are base64 and start with version byte 0x01.
pub fn is_encrypted(stored: &str) -> bool {
    // Unencrypted keys start with 0x
    if stored.starts_with("0x") || stored.starts_with("0X") {
        return false;
    }
    // Try to decode as base64 and check version
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, stored)
        .map(|bytes| !bytes.is_empty() && bytes[0] == CURRENT_VERSION)
        .unwrap_or(false)
}
