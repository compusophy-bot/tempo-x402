use actix_web::{get, post, web, HttpRequest, HttpResponse};
use alloy::providers::{Provider, RootProvider};
use futures::StreamExt;
use std::sync::Arc;

use x402_server::config::PaymentConfig;
use x402_server::middleware;

use x402_types::{EXPLORER_BASE, TEMPO_NETWORK};

#[get("/blockNumber")]
pub async fn block_number(
    req: HttpRequest,
    payment_config: web::Data<PaymentConfig>,
    provider: web::Data<Arc<RootProvider>>,
    http_client: web::Data<reqwest::Client>,
) -> HttpResponse {
    // Use the payment gate
    let settle_response =
        match middleware::require_payment(&req, &payment_config, &http_client).await {
            Ok(s) => s,
            Err(resp) => {
                // If it's a 200 OK, it means the route isn't gated — continue
                if resp.status() == actix_web::http::StatusCode::OK {
                    return get_block_number(&provider).await;
                }
                return resp;
            }
        };

    // Payment succeeded — return content with settlement info
    let settle_json = serde_json::to_string(&settle_response).unwrap_or_default();

    let mut response = get_block_number(&provider).await;
    if let Ok(header_val) = actix_web::http::header::HeaderValue::from_str(&settle_json) {
        response.headers_mut().insert(
            actix_web::http::header::HeaderName::from_static("x-payment-response"),
            header_val,
        );
    }

    tracing::info!(
        payer = ?settle_response.payer,
        tx = %settle_response.transaction,
        "served paid /blockNumber request"
    );

    response
}

async fn get_block_number(provider: &RootProvider) -> HttpResponse {
    match provider.get_block_number().await {
        Ok(bn) => HttpResponse::Ok().json(serde_json::json!({
            "blockNumber": bn.to_string()
        })),
        Err(e) => {
            tracing::error!(error = %e, "failed to get block number from chain");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "failed to get block number"
            }))
        }
    }
}

/// Send a single SSE event
fn sse_event(data: &serde_json::Value) -> String {
    format!(
        "data: {}\n\n",
        serde_json::to_string(data).unwrap_or_default()
    )
}

#[post("/api/demo")]
pub async fn api_demo(
    _payment_config: web::Data<PaymentConfig>,
    http_client: web::Data<reqwest::Client>,
) -> HttpResponse {
    let http_client = http_client.into_inner();
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(16);

    tokio::spawn(async move {
        run_demo(tx, &http_client).await;
    });

    let byte_stream = tokio_stream::wrappers::ReceiverStream::new(rx)
        .map(|s| Ok::<_, actix_web::Error>(web::Bytes::from(s)));

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(byte_stream)
}

