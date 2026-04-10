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

fn starts_with(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    &haystack[..needle.len()] == needle
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

fn copy_to(buf: &mut [u8], offset: usize, src: &[u8]) -> usize {
    let end = if offset + src.len() > buf.len() { buf.len() } else { offset + src.len() };
    let mut i = offset;
    while i < end {
        buf[i] = src[i - offset];
        i += 1;
    }
    end
}

/// Extracts version from path (e.g., /v1/users -> 1) or from API-Version header
fn detect_version(path: &[u8], request: &[u8]) -> u8 {
    // Check path first: /v1/... or /v2/...
    if path.len() >= 3 && path[0] == b'/' && path[1] == b'v' && path[2] >= b'1' && path[2] <= b'9' {
        return path[2] - b'0';
    }
    // Fall back to API-Version header
    if let Some(ver) = find_header_value(request, b"API-Version")
        .or_else(|| find_header_value(request, b"api-version"))
    {
        let vb = ver.as_bytes();
        if !vb.is_empty() && vb[0] >= b'1' && vb[0] <= b'9' {
            return vb[0] - b'0';
        }
    }
    1 // default to v1
}

/// Extract resource name from versioned path: /v1/users -> users
fn extract_resource<'a>(path: &'a [u8]) -> &'a str {
    // Skip /vN/
    if path.len() >= 4 && path[0] == b'/' && path[1] == b'v' && path[2] >= b'1' && path[2] <= b'9' && path[3] == b'/' {
        let rest = &path[4..];
        // Find end of resource name (next / or end)
        let mut end = 0;
        while end < rest.len() && rest[end] != b'/' && rest[end] != b'?' {
            end += 1;
        }
        if let Ok(s) = core::str::from_utf8(&rest[..end]) {
            return s;
        }
    }
    "status"
}

const V1_USERS: &str = r#"{"version":1,"format":"basic","users":["alice","bob","charlie"],"total":3}"#;
const V2_USERS: &str = r#"{"version":2,"format":"detailed","data":{"users":[{"id":1,"name":"alice","role":"admin"},{"id":2,"name":"bob","role":"user"},{"id":3,"name":"charlie","role":"user"}],"total":3,"page":1,"has_more":false}}"#;
const V1_STATUS: &str = r#"{"version":1,"status":"ok"}"#;
const V2_STATUS: &str = r#"{"version":2,"status":"operational","uptime_seconds":86400,"chain_id":42431,"services":{"gateway":"healthy","soul":"healthy","identity":"healthy"}}"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };

    host_log(0, "api_versioning: detecting API version");

    let path = find_json_str(request, b"path").unwrap_or("/v1/status");
    let path_bytes = path.as_bytes();
    let version = detect_version(path_bytes, request);
    let resource = extract_resource(path_bytes);

    if version > 2 {
        let buf = unsafe { &mut SCRATCH };
        let mut pos = 0;
        pos = copy_to(buf, pos, b"{\"error\":\"unsupported_version\",\"message\":\"API version ");
        buf[pos] = version + b'0';
        pos += 1;
        pos = copy_to(buf, pos, b" is not supported. Use v1 or v2.\",\"supported\":[1,2]}");
        let body = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
        respond(400, body, "application/json");
        return;
    }

    let resource_bytes = resource.as_bytes();
    let body = if contains_substr(resource_bytes, b"user") {
        if version == 2 { V2_USERS } else { V1_USERS }
    } else {
        if version == 2 { V2_STATUS } else { V1_STATUS }
    };

    host_log(0, if version == 2 { "api_versioning: serving v2" } else { "api_versioning: serving v1" });
    respond(200, body, "application/json");
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
