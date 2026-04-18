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

fn kv_read(key: &str) -> Option<&'static str> {
    unsafe {
        let result = kv_get(key.as_ptr(), key.len() as i32);
        if result < 0 { return None; }
        let ptr = (result >> 32) as *const u8;
        let len = (result & 0xFFFFFFFF) as usize;
        let bytes = core::slice::from_raw_parts(ptr, len);
        core::str::from_utf8(bytes).ok()
    }
}

fn kv_write(key: &str, value: &str) {
    unsafe {
        kv_set(key.as_ptr(), key.len() as i32, value.as_ptr(), value.len() as i32);
    }
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
                while j < json.len() && (json[j] == b':' || json[j] == b' ') { j += 1; }
                if j < json.len() && json[j] == b'"' {
                    let val_start = j + 1;
                    let mut val_end = val_start;
                    while val_end < json.len() && json[val_end] != b'"' { val_end += 1; }
                    return core::str::from_utf8(&json[val_start..val_end]).ok();
                }
            }
        }
        i += 1;
    }
    None
}

fn starts_with(haystack: &[u8], needle: &[u8]) -> bool {
    if haystack.len() < needle.len() { return false; }
    let mut i = 0;
    while i < needle.len() {
        if haystack[i] != needle[i] { return false; }
        i += 1;
    }
    true
}

fn bytes_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] { return false; }
        i += 1;
    }
    true
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

fn append(pos: usize, data: &[u8]) -> usize {
    unsafe {
        let mut i = 0;
        while i < data.len() && pos + i < SCRATCH.len() {
            SCRATCH[pos + i] = data[i];
            i += 1;
        }
        pos + i
    }
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };
    let method = find_json_str(request, b"method").unwrap_or("GET");
    let path = find_json_str(request, b"path").unwrap_or("/");
    let body = find_json_str(request, b"body").unwrap_or("");

    host_log(0, "key_value_api: handling request");

    let path_bytes = path.as_bytes();

    // Path format: /kv/{key}
    if !starts_with(path_bytes, b"/kv/") {
        respond(200, r#"{"service":"key-value store","usage":"GET /kv/{key} to read, POST /kv/{key} with body {\"value\":\"...\"} to write"}"#, "application/json");
        return;
    }

    let key = unsafe { core::str::from_utf8(&path_bytes[4..]).unwrap_or("") };
    if key.is_empty() {
        respond(400, r#"{"error":"key is required"}"#, "application/json");
        return;
    }

    if bytes_equal(method.as_bytes(), b"GET") {
        match kv_read(key) {
            Some(value) => {
                let mut pos = 0;
                pos = append(pos, b"{\"key\":\"");
                pos = append(pos, key.as_bytes());
                pos = append(pos, b"\",\"value\":\"");
                pos = append(pos, value.as_bytes());
                pos = append(pos, b"\"}");
                unsafe {
                    let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
                    respond(200, resp, "application/json");
                }
            }
            None => {
                let mut pos = 0;
                pos = append(pos, b"{\"error\":\"key not found\",\"key\":\"");
                pos = append(pos, key.as_bytes());
                pos = append(pos, b"\"}");
                unsafe {
                    let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
                    respond(404, resp, "application/json");
                }
            }
        }
    } else if bytes_equal(method.as_bytes(), b"POST") {
        let value = find_json_str(body.as_bytes(), b"value").unwrap_or(body);
        kv_write(key, value);
        let mut pos = 0;
        pos = append(pos, b"{\"stored\":true,\"key\":\"");
        pos = append(pos, key.as_bytes());
        pos = append(pos, b"\",\"value\":\"");
        pos = append(pos, value.as_bytes());
        pos = append(pos, b"\"}");
        unsafe {
            let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
            respond(200, resp, "application/json");
        }
    } else if bytes_equal(method.as_bytes(), b"DELETE") {
        kv_write(key, "");
        let mut pos = 0;
        pos = append(pos, b"{\"deleted\":true,\"key\":\"");
        pos = append(pos, key.as_bytes());
        pos = append(pos, b"\"}");
        unsafe {
            let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
            respond(200, resp, "application/json");
        }
    } else {
        respond(405, r#"{"error":"method not allowed"}"#, "application/json");
    }
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
