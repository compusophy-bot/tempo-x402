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

const VALID_TOKEN: &[u8] = b"Bearer x402-cartridge-secret-token-2024";

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };

    host_log(0, "basic_auth: checking authorization");

    let auth = find_header_value(request, b"Authorization")
        .or_else(|| find_header_value(request, b"authorization"));

    match auth {
        Some(token) => {
            if bytes_equal(token.as_bytes(), VALID_TOKEN) {
                host_log(0, "basic_auth: authorized");
                respond(200, r#"{"authenticated":true,"user":"cartridge-admin","permissions":["read","write","execute"]}"#, "application/json");
            } else {
                host_log(1, "basic_auth: invalid token");
                respond(403, r#"{"error":"forbidden","message":"invalid token"}"#, "application/json");
            }
        }
        None => {
            host_log(1, "basic_auth: no authorization header");
            respond(401, r#"{"error":"unauthorized","message":"missing Authorization header","hint":"use Bearer x402-cartridge-secret-token-2024"}"#, "application/json");
        }
    }
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
