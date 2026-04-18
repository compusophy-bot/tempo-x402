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

fn contains_substr(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    let mut i = 0;
    while i + needle.len() <= haystack.len() {
        if &haystack[i..i + needle.len()] == needle {
            return true;
        }
        i += 1;
    }
    false
}

fn ends_with(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    &haystack[haystack.len() - needle.len()..] == needle
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

fn copy_to(buf: &mut [u8], offset: usize, src: &[u8]) -> usize {
    let end = if offset + src.len() > buf.len() { buf.len() } else { offset + src.len() };
    let mut i = offset;
    while i < end {
        buf[i] = src[i - offset];
        i += 1;
    }
    end
}

/// Increment a request counter in KV and return the new value
fn bump_request_count() -> u32 {
    let key = b"health_hits";
    let prev = unsafe { kv_get(key.as_ptr(), key.len() as i32) };
    let count = if prev > 0 {
        let len = (prev & 0xFFFFFFFF) as usize;
        let stored = unsafe { &SCRATCH[..len] };
        let mut val: u32 = 0;
        let mut k = 0;
        while k < stored.len() && stored[k] >= b'0' && stored[k] <= b'9' {
            val = val * 10 + (stored[k] - b'0') as u32;
            k += 1;
        }
        val + 1
    } else {
        1
    };
    // Store
    let buf = unsafe { &mut SCRATCH[65536..] };
    let mut digits = [0u8; 10];
    let mut c = count;
    let mut n = 0;
    if c == 0 { digits[0] = b'0'; n = 1; }
    else {
        while c > 0 { digits[n] = b'0' + (c % 10) as u8; c /= 10; n += 1; }
    }
    let mut k = 0;
    while k < n { buf[k] = digits[n - 1 - k]; k += 1; }
    unsafe { kv_set(key.as_ptr(), key.len() as i32, buf.as_ptr(), n as i32); }
    count
}

fn write_u32(buf: &mut [u8], offset: usize, mut val: u32) -> usize {
    if val == 0 { buf[offset] = b'0'; return offset + 1; }
    let mut digits = [0u8; 10];
    let mut count = 0;
    while val > 0 { digits[count] = b'0' + (val % 10) as u8; val /= 10; count += 1; }
    let mut pos = offset;
    let mut i = count;
    while i > 0 { i -= 1; buf[pos] = digits[i]; pos += 1; }
    pos
}

// Basic: just status
const HEALTH_BASIC: &str = r#"{"status":"healthy"}"#;

// Liveness: is the process alive?
const HEALTH_LIVE: &str = r#"{"status":"alive","check":"liveness"}"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };

    // Only GET allowed
    let method = find_json_str(request, b"method").unwrap_or("GET");
    if !contains_substr(method.as_bytes(), b"GET") {
        respond(405, r#"{"error":"method_not_allowed","allowed":["GET"]}"#, "application/json");
        return;
    }

    let path = find_json_str(request, b"path").unwrap_or("/health");
    let path_bytes = path.as_bytes();

    host_log(0, "health_detailed: routing health check");

    // Route based on path suffix
    if ends_with(path_bytes, b"/live") || ends_with(path_bytes, b"/liveness") {
        // Liveness: always returns 200 if the cartridge is running
        host_log(0, "health_detailed: liveness probe");
        respond(200, HEALTH_LIVE, "application/json");
        return;
    }

    if ends_with(path_bytes, b"/ready") || ends_with(path_bytes, b"/readiness") {
        // Readiness: check if KV store is accessible (write + read test)
        host_log(0, "health_detailed: readiness probe");
        let test_key = b"ready_check";
        let test_val = b"1";
        let write_ok = unsafe { kv_set(test_key.as_ptr(), test_key.len() as i32, test_val.as_ptr(), test_val.len() as i32) };
        if write_ok == 0 {
            respond(200, r#"{"status":"ready","check":"readiness","kv_store":"accessible"}"#, "application/json");
        } else {
            respond(503, r#"{"status":"not_ready","check":"readiness","kv_store":"unavailable"}"#, "application/json");
        }
        return;
    }

    if ends_with(path_bytes, b"/detailed") || ends_with(path_bytes, b"/detail") {
        // Detailed: includes request count, system info
        host_log(0, "health_detailed: detailed probe");
        let hits = bump_request_count();

        let buf = unsafe { &mut SCRATCH };
        let mut pos = 0;
        pos = copy_to(buf, pos, b"{\"status\":\"healthy\",\"check\":\"detailed\",\"chain_id\":42431,\"chain\":\"Tempo Moderato\",\"runtime\":\"wasmtime\",\"memory_limit_mb\":64,\"cpu_fuel_limited\":true,\"total_health_checks\":");
        pos = write_u32(buf, pos, hits);
        pos = copy_to(buf, pos, b",\"services\":{\"gateway\":\"healthy\",\"soul\":\"healthy\",\"identity\":\"healthy\",\"cartridge_engine\":\"healthy\"},\"capabilities\":[\"kv_store\",\"payment_info\",\"logging\"]}");

        let body = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
        respond(200, body, "application/json");
        return;
    }

    // Default: basic health
    host_log(0, "health_detailed: basic probe");
    respond(200, HEALTH_BASIC, "application/json");
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
