use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use alloy::providers::Provider;
use serde_json::Value;

use crate::state::NodeState;

#[get("/network-stats")]
pub async fn network_stats(state: web::Data<NodeState>) -> impl Responder {
    let facilitator = match state.gateway.facilitator.as_ref() {
        Some(f) => f,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({ "error": "Facilitator not enabled" }))
        }
    };

    let provider = facilitator.facilitator.provider();

    let (block_number, chain_id, gas_price) = tokio::join!(
        provider.get_block_number(),
        provider.get_chain_id(),
        provider.get_gas_price(),
    );

    HttpResponse::Ok().json(serde_json::json!({
        "block_number": block_number.ok(),
        "chain_id": chain_id.ok(),
        "gas_price": gas_price.ok(),
    }))
}

#[get("/echo-ip")]
pub async fn echo_ip(req: HttpRequest) -> impl Responder {
    let connection_info = req.connection_info();
    let ip = connection_info.realip_remote_addr().unwrap_or("unknown");
    HttpResponse::Ok().json(serde_json::json!({ "ip": ip }))
}

#[get("/headers")]
pub async fn headers(req: HttpRequest) -> impl Responder {
    let mut headers = serde_json::Map::new();
    for (name, value) in req.headers() {
        if let Ok(val_str) = value.to_str() {
            headers.insert(name.to_string(), Value::String(val_str.to_string()));
        }
    }
    HttpResponse::Ok().json(headers)
}

#[post("/json-validator")]
pub async fn json_validator(body: String) -> impl Responder {
    match serde_json::from_str::<Value>(&body) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({ "valid": true })),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "valid": false, "error": e.to_string() })),
    }
}

#[post("/hex-converter")]
pub async fn hex_converter(body: String) -> impl Responder {
    if let Ok(decoded) = alloy::hex::decode(body.trim()) {
        HttpResponse::Ok().json(serde_json::json!({
            "action": "decode",
            "result": String::from_utf8_lossy(&decoded).to_string()
        }))
    } else {
        HttpResponse::Ok().json(serde_json::json!({
            "action": "encode",
            "result": alloy::hex::encode(body.trim())
        }))
    }
}

#[post("/estimate-gas")]
pub async fn estimate_gas(
    state: web::Data<NodeState>,
    body: web::Json<serde_json::Value>,
) -> impl Responder {
    let facilitator = match state.gateway.facilitator.as_ref() {
        Some(f) => f,
        None => {
            return HttpResponse::ServiceUnavailable()
                .json(serde_json::json!({ "error": "Facilitator not enabled" }))
        }
    };

    let provider = facilitator.facilitator.provider();

    // Body should be a JSON object representing a TransactionRequest
    // For now, we use a generic value as alloy can often deserialize from it
    match provider.estimate_gas(serde_json::from_value(body.into_inner()).unwrap_or_default()).await {
        Ok(gas) => HttpResponse::Ok().json(serde_json::json!({ "gas_limit": gas })),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e.to_string() })),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/utils")
            .service(network_stats)
            .service(echo_ip)
            .service(headers)
            .service(json_validator)
            .service(hex_converter)
            .service(estimate_gas),
    );
}
