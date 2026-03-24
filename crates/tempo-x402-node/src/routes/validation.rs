use actix_web::{web, HttpResponse, Responder};
use x402_soul::validation::run_consistency_check;
use serde::Serialize;

#[derive(Serialize)]
struct ValidationResponse {
    status: String,
    message: String,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/validation/check", web::get().to(check_handler));
}

async fn check_handler() -> impl Responder {
    match run_consistency_check() {
        Ok(_) => HttpResponse::Ok().json(ValidationResponse {
            status: "success".to_string(),
            message: "Test Passing: System consistency verified.".to_string(),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ValidationResponse {
            status: "error".to_string(),
            message: format!("Consistency check failed: {}", e),
        }),
    }
}
