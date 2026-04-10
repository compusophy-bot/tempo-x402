#![no_std]
#![no_main]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! { loop {} }

#[link(wasm_import_module = "x402")]
extern "C" {
    fn response(status: i32, body_ptr: *const u8, body_len: i32, ct_ptr: *const u8, ct_len: i32);
    fn log(level: i32, msg_ptr: *const u8, msg_len: i32);
    fn kv_get(key_ptr: *const u8, key_len: i32) -> i64;
    fn kv_set(key_ptr: *const u8, key_len: i32, val_ptr: *const u8, val_len: i32) -> i32;
    fn payment_info() -> i64;
}

fn respond(status: i32, body: &str, content_type: &str) {
    unsafe {
        response(status, body.as_ptr(), body.len() as i32, content_type.as_ptr(), content_type.len() as i32);
    }
}

fn host_log(level: i32, msg: &str) {
    unsafe { log(level, msg.as_ptr(), msg.len() as i32); }
}

const STATUS_JSON: &str = r#"{
  "status": "partial_outage",
  "updated_at": "2026-04-10T14:30:00Z",
  "services": [
    {
      "name": "API Gateway",
      "status": "operational",
      "latency_ms": 12,
      "uptime_30d": 99.98,
      "description": "Core API routing and request handling"
    },
    {
      "name": "Authentication",
      "status": "operational",
      "latency_ms": 8,
      "uptime_30d": 99.99,
      "description": "OAuth2, SSO, and session management"
    },
    {
      "name": "Primary Database",
      "status": "operational",
      "latency_ms": 3,
      "uptime_30d": 99.99,
      "description": "PostgreSQL primary cluster (US-East)"
    },
    {
      "name": "Read Replicas",
      "status": "operational",
      "latency_ms": 5,
      "uptime_30d": 99.97,
      "description": "PostgreSQL read replicas (multi-region)"
    },
    {
      "name": "Cache Layer",
      "status": "degraded",
      "latency_ms": 45,
      "uptime_30d": 98.50,
      "description": "Redis cluster for session and query caching",
      "incident": {
        "id": "INC-2026-0410-001",
        "title": "Elevated cache latency in EU-West region",
        "started_at": "2026-04-10T13:15:00Z",
        "severity": "minor",
        "message": "We are investigating elevated cache response times in the EU-West region. US and APAC regions are unaffected. A mitigation is in progress."
      }
    },
    {
      "name": "Object Storage",
      "status": "operational",
      "latency_ms": 18,
      "uptime_30d": 99.95,
      "description": "S3-compatible blob storage for user assets"
    },
    {
      "name": "Search Index",
      "status": "operational",
      "latency_ms": 22,
      "uptime_30d": 99.92,
      "description": "Elasticsearch full-text search cluster"
    },
    {
      "name": "Webhook Delivery",
      "status": "operational",
      "latency_ms": 85,
      "uptime_30d": 99.88,
      "description": "Outbound webhook dispatch and retry queue"
    },
    {
      "name": "Background Jobs",
      "status": "operational",
      "latency_ms": 0,
      "uptime_30d": 99.96,
      "description": "Async task processing (email, reports, cleanup)"
    },
    {
      "name": "CDN",
      "status": "operational",
      "latency_ms": 2,
      "uptime_30d": 100.00,
      "description": "Global content delivery network for static assets"
    }
  ],
  "recent_incidents": [
    {
      "id": "INC-2026-0410-001",
      "title": "Elevated cache latency in EU-West",
      "status": "investigating",
      "severity": "minor",
      "started_at": "2026-04-10T13:15:00Z"
    },
    {
      "id": "INC-2026-0408-002",
      "title": "Brief API gateway timeout spike",
      "status": "resolved",
      "severity": "minor",
      "started_at": "2026-04-08T09:42:00Z",
      "resolved_at": "2026-04-08T09:58:00Z"
    }
  ]
}"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(1, "serving status page json");
    respond(200, STATUS_JSON, "application/json");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
