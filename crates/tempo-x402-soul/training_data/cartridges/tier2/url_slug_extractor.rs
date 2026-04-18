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

/// Extract the slug from a path like /posts/{slug}
/// Returns the segment after /posts/
fn extract_slug_from_path<'a>(path: &'a str) -> Option<&'a str> {
    let prefix = b"/posts/";
    let path_bytes = path.as_bytes();
    if path_bytes.len() <= prefix.len() {
        return None;
    }
    let mut i = 0;
    while i < prefix.len() {
        if path_bytes[i] != prefix[i] {
            return None;
        }
        i += 1;
    }
    // Slug starts after prefix, ends at next / or end of string
    let slug_start = prefix.len();
    let mut slug_end = slug_start;
    while slug_end < path_bytes.len() && path_bytes[slug_end] != b'/' && path_bytes[slug_end] != b'?' {
        slug_end += 1;
    }
    if slug_end == slug_start {
        return None;
    }
    core::str::from_utf8(&path_bytes[slug_start..slug_end]).ok()
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };
    let path = find_json_str(request, b"path").unwrap_or("/");

    host_log(0, "url_slug_extractor: extracting slug from path");

    match extract_slug_from_path(path) {
        Some(slug) => {
            let mut pos = 0;
            pos = copy_to_scratch(pos, b"{\"slug\":\"");
            pos = copy_to_scratch(pos, slug.as_bytes());
            pos = copy_to_scratch(pos, b"\",\"resolved\":true,\"post_url\":\"/posts/");
            pos = copy_to_scratch(pos, slug.as_bytes());
            pos = copy_to_scratch(pos, b"\"}");

            let result = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
            respond(200, result, "application/json");
        }
        None => {
            respond(400, r#"{"error":"missing slug","hint":"use /posts/{slug} format"}"#, "application/json");
        }
    }
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
