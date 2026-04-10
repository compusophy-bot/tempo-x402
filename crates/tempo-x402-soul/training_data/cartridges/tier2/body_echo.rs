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

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };
    let method = find_json_str(request, b"method").unwrap_or("GET");
    let body = find_json_str(request, b"body").unwrap_or("");

    host_log(0, "body_echo: reading request body");

    // Only accept POST/PUT with body content
    let is_post = method.as_bytes() == b"POST";
    let is_put = method.as_bytes() == b"PUT";

    if !is_post && !is_put {
        respond(400, r#"{"error":"body_echo requires POST or PUT method","received_method":"see logs"}"#, "application/json");
        return;
    }

    if body.is_empty() {
        respond(400, r#"{"error":"empty body","hint":"send data in the request body"}"#, "application/json");
        return;
    }

    // Build: {"received":"...","length":N,"method":"POST"}
    let mut pos = 0;
    pos = copy_to_scratch(pos, b"{\"received\":\"");
    pos = copy_to_scratch(pos, body.as_bytes());
    pos = copy_to_scratch(pos, b"\",\"length\":");
    pos = write_usize(pos, body.len());
    pos = copy_to_scratch(pos, b",\"method\":\"");
    pos = copy_to_scratch(pos, method.as_bytes());
    pos = copy_to_scratch(pos, b"\"}");

    let result = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
    respond(200, result, "application/json");
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