async fn run_demo(tx: tokio::sync::mpsc::Sender<String>, http_client: &reqwest::Client) {
    let send = |data: serde_json::Value| {
        let tx = tx.clone();
        async move {
            let _ = tx.send(sse_event(&data)).await;
        }
    };

    let evm_private_key = match std::env::var("EVM_PRIVATE_KEY") {
        Ok(k) => k,
        Err(_) => {
            send(serde_json::json!({
                "type": "error",
                "error": "EVM_PRIVATE_KEY not configured -- cannot run demo"
            }))
            .await;
            return;
        }
    };

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4021);
    let server_url = format!("http://localhost:{port}");

    send(serde_json::json!({
        "type": "step",
        "step": "request",
        "detail": "GET /blockNumber",
        "status": "ok"
    }))
    .await;

    let initial = match http_client
        .get(format!("{server_url}/blockNumber"))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            send(serde_json::json!({
                "type": "step",
                "step": "payment_required",
                "detail": format!("Request failed: {e}"),
                "status": "error"
            }))
            .await;
            send(serde_json::json!({
                "type": "error",
                "error": format!("Initial request failed: {e}")
            }))
            .await;
            return;
        }
    };

    if initial.status().as_u16() != 402 {
        send(serde_json::json!({
            "type": "step",
            "step": "payment_required",
            "detail": format!("Expected 402 but got {}", initial.status()),
            "status": "error"
        }))
        .await;
        send(serde_json::json!({
            "type": "error",
            "error": format!("Unexpected status: {}", initial.status())
        }))
        .await;
        return;
    }

    let payment_required: x402_types::PaymentRequiredBody = match initial.json().await {
        Ok(b) => b,
        Err(e) => {
            send(serde_json::json!({
                "type": "step",
                "step": "payment_required",
                "detail": format!("Failed to parse 402 body: {e}"),
                "status": "error"
            }))
            .await;
            send(serde_json::json!({
                "type": "error",
                "error": format!("Parse error: {e}")
            }))
            .await;
            return;
        }
    };

    let requirements = match payment_required.accepts.first() {
        Some(r) => r,
        None => {
            send(serde_json::json!({
                "type": "error",
                "error": "No accepted payment schemes"
            }))
            .await;
            return;
        }
    };

    let network_display = if requirements.network == TEMPO_NETWORK {
        "Tempo Moderato"
    } else {
        &requirements.network
    };
    send(serde_json::json!({
        "type": "step",
        "step": "payment_required",
        "detail": format!("402 -- {} pathUSD on {}", requirements.price, network_display),
        "status": "ok"
    }))
    .await;

    let signer: alloy::signers::local::PrivateKeySigner = match evm_private_key.parse() {
        Ok(s) => s,
        Err(e) => {
            send(serde_json::json!({
                "type": "error",
                "error": format!("Invalid private key: {e}")
            }))
            .await;
            return;
        }
    };

    let payer_address = signer.address();
    let client = x402_tempo::TempoSchemeClient::new(signer);

    use x402_types::SchemeClient;
    let payload = match client.create_payment_payload(1, requirements).await {
        Ok(p) => p,
        Err(e) => {
            send(serde_json::json!({
                "type": "error",
                "error": format!("Signing failed: {e}")
            }))
            .await;
            return;
        }
    };

    send(serde_json::json!({
        "type": "step",
        "step": "sign",
        "detail": "Signed EIP-712 PaymentAuthorization",
        "payer": format!("{payer_address}"),
        "status": "ok"
    }))
    .await;

    let payload_json = serde_json::to_vec(&payload).unwrap_or_default();
    let encoded = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &payload_json,
    );

    let paid_response = match http_client
        .get(format!("{server_url}/blockNumber"))
        .header("X-PAYMENT", &encoded)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            send(serde_json::json!({
                "type": "step",
                "step": "verify",
                "detail": format!("Paid request failed: {e}"),
                "status": "error"
            }))
            .await;
            send(serde_json::json!({
                "type": "error",
                "error": format!("Paid request failed: {e}")
            }))
            .await;
            return;
        }
    };

    if !paid_response.status().is_success() {
        let _err_body = paid_response.text().await.unwrap_or_default();
        send(serde_json::json!({
            "type": "step",
            "step": "verify",
            "detail": "Payment verification failed",
            "status": "error"
        }))
        .await;
        send(serde_json::json!({
            "type": "error",
            "error": "Payment failed"
        }))
        .await;
        return;
    }

    let tx_hash = paid_response
        .headers()
        .get("x-payment-response")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| serde_json::from_str::<x402_types::SettleResponse>(s).ok())
        .map(|s| s.transaction)
        .unwrap_or_default();

    send(serde_json::json!({
        "type": "step",
        "step": "verify",
        "detail": "Facilitator verified signature, balance, allowance",
        "status": "ok"
    }))
    .await;

    send(serde_json::json!({
        "type": "step",
        "step": "settle",
        "detail": "transferFrom executed on TIP-20",
        "tx": tx_hash,
        "status": "ok"
    }))
    .await;

    let body: serde_json::Value = paid_response.json().await.unwrap_or_default();

    send(serde_json::json!({
        "type": "step",
        "step": "response",
        "detail": "Block number received",
        "data": body,
        "status": "ok"
    }))
    .await;

    let explorer_url = if tx_hash.is_empty() {
        String::new()
    } else {
        format!("{EXPLORER_BASE}/tx/{tx_hash}")
    };

    send(serde_json::json!({
        "type": "done",
        "transaction": tx_hash,
        "explorerUrl": explorer_url,
    }))
    .await;
}
