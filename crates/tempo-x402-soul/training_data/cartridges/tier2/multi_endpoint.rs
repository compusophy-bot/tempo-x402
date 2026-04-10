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

fn find_json_str<'a>(json: &'a [u8], key: &[u8]) -> Option<&'a str> {
    let mut i = 0;
    while i + key.len() + 3 < json.len() {
        if json[i] == b'"' {
            let start = i + 1;
            if start + key.len() < json.len()
                && &json[start..start + key.len()] == key
                && json[start + key.len()] == b'"'
            {
                let mut j = start + key.len() + 1;
                while j < json.len() && (json[j] == b':' || json[j] == b' ') {
                    j += 1;
                }
                if j < json.len() && json[j] == b'"' {
                    let val_start = j + 1;
                    let mut val_end = val_start;
                    while val_end < json.len() && json[val_end] != b'"' {
                        val_end += 1;
                    }
                    return core::str::from_utf8(&json[val_start..val_end]).ok();
                }
            }
        }
        i += 1;
    }
    None
}

fn bytes_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

fn starts_with(haystack: &[u8], needle: &[u8]) -> bool {
    if haystack.len() < needle.len() {
        return false;
    }
    let mut i = 0;
    while i < needle.len() {
        if haystack[i] != needle[i] {
            return false;
        }
        i += 1;
    }
    true
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

const USERS_RESPONSE: &str = r#"{"endpoint":"/api/v1/users","data":[{"id":1,"name":"alice","role":"admin"},{"id":2,"name":"bob","role":"user"},{"id":3,"name":"charlie","role":"viewer"}],"total":3}"#;

const CONFIG_RESPONSE: &str = r#"{"endpoint":"/api/v1/config","data":{"chain_id":42431,"network":"tempo-moderato","token":"pathUSD","max_gas":1000000,"features":{"wasm":true,"kv_store":true,"payments":true}}}"#;

const STATUS_RESPONSE: &str = r#"{"endpoint":"/api/v1/status","data":{"healthy":true,"version":"2.1.0","uptime_hours":720,"active_cartridges":12,"requests_served":50000}}"#;

const METRICS_RESPONSE: &str = r#"{"endpoint":"/api/v1/metrics","data":{"cpu_usage_pct":23,"memory_mb":64,"wasm_instances":4,"avg_response_ms":12,"p99_response_ms":45}}"#;

const INDEX_RESPONSE: &str = r#"{"endpoints":["/api/v1/users","/api/v1/config","/api/v1/status","/api/v1/metrics"],"version":"v1"}"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };
    let path = find_json_str(request, b"path").unwrap_or("/");
    let path_bytes = path.as_bytes();

    host_log(0, "multi_endpoint: routing API v1 request");

    if bytes_equal(path_bytes, b"/api/v1/users") {
        respond(200, USERS_RESPONSE, "application/json");
    } else if bytes_equal(path_bytes, b"/api/v1/config") {
        respond(200, CONFIG_RESPONSE, "application/json");
    } else if bytes_equal(path_bytes, b"/api/v1/status") {
        respond(200, STATUS_RESPONSE, "application/json");
    } else if bytes_equal(path_bytes, b"/api/v1/metrics") {
        respond(200, METRICS_RESPONSE, "application/json");
    } else if bytes_equal(path_bytes, b"/api/v1") || bytes_equal(path_bytes, b"/api/v1/") {
        respond(200, INDEX_RESPONSE, "application/json");
    } else if starts_with(path_bytes, b"/api/v1") {
        respond(404, r#"{"error":"endpoint not found","api_version":"v1"}"#, "application/json");
    } else {
        respond(404, r#"{"error":"not found","hint":"all endpoints are under /api/v1/"}"#, "application/json");
    }
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
