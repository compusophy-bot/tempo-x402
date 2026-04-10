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

/// Simple FNV-1a hash to compute an ETag from content
fn fnv1a_hash(data: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c9dc5;
    let mut i = 0;
    while i < data.len() {
        hash ^= data[i] as u32;
        hash = hash.wrapping_mul(0x01000193);
        i += 1;
    }
    hash
}

fn u32_to_hex(value: u32, buf: &mut [u8; 8]) {
    let hex = b"0123456789abcdef";
    let mut i = 0;
    while i < 8 {
        buf[7 - i] = hex[((value >> (i * 4)) & 0xf) as usize];
        i += 1;
    }
}

const RESOURCE_BODY: &str = r#"{"id":"tempo-42431","name":"Tempo Moderato","chain_id":42431,"token":"pathUSD","status":"active","block_time_ms":2000,"version":"1.0.0"}"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };

    host_log(0, "conditional_get: checking If-None-Match");

    // Compute ETag of the resource
    let hash = fnv1a_hash(RESOURCE_BODY.as_bytes());
    let mut hex_buf = [0u8; 8];
    u32_to_hex(hash, &mut hex_buf);

    // Build ETag string: "abcdef01"
    let buf = unsafe { &mut SCRATCH };
    let mut etag_pos = 0;
    buf[etag_pos] = b'"';
    etag_pos += 1;
    let mut k = 0;
    while k < 8 {
        buf[etag_pos] = hex_buf[k];
        etag_pos += 1;
        k += 1;
    }
    buf[etag_pos] = b'"';
    etag_pos += 1;

    let etag = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..etag_pos]) };

    // Check If-None-Match header
    let if_none_match = find_header_value(request, b"If-None-Match")
        .or_else(|| find_header_value(request, b"if-none-match"));

    if let Some(client_etag) = if_none_match {
        // Check if client's etag contains our etag (handles quoted and unquoted)
        let client_bytes = client_etag.as_bytes();
        let our_hex = &hex_buf[..];
        let mut found = false;
        if client_bytes.len() >= 8 {
            let mut i = 0;
            while i + 8 <= client_bytes.len() {
                if &client_bytes[i..i + 8] == our_hex {
                    found = true;
                    break;
                }
                i += 1;
            }
        }

        if found {
            host_log(0, "conditional_get: 304 Not Modified");
            // Return 304 with ETag info but empty body
            let mut pos = 1024; // offset to avoid etag area
            pos = copy_to(buf, pos, b"{\"status\":304,\"etag\":");
            pos = copy_to(buf, pos, etag.as_bytes());
            pos = copy_to(buf, pos, b",\"message\":\"Not Modified\"}");
            let body = unsafe { core::str::from_utf8_unchecked(&SCRATCH[1024..pos]) };
            respond(304, body, "application/json");
            return;
        }
    }

    // Full response with ETag
    host_log(0, "conditional_get: 200 OK with ETag");
    let mut pos = 1024;
    pos = copy_to(buf, pos, b"{\"etag\":");
    pos = copy_to(buf, pos, etag.as_bytes());
    pos = copy_to(buf, pos, b",\"data\":");
    pos = copy_to(buf, pos, RESOURCE_BODY.as_bytes());
    pos = copy_to(buf, pos, b"}");
    let body = unsafe { core::str::from_utf8_unchecked(&SCRATCH[1024..pos]) };
    respond(200, body, "application/json");
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
