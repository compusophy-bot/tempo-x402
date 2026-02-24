use prometheus::{Histogram, HistogramOpts, IntCounter, IntCounterVec, Opts, Registry};
use std::sync::LazyLock;

pub static REGISTRY: LazyLock<Registry> = LazyLock::new(Registry::new);

// Request counters
pub static REQUESTS_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        Opts::new("gateway_requests_total", "Total number of requests"),
        &["method", "path", "status"],
    )
    .unwrap()
});

// Payment counters
pub static PAYMENTS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "gateway_payments_total",
        "Total number of successful payments",
    )
    .unwrap()
});

pub static PAYMENTS_FAILED: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new("gateway_payments_failed", "Total number of failed payments").unwrap()
});

// Endpoint counters
pub static ENDPOINTS_REGISTERED: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "gateway_endpoints_registered",
        "Total number of endpoints registered",
    )
    .unwrap()
});

// Proxy metrics
pub static PROXY_REQUESTS_TOTAL: LazyLock<IntCounter> = LazyLock::new(|| {
    IntCounter::new(
        "gateway_proxy_requests_total",
        "Total number of proxied requests",
    )
    .unwrap()
});

pub static PROXY_LATENCY: LazyLock<Histogram> = LazyLock::new(|| {
    Histogram::with_opts(
        HistogramOpts::new("gateway_proxy_latency_seconds", "Proxy request latency")
            .buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
    )
    .unwrap()
});

// Per-endpoint counters
pub static ENDPOINT_PAYMENTS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        Opts::new(
            "gateway_endpoint_payments_total",
            "Successful payments per endpoint",
        ),
        &["slug"],
    )
    .unwrap()
});

pub static ENDPOINT_REVENUE: LazyLock<IntCounterVec> = LazyLock::new(|| {
    IntCounterVec::new(
        Opts::new(
            "gateway_endpoint_revenue_total",
            "Revenue in token units per endpoint",
        ),
        &["slug"],
    )
    .unwrap()
});

/// Register all metrics with the registry
pub fn register_metrics() {
    REGISTRY.register(Box::new(REQUESTS_TOTAL.clone())).unwrap();
    REGISTRY.register(Box::new(PAYMENTS_TOTAL.clone())).unwrap();
    REGISTRY
        .register(Box::new(PAYMENTS_FAILED.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(ENDPOINTS_REGISTERED.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(PROXY_REQUESTS_TOTAL.clone()))
        .unwrap();
    REGISTRY.register(Box::new(PROXY_LATENCY.clone())).unwrap();
    REGISTRY
        .register(Box::new(ENDPOINT_PAYMENTS.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(ENDPOINT_REVENUE.clone()))
        .unwrap();
}
