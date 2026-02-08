use crate::X402Error;
use alloy::primitives::{Address, U256};
use alloy::providers::Provider;

use crate::TIP20;

/// Query the TIP-20 balance of `owner`.
pub async fn balance_of<P: Provider>(
    provider: &P,
    token: Address,
    owner: Address,
) -> Result<U256, X402Error> {
    let contract = TIP20::new(token, provider);
    let balance = contract
        .balanceOf(owner)
        .call()
        .await
        .map_err(|e| X402Error::ChainError(format!("balanceOf failed: {e}")))?;
    Ok(balance)
}

/// Query the TIP-20 allowance that `owner` has granted to `spender`.
pub async fn allowance<P: Provider>(
    provider: &P,
    token: Address,
    owner: Address,
    spender: Address,
) -> Result<U256, X402Error> {
    let contract = TIP20::new(token, provider);
    let remaining = contract
        .allowance(owner, spender)
        .call()
        .await
        .map_err(|e| X402Error::ChainError(format!("allowance failed: {e}")))?;
    Ok(remaining)
}

/// Execute `transferFrom(from, to, value)` on the TIP-20 contract.
/// Returns the transaction hash.
///
/// Includes a 60-second timeout on receipt confirmation to prevent indefinite hangs
/// when the chain is congested or gas price is too low.
pub async fn transfer_from<P: Provider>(
    provider: &P,
    token: Address,
    from: Address,
    to: Address,
    value: U256,
) -> Result<alloy::primitives::TxHash, X402Error> {
    let contract = TIP20::new(token, provider);
    // Timeout on send() to prevent indefinite hang if the RPC is unresponsive.
    // This holds the per-payer lock in the caller, so a hang here blocks all
    // settlement for that payer.
    let pending = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        contract.transferFrom(from, to, value).send(),
    )
    .await
    .map_err(|_| X402Error::ChainError("transferFrom send timed out after 30s".to_string()))?
    .map_err(|e| X402Error::ChainError(format!("transferFrom send failed: {e}")))?;

    let receipt = tokio::time::timeout(std::time::Duration::from_secs(60), pending.get_receipt())
        .await
        .map_err(|_| X402Error::ChainError("transferFrom receipt timed out after 60s".to_string()))?
        .map_err(|e| X402Error::ChainError(format!("transferFrom receipt failed: {e}")))?;

    if !receipt.status() {
        return Err(X402Error::ChainError("transferFrom reverted".to_string()));
    }

    Ok(receipt.transaction_hash)
}

/// Execute `approve(spender, amount)` on the TIP-20 contract.
/// Returns the transaction hash.
///
/// Includes a 60-second timeout on receipt confirmation.
pub async fn approve<P: Provider>(
    provider: &P,
    token: Address,
    spender: Address,
    amount: U256,
) -> Result<alloy::primitives::TxHash, X402Error> {
    let contract = TIP20::new(token, provider);
    let pending = contract
        .approve(spender, amount)
        .send()
        .await
        .map_err(|e| X402Error::ChainError(format!("approve send failed: {e}")))?;

    let receipt = tokio::time::timeout(std::time::Duration::from_secs(60), pending.get_receipt())
        .await
        .map_err(|_| X402Error::ChainError("approve receipt timed out after 60s".to_string()))?
        .map_err(|e| X402Error::ChainError(format!("approve receipt failed: {e}")))?;

    if !receipt.status() {
        return Err(X402Error::ChainError("approve reverted".to_string()));
    }

    Ok(receipt.transaction_hash)
}
