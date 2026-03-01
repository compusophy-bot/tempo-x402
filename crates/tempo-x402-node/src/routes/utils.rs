use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde_json::Value;

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
    if let Ok(decoded) = hex::decode(body.trim()) {
        HttpResponse::Ok().json(serde_json::json!({
            "action": "decode",
            "result": String::from_utf8_lossy(&decoded).to_string()
        }))
    } else {
        HttpResponse::Ok().json(serde_json::json!({
            "action": "encode",
            "result": hex::encode(body.trim())
        }))
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/utils")
            .service(echo_ip)
            .service(headers)
            .service(json_validator)
            .service(hex_converter),
    );
}
