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

/// Extract the body object from the request envelope
fn find_body_region(json: &[u8]) -> Option<&[u8]> {
    let body_tag = b"\"body\"";
    let mut i = 0;
    while i + body_tag.len() < json.len() {
        if &json[i..i + body_tag.len()] == &body_tag[..] {
            let mut j = i + body_tag.len();
            while j < json.len() && (json[j] == b':' || json[j] == b' ') {
                j += 1;
            }
            if j < json.len() && json[j] == b'{' {
                let start = j;
                let mut depth = 0;
                let mut k = j;
                while k < json.len() {
                    if json[k] == b'{' { depth += 1; }
                    if json[k] == b'}' {
                        depth -= 1;
                        if depth == 0 {
                            return Some(&json[start..k + 1]);
                        }
                    }
                    k += 1;
                }
            }
        }
        i += 1;
    }
    None
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

// Known webhook event types
const VALID_EVENTS: &[&[u8]] = &[
    b"payment.completed",
    b"payment.failed",
    b"transfer.initiated",
    b"transfer.completed",
    b"identity.created",
    b"identity.updated",
    b"cartridge.deployed",
    b"cartridge.error",
    b"clone.spawned",
    b"clone.terminated",
];

fn is_valid_event(event_type: &[u8]) -> bool {
    let mut i = 0;
    while i < VALID_EVENTS.len() {
        if event_type == VALID_EVENTS[i] {
            return true;
        }
        i += 1;
    }
    false
}

/// Simple counter stored in KV to track received webhooks
fn increment_counter() -> u32 {
    let key = b"webhook_count";
    let prev = unsafe { kv_get(key.as_ptr(), key.len() as i32) };
    let count = if prev > 0 {
        // Read the stored value from SCRATCH
        let len = (prev & 0xFFFFFFFF) as usize;
        let ptr = ((prev >> 32) & 0xFFFFFFFF) as usize;
        // Parse number from stored bytes
        let stored = unsafe { &SCRATCH[..len] };
        let mut val: u32 = 0;
        let mut k = 0;
        while k < stored.len() {
            if stored[k] >= b'0' && stored[k] <= b'9' {
                val = val * 10 + (stored[k] - b'0') as u32;
            }
            k += 1;
        }
        val + 1
    } else {
        1
    };

    // Store new count
    let buf = unsafe { &mut SCRATCH[65536..] };
    let mut digits = [0u8; 10];
    let mut c = count;
    let mut n = 0;
    if c == 0 {
        digits[0] = b'0';
        n = 1;
    } else {
        while c > 0 {
            digits[n] = b'0' + (c % 10) as u8;
            c /= 10;
            n += 1;
        }
    }
    // Reverse
    let mut k = 0;
    while k < n {
        buf[k] = digits[n - 1 - k];
        k += 1;
    }
    unsafe { kv_set(key.as_ptr(), key.len() as i32, buf.as_ptr(), n as i32); }
    count
}

fn write_u32(buf: &mut [u8], offset: usize, mut val: u32) -> usize {
    if val == 0 {
        buf[offset] = b'0';
        return offset + 1;
    }
    let mut digits = [0u8; 10];
    let mut count = 0;
    while val > 0 {
        digits[count] = b'0' + (val % 10) as u8;
        val /= 10;
        count += 1;
    }
    let mut pos = offset;
    let mut i = count;
    while i > 0 {
        i -= 1;
        buf[pos] = digits[i];
        pos += 1;
    }
    pos
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };

    host_log(0, "webhook_receiver: processing incoming webhook");

    // Must be POST
    let method = find_json_str(request, b"method").unwrap_or("GET");
    if !contains_substr(method.as_bytes(), b"POST") {
        respond(405, r#"{"error":"method_not_allowed","message":"Webhooks must be POST"}"#, "application/json");
        return;
    }

    // Check Content-Type
    let ct = find_header_value(request, b"Content-Type")
        .or_else(|| find_header_value(request, b"content-type"))
        .unwrap_or("");
    if !contains_substr(ct.as_bytes(), b"json") {
        respond(415, r#"{"error":"unsupported_media_type","message":"Content-Type must be application/json"}"#, "application/json");
        return;
    }

    // Extract body
    let body = match find_body_region(request) {
        Some(b) => b,
        None => {
            respond(400, r#"{"error":"missing_body","message":"Webhook body is required"}"#, "application/json");
            return;
        }
    };

    // Extract event type
    let event_type = match find_json_str(body, b"event") {
        Some(e) => e,
        None => {
            respond(400, r#"{"error":"missing_event","message":"Field 'event' is required in webhook body"}"#, "application/json");
            return;
        }
    };

    // Validate event type
    if !is_valid_event(event_type.as_bytes()) {
        let buf = unsafe { &mut SCRATCH };
        let mut pos = 0;
        pos = copy_to(buf, pos, b"{\"error\":\"unknown_event\",\"message\":\"Unrecognized event type: ");
        let ev_trunc = if event_type.len() > 64 { &event_type.as_bytes()[..64] } else { event_type.as_bytes() };
        pos = copy_to(buf, pos, ev_trunc);
        pos = copy_to(buf, pos, b"\"}");
        let resp = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
        respond(400, resp, "application/json");
        return;
    }

    // Extract optional webhook ID for idempotency
    let webhook_id = find_json_str(body, b"id").unwrap_or("none");

    // Increment counter
    let count = increment_counter();

    host_log(0, "webhook_receiver: event accepted");

    let buf = unsafe { &mut SCRATCH };
    let mut pos = 0;
    pos = copy_to(buf, pos, b"{\"status\":\"accepted\",\"event\":\"");
    pos = copy_to(buf, pos, event_type.as_bytes());
    pos = copy_to(buf, pos, b"\",\"webhook_id\":\"");
    pos = copy_to(buf, pos, webhook_id.as_bytes());
    pos = copy_to(buf, pos, b"\",\"sequence\":");
    pos = write_u32(buf, pos, count);
    pos = copy_to(buf, pos, b",\"message\":\"Webhook received and queued for processing\"}");

    let resp = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
    respond(202, resp, "application/json");
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
