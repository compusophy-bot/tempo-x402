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

fn find_header_value<'a>(json: &'a [u8], header_key: &[u8]) -> Option<&'a str> {
    let headers_tag = b"\"headers\"";
    let mut i = 0;
    while i + headers_tag.len() < json.len() {
        if &json[i..i + headers_tag.len()] == &headers_tag[..] {
            let region_start = i + headers_tag.len();
            let region = &json[region_start..];
            return find_json_str(region, header_key);
        }
        i += 1;
    }
    None
}

/// Parse a decimal string into usize
fn parse_usize(s: &[u8]) -> Option<usize> {
    if s.is_empty() {
        return None;
    }
    let mut result: usize = 0;
    let mut i = 0;
    while i < s.len() {
        if s[i] < b'0' || s[i] > b'9' {
            return None;
        }
        result = result * 10 + (s[i] - b'0') as usize;
        i += 1;
    }
    Some(result)
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

fn copy_to_scratch(offset: usize, src: &[u8]) -> usize {
    unsafe {
        let mut i = 0;
        while i < src.len() && offset + i < SCRATCH.len() {
            SCRATCH[offset + i] = src[i];
            i += 1;
        }
        offset + i
    }
}

fn write_usize(offset: usize, value: usize) -> usize {
    if value == 0 {
        return copy_to_scratch(offset, b"0");
    }
    let mut digits: [u8; 20] = [0u8; 20];
    let mut d = 0;
    let mut n = value;
    while n > 0 {
        digits[d] = b'0' + (n % 10) as u8;
        n /= 10;
        d += 1;
    }
    let mut pos = offset;
    let mut k = d;
    while k > 0 {
        k -= 1;
        pos = copy_to_scratch(pos, &digits[k..k + 1]);
    }
    pos
}

const RATE_LIMIT: usize = 100;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };

    host_log(0, "rate_limit_checker: checking rate limit headers");

    // Check X-Rate-Limit-Remaining header (set by upstream proxy/gateway)
    let remaining_str = find_header_value(request, b"X-Rate-Limit-Remaining")
        .or_else(|| find_header_value(request, b"x-rate-limit-remaining"));

    // Check X-Rate-Limit-Count header (requests made so far)
    let count_str = find_header_value(request, b"X-Rate-Limit-Count")
        .or_else(|| find_header_value(request, b"x-rate-limit-count"));

    // Determine if rate limited
    let current_count = match count_str {
        Some(s) => parse_usize(s.as_bytes()).unwrap_or(0),
        None => 0,
    };

    let remaining = match remaining_str {
        Some(s) => parse_usize(s.as_bytes()).unwrap_or(RATE_LIMIT),
        None => {
            // If no remaining header, compute from count
            if current_count >= RATE_LIMIT { 0 } else { RATE_LIMIT - current_count }
        }
    };

    if remaining == 0 || current_count >= RATE_LIMIT {
        host_log(1, "rate_limit_checker: rate limit exceeded");

        let mut pos = 0;
        pos = copy_to_scratch(pos, b"{\"error\":\"rate limit exceeded\",\"status\":429,\"limit\":");
        pos = write_usize(pos, RATE_LIMIT);
        pos = copy_to_scratch(pos, b",\"used\":");
        pos = write_usize(pos, current_count);
        pos = copy_to_scratch(pos, b",\"remaining\":0,\"retry_after_seconds\":60}");

        let result = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
        respond(429, result, "application/json");
    } else {
        host_log(0, "rate_limit_checker: request allowed");

        let mut pos = 0;
        pos = copy_to_scratch(pos, b"{\"allowed\":true,\"limit\":");
        pos = write_usize(pos, RATE_LIMIT);
        pos = copy_to_scratch(pos, b",\"used\":");
        pos = write_usize(pos, current_count);
        pos = copy_to_scratch(pos, b",\"remaining\":");
        pos = write_usize(pos, remaining);
        pos = copy_to_scratch(pos, b",\"message\":\"request processed successfully\"}");

        let result = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
        respond(200, result, "application/json");
    }
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
