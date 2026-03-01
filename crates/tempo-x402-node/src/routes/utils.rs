use actix_web::{web, HttpRequest, HttpResponse};
use serde_json::json;

/// Echo the caller's IP address.
async fn echo_ip(req: HttpRequest) -> HttpResponse {
    let connection_info = req.connection_info();
    let ip = connection_info.realip_remote_addr().unwrap_or("unknown");
    HttpResponse::Ok().json(json!({ "ip": ip }))
}

/// Echo the request headers.
async fn headers(req: HttpRequest) -> HttpResponse {
    let mut headers = serde_json::Map::new();
    for (name, value) in req.headers() {
        if let Ok(val_str) = value.to_str() {
            headers.insert(name.to_string(), serde_json::Value::String(val_str.to_string()));
        }
    }
    HttpResponse::Ok().json(headers)
}

/// Validate a JSON payload.
async fn json_validator(body: String) -> HttpResponse {
    match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(_) => HttpResponse::Ok().json(json!({ "valid": true })),
        Err(e) => HttpResponse::BadRequest().json(json!({ "valid": false, "error": e.to_string() })),
    }
}

/// Convert a string to hex.
async fn hex_converter(body: String) -> HttpResponse {
    let hex = alloy::hex::encode(body);
    HttpResponse::Ok().json(json!({ "hex": hex }))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/utils/echo-ip", web::get().to(echo_ip))
        .route("/utils/headers", web::get().to(headers))
        .route("/utils/json-validator", web::post().to(json_validator))
        .route("/utils/hex-converter", web::post().to(hex_converter));
}
