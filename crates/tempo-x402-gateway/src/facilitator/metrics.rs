use prometheus::{
    register_histogram_vec, register_int_counter_vec, Encoder, HistogramVec, IntCounterVec,
    TextEncoder,
};
use std::sync::LazyLock;

pub static VERIFY_REQUESTS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "x402_facilitator_verify_total",
        "Total verification requests",
        &["result"]
    )
    .unwrap()
});

pub static SETTLE_REQUESTS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "x402_facilitator_settle_total",
        "Total settlement requests",
        &["result"]
    )
    .unwrap()
});

pub static SETTLE_LATENCY: LazyLock<HistogramVec> = LazyLock::new(|| {
    register_histogram_vec!(
        "x402_facilitator_settle_duration_seconds",
        "Settlement latency in seconds",
        &["result"],
        vec![0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]
    )
    .unwrap()
});

pub static HMAC_FAILURES: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "x402_facilitator_hmac_failures_total",
        "HMAC authentication failures",
        &["reason"]
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
