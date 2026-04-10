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

fn contains_substr_ci(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    let mut i = 0;
    while i + needle.len() <= haystack.len() {
        let mut matched = true;
        let mut k = 0;
        while k < needle.len() {
            let a = if haystack[i + k] >= b'A' && haystack[i + k] <= b'Z' { haystack[i + k] + 32 } else { haystack[i + k] };
            let b = if needle[k] >= b'A' && needle[k] <= b'Z' { needle[k] + 32 } else { needle[k] };
            if a != b {
                matched = false;
                break;
            }
            k += 1;
        }
        if matched {
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

fn detect_browser(ua: &[u8]) -> &'static str {
    if contains_substr_ci(ua, b"Edg/") { return "Edge"; }
    if contains_substr_ci(ua, b"OPR/") || contains_substr_ci(ua, b"Opera") { return "Opera"; }
    if contains_substr_ci(ua, b"Chrome/") { return "Chrome"; }
    if contains_substr_ci(ua, b"Safari/") && !contains_substr_ci(ua, b"Chrome") { return "Safari"; }
    if contains_substr_ci(ua, b"Firefox/") { return "Firefox"; }
    if contains_substr_ci(ua, b"curl/") { return "curl"; }
    "Unknown"
}

fn detect_os(ua: &[u8]) -> &'static str {
    if contains_substr_ci(ua, b"Windows NT 10") { return "Windows 10/11"; }
    if contains_substr_ci(ua, b"Windows") { return "Windows"; }
    if contains_substr_ci(ua, b"Mac OS X") { return "macOS"; }
    if contains_substr_ci(ua, b"Android") { return "Android"; }
    if contains_substr_ci(ua, b"iPhone") || contains_substr_ci(ua, b"iPad") { return "iOS"; }
    if contains_substr_ci(ua, b"Linux") { return "Linux"; }
    "Unknown"
}

fn detect_device(ua: &[u8]) -> &'static str {
    if contains_substr_ci(ua, b"Mobile") || contains_substr_ci(ua, b"Android") { return "mobile"; }
    if contains_substr_ci(ua, b"iPad") || contains_substr_ci(ua, b"Tablet") { return "tablet"; }
    if contains_substr_ci(ua, b"curl") || contains_substr_ci(ua, b"bot") || contains_substr_ci(ua, b"Bot") {
        return "bot";
    }
    "desktop"
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };

    host_log(0, "user_agent_parser: parsing User-Agent header");

    let ua = find_header_value(request, b"User-Agent")
        .or_else(|| find_header_value(request, b"user-agent"))
        .unwrap_or("Unknown");

    let ua_bytes = ua.as_bytes();
    let browser = detect_browser(ua_bytes);
    let os = detect_os(ua_bytes);
    let device = detect_device(ua_bytes);
    let is_bot = contains_substr_ci(ua_bytes, b"bot") || contains_substr_ci(ua_bytes, b"crawler")
        || contains_substr_ci(ua_bytes, b"spider");

    let buf = unsafe { &mut SCRATCH };
    let mut pos = 0;
    pos = copy_to(buf, pos, b"{\"browser\":\"");
    pos = copy_to(buf, pos, browser.as_bytes());
    pos = copy_to(buf, pos, b"\",\"os\":\"");
    pos = copy_to(buf, pos, os.as_bytes());
    pos = copy_to(buf, pos, b"\",\"device_type\":\"");
    pos = copy_to(buf, pos, device.as_bytes());
    pos = copy_to(buf, pos, b"\",\"is_bot\":");
    pos = copy_to(buf, pos, if is_bot { b"true" } else { b"false" });
    pos = copy_to(buf, pos, b",\"raw\":\"");
    // Truncate raw UA to prevent buffer overflow
    let ua_trunc = if ua.len() > 256 { &ua.as_bytes()[..256] } else { ua.as_bytes() };
    pos = copy_to(buf, pos, ua_trunc);
    pos = copy_to(buf, pos, b"\"}");

    let body = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
    respond(200, body, "application/json");
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
