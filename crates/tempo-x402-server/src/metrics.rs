use prometheus::{register_int_counter_vec, Encoder, IntCounterVec, TextEncoder};
use std::sync::LazyLock;

pub static REQUESTS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "x402_server_requests_total",
        "Total HTTP requests",
        &["endpoint", "status"]
    )
    .unwrap()
});

pub static PAYMENT_ATTEMPTS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "x402_server_payment_attempts_total",
        "Total payment attempts",
        &["result"]
    )
    .unwrap()
});

pub fn metrics_output() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    if encoder.encode(&metric_families, &mut buffer).is_err() {
        return String::new();
    }
    String::from_utf8(buffer).unwrap_or_default()
}
