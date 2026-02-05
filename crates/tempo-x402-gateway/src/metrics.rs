use lazy_static::lazy_static;
use prometheus::{Histogram, HistogramOpts, IntCounter, IntCounterVec, Opts, Registry};

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    // Request counters
    pub static ref REQUESTS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("gateway_requests_total", "Total number of requests"),
        &["method", "path", "status"]
    ).unwrap();

    // Payment counters
    pub static ref PAYMENTS_TOTAL: IntCounter = IntCounter::new(
        "gateway_payments_total",
        "Total number of successful payments"
    ).unwrap();

    pub static ref PAYMENTS_FAILED: IntCounter = IntCounter::new(
        "gateway_payments_failed",
        "Total number of failed payments"
    ).unwrap();

    // Endpoint counters
    pub static ref ENDPOINTS_REGISTERED: IntCounter = IntCounter::new(
        "gateway_endpoints_registered",
        "Total number of endpoints registered"
    ).unwrap();

    // Proxy metrics
    pub static ref PROXY_REQUESTS_TOTAL: IntCounter = IntCounter::new(
        "gateway_proxy_requests_total",
        "Total number of proxied requests"
    ).unwrap();

    pub static ref PROXY_LATENCY: Histogram = Histogram::with_opts(
        HistogramOpts::new("gateway_proxy_latency_seconds", "Proxy request latency")
            .buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0])
    ).unwrap();
}

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
}
