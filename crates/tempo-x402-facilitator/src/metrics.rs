use lazy_static::lazy_static;
use prometheus::{
    register_histogram_vec, register_int_counter_vec, Encoder, HistogramVec, IntCounterVec,
    TextEncoder,
};

lazy_static! {
    pub static ref VERIFY_REQUESTS: IntCounterVec = register_int_counter_vec!(
        "x402_facilitator_verify_total",
        "Total verification requests",
        &["result"]
    )
    .unwrap();
    pub static ref SETTLE_REQUESTS: IntCounterVec = register_int_counter_vec!(
        "x402_facilitator_settle_total",
        "Total settlement requests",
        &["result"]
    )
    .unwrap();
    pub static ref SETTLE_LATENCY: HistogramVec = register_histogram_vec!(
        "x402_facilitator_settle_duration_seconds",
        "Settlement latency in seconds",
        &["result"],
        vec![0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    )
    .unwrap();
    pub static ref HMAC_FAILURES: IntCounterVec = register_int_counter_vec!(
        "x402_facilitator_hmac_failures_total",
        "HMAC authentication failures",
        &["reason"]
    )
    .unwrap();
}

pub fn metrics_output() -> String {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    if encoder.encode(&metric_families, &mut buffer).is_err() {
        return String::new();
    }
    String::from_utf8(buffer).unwrap_or_default()
}
