use alloy::signers::local::PrivateKeySigner;
use x402::{TempoSchemeClient, X402Client};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let evm_private_key =
        std::env::var("EVM_PRIVATE_KEY").expect("EVM_PRIVATE_KEY environment variable is required");

    let base_url = std::env::var("RESOURCE_SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:4021".to_string());

    let endpoint_path =
        std::env::var("ENDPOINT_PATH").unwrap_or_else(|_| "/blockNumber".to_string());

    let url = format!("{base_url}{endpoint_path}");

    let signer: PrivateKeySigner = evm_private_key.parse().expect("invalid EVM_PRIVATE_KEY");

    println!("Requesting: {url}");
    println!("Paying from: {}\n", signer.address());

    let scheme_client = TempoSchemeClient::new(signer);
    let client = X402Client::new(scheme_client);

    match client.fetch(&url, reqwest::Method::GET).await {
        Ok((resp, settle)) => {
            if resp.status().is_success() {
                let body: serde_json::Value = resp.json().await.unwrap_or_default();
                println!("Response data:");
                println!(
                    "{}",
                    serde_json::to_string_pretty(&body).unwrap_or_default()
                );

                if let Some(s) = settle {
                    println!("\nPayment settled:");
                    println!("{}", serde_json::to_string_pretty(&s).unwrap_or_default());
                }
            } else {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                eprintln!("Request failed with status: {status}");
                eprintln!("{body}");
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
