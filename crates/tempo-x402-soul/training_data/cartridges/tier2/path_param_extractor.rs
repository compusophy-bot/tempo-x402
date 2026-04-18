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

/// Extract numeric ID from path like /items/123 or /items/123/details
fn extract_numeric_id(path: &[u8], prefix: &[u8]) -> Option<usize> {
    if path.len() <= prefix.len() {
        return None;
    }
    // Check prefix matches
    let mut i = 0;
    while i < prefix.len() {
        if path[i] != prefix[i] {
            return None;
        }
        i += 1;
    }
    // Now parse digits
    let mut id: usize = 0;
    let mut found_digit = false;
    while i < path.len() && path[i] >= b'0' && path[i] <= b'9' {
        id = id * 10 + (path[i] - b'0') as usize;
        found_digit = true;
        i += 1;
    }
    if found_digit {
        Some(id)
    } else {
        None
    }
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };
    let path = find_json_str(request, b"path").unwrap_or("/");
    let method = find_json_str(request, b"method").unwrap_or("GET");
    let path_bytes = path.as_bytes();

    host_log(0, "path_param_extractor: extracting numeric ID from path");

    match extract_numeric_id(path_bytes, b"/items/") {
        Some(id) => {
            let mut pos = 0;
            pos = copy_to_scratch(pos, b"{\"resource\":\"item\",\"id\":");
            pos = write_usize(pos, id);
            pos = copy_to_scratch(pos, b",\"method\":\"");
            pos = copy_to_scratch(pos, method.as_bytes());
            pos = copy_to_scratch(pos, b"\",\"found\":true,\"url\":\"/items/");
            pos = write_usize(pos, id);
            pos = copy_to_scratch(pos, b"\"}");

            let result = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
            respond(200, result, "application/json");
        }
        None => {
            // Try /users/ prefix as well
            match extract_numeric_id(path_bytes, b"/users/") {
                Some(id) => {
                    let mut pos = 0;
                    pos = copy_to_scratch(pos, b"{\"resource\":\"user\",\"id\":");
                    pos = write_usize(pos, id);
                    pos = copy_to_scratch(pos, b",\"method\":\"");
                    pos = copy_to_scratch(pos, method.as_bytes());
                    pos = copy_to_scratch(pos, b"\",\"found\":true}");

                    let result = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
                    respond(200, result, "application/json");
                }
                None => {
                    respond(400, r#"{"error":"no numeric ID found","hint":"use /items/{id} or /users/{id}","examples":["/items/42","/users/7"]}"#, "application/json");
                }
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
