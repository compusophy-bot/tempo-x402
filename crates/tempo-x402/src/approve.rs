use alloy::network::EthereumWallet;
use alloy::primitives::{Address, U256};
use alloy::providers::ProviderBuilder;
use alloy::signers::local::PrivateKeySigner;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let client_key =
        std::env::var("EVM_PRIVATE_KEY").expect("EVM_PRIVATE_KEY environment variable is required");

    let facilitator_address: Address = std::env::var("FACILITATOR_ADDRESS")
        .expect("FACILITATOR_ADDRESS environment variable is required")
        .parse()
        .expect("invalid FACILITATOR_ADDRESS");

    let token: Address = std::env::var("TEMPO_TOKEN")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(x402::DEFAULT_TOKEN);

    let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| x402::RPC_URL.to_string());

    let approve_amount: U256 = match std::env::var("APPROVE_AMOUNT") {
        Ok(val) => val
            .parse::<U256>()
            .expect("invalid APPROVE_AMOUNT -- must be a valid U256"),
        Err(_) => {
            eprintln!(
                "ERROR: APPROVE_AMOUNT is required.\n\
                 Set it to the token amount to approve (e.g. 1000000000 for 1000 tokens at 6 decimals).\n\
                 To grant unlimited approval (NOT recommended), set APPROVE_AMOUNT=MAX."
            );
            std::process::exit(1);
        }
    };
    // Allow "MAX" as a convenience alias for U256::MAX
    let approve_amount = if std::env::var("APPROVE_AMOUNT").as_deref() == Ok("MAX") {
        tracing::warn!(
            "APPROVE_AMOUNT=MAX — granting unlimited spend authority to the facilitator. \
             This is NOT recommended for production."
        );
        U256::MAX
    } else {
        approve_amount
    };

    let signer: PrivateKeySigner = client_key.parse().expect("invalid EVM_PRIVATE_KEY");
    let account_address = signer.address();

    println!("Approving facilitator for TIP-20 token...");
    println!("  Client:      {account_address}");
    println!("  Facilitator: {facilitator_address}");
    println!("  Token:       {token}");
    println!("  Amount:      {approve_amount}");

    let provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(signer))
        .connect_http(rpc_url.parse().expect("invalid RPC_URL"));

    // Check current allowance
    let current = x402::tip20::allowance(&provider, token, account_address, facilitator_address)
        .await
        .expect("failed to read allowance");

    println!("\nCurrent allowance: {current}");

    if current >= approve_amount {
        println!("Facilitator already has sufficient allowance -- nothing to do.");
        return;
    }

    println!("Sending approval transaction...");
    let tx_hash = x402::tip20::approve(&provider, token, facilitator_address, approve_amount)
        .await
        .expect("approval failed");

    println!("  tx: {tx_hash}");
    println!("Approval confirmed.");
}
