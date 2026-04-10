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

/// Search for a header value inside the nested "headers" object
fn find_header_value<'a>(json: &'a [u8], header_key: &[u8]) -> Option<&'a str> {
    // Find "headers" key first, then search within for the header_key
    let headers_tag = b"\"headers\"";
    let mut i = 0;
    while i + headers_tag.len() < json.len() {
        if &json[i..i + headers_tag.len()] == &headers_tag[..] {
            // Found headers, now search after this for the header key
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

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

const HTML_RESPONSE: &str = "<!DOCTYPE html>\n<html>\n<head><title>Content Negotiation</title></head>\n<body>\n<h1>Content Negotiation Demo</h1>\n<p>You requested HTML content.</p>\n<ul>\n<li>Status: operational</li>\n<li>Version: 1.0.0</li>\n<li>Chain: Tempo Moderato (42431)</li>\n</ul>\n</body>\n</html>";

const JSON_RESPONSE: &str = r#"{"status":"operational","version":"1.0.0","chain":"Tempo Moderato","chain_id":42431,"format":"json"}"#;

const TEXT_RESPONSE: &str = "Content Negotiation Demo\nStatus: operational\nVersion: 1.0.0\nChain: Tempo Moderato (42431)\nFormat: text/plain";

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };

    host_log(0, "content_negotiation: checking Accept header");

    let accept = find_header_value(request, b"Accept")
        .or_else(|| find_header_value(request, b"accept"))
        .unwrap_or("application/json");

    let accept_bytes = accept.as_bytes();

    if contains_substr(accept_bytes, b"text/html") {
        host_log(0, "content_negotiation: serving HTML");
        respond(200, HTML_RESPONSE, "text/html");
    } else if contains_substr(accept_bytes, b"text/plain") {
        host_log(0, "content_negotiation: serving plain text");
        respond(200, TEXT_RESPONSE, "text/plain");
    } else {
        host_log(0, "content_negotiation: serving JSON (default)");
        respond(200, JSON_RESPONSE, "application/json");
    }
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
