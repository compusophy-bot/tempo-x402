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

struct RedirectRule {
    from: &'static [u8],
    to: &'static str,
    permanent: bool,
}

const REDIRECTS: &[RedirectRule] = &[
    RedirectRule { from: b"/old-path", to: "/new-path", permanent: true },
    RedirectRule { from: b"/blog", to: "/posts", permanent: true },
    RedirectRule { from: b"/dashboard", to: "/app/dashboard", permanent: false },
    RedirectRule { from: b"/docs", to: "/api/v1/docs", permanent: true },
    RedirectRule { from: b"/login", to: "/auth/login", permanent: false },
    RedirectRule { from: b"/v1", to: "/api/v1", permanent: true },
];

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };
    let path = find_json_str(request, b"path").unwrap_or("/");
    let path_bytes = path.as_bytes();

    host_log(0, "redirect_handler: checking redirect rules");

    let mut i = 0;
    while i < REDIRECTS.len() {
        if bytes_equal(path_bytes, REDIRECTS[i].from) {
            let status = if REDIRECTS[i].permanent { 301 } else { 302 };
            let status_text = if REDIRECTS[i].permanent { "moved permanently" } else { "found" };

            let mut pos = 0;
            pos = copy_to_scratch(pos, b"{\"redirect\":true,\"from\":\"");
            pos = copy_to_scratch(pos, path_bytes);
            pos = copy_to_scratch(pos, b"\",\"to\":\"");
            pos = copy_to_scratch(pos, REDIRECTS[i].to.as_bytes());
            pos = copy_to_scratch(pos, b"\",\"status\":\"");
            pos = copy_to_scratch(pos, status_text.as_bytes());
            pos = copy_to_scratch(pos, b"\",\"location\":\"");
            pos = copy_to_scratch(pos, REDIRECTS[i].to.as_bytes());
            pos = copy_to_scratch(pos, b"\"}");

            let result = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
            host_log(0, "redirect_handler: redirect matched");
            respond(status, result, "application/json");
            return;
        }
        i += 1;
    }

    // No redirect matched — serve normally
    respond(200, r#"{"message":"no redirect for this path","path":"served directly"}"#, "application/json");
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
