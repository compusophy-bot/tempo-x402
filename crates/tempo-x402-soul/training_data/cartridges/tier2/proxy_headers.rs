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

/// Extract the first IP from a comma-separated X-Forwarded-For header
fn first_ip(xff: &str) -> &str {
    let bytes = xff.as_bytes();
    let mut end = 0;
    while end < bytes.len() && bytes[end] != b',' && bytes[end] != b' ' {
        end += 1;
    }
    if let Ok(s) = core::str::from_utf8(&bytes[..end]) {
        s
    } else {
        xff
    }
}

/// Count hops in X-Forwarded-For (number of commas + 1)
fn count_hops(xff: &str) -> u32 {
    let bytes = xff.as_bytes();
    let mut count: u32 = 1;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b',' {
            count += 1;
        }
        i += 1;
    }
    count
}

fn write_u32(buf: &mut [u8], offset: usize, mut val: u32) -> usize {
    if val == 0 { buf[offset] = b'0'; return offset + 1; }
    let mut digits = [0u8; 10];
    let mut n = 0;
    while val > 0 { digits[n] = b'0' + (val % 10) as u8; val /= 10; n += 1; }
    let mut pos = offset;
    let mut i = n;
    while i > 0 { i -= 1; buf[pos] = digits[i]; pos += 1; }
    pos
}

/// Detect if an IP looks like a private/internal address
fn is_private_ip(ip: &[u8]) -> bool {
    // 10.x.x.x
    if ip.len() >= 3 && ip[0] == b'1' && ip[1] == b'0' && ip[2] == b'.' {
        return true;
    }
    // 192.168.x.x
    if contains_substr(ip, b"192.168.") {
        return true;
    }
    // 172.16-31.x.x (simplified: starts with 172.)
    if ip.len() >= 4 && ip[0] == b'1' && ip[1] == b'7' && ip[2] == b'2' && ip[3] == b'.' {
        return true;
    }
    // 127.x.x.x
    if ip.len() >= 4 && ip[0] == b'1' && ip[1] == b'2' && ip[2] == b'7' && ip[3] == b'.' {
        return true;
    }
    // ::1
    if ip == b"::1" {
        return true;
    }
    false
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };

    host_log(0, "proxy_headers: extracting client info from proxy headers");

    // Try multiple proxy header variants
    let xff = find_header_value(request, b"X-Forwarded-For")
        .or_else(|| find_header_value(request, b"x-forwarded-for"));

    let real_ip = find_header_value(request, b"X-Real-IP")
        .or_else(|| find_header_value(request, b"x-real-ip"));

    let forwarded_proto = find_header_value(request, b"X-Forwarded-Proto")
        .or_else(|| find_header_value(request, b"x-forwarded-proto"));

    let forwarded_host = find_header_value(request, b"X-Forwarded-Host")
        .or_else(|| find_header_value(request, b"x-forwarded-host"));

    let forwarded_port = find_header_value(request, b"X-Forwarded-Port")
        .or_else(|| find_header_value(request, b"x-forwarded-port"));

    // Determine client IP: prefer X-Real-IP, then first from X-Forwarded-For
    let client_ip = if let Some(ip) = real_ip {
        ip
    } else if let Some(xff_val) = xff {
        first_ip(xff_val)
    } else {
        "unknown"
    };

    let hops = if let Some(xff_val) = xff { count_hops(xff_val) } else { 0 };
    let is_proxied = xff.is_some() || real_ip.is_some();
    let is_private = is_private_ip(client_ip.as_bytes());

    let buf = unsafe { &mut SCRATCH };
    let mut pos = 0;

    pos = copy_to(buf, pos, b"{\"client_ip\":\"");
    let ip_trunc = if client_ip.len() > 45 { &client_ip.as_bytes()[..45] } else { client_ip.as_bytes() };
    pos = copy_to(buf, pos, ip_trunc);
    pos = copy_to(buf, pos, b"\",\"is_proxied\":");
    pos = copy_to(buf, pos, if is_proxied { b"true" } else { b"false" });
    pos = copy_to(buf, pos, b",\"is_private_ip\":");
    pos = copy_to(buf, pos, if is_private { b"true" } else { b"false" });
    pos = copy_to(buf, pos, b",\"proxy_hops\":");
    pos = write_u32(buf, pos, hops);

    pos = copy_to(buf, pos, b",\"protocol\":\"");
    let proto = forwarded_proto.unwrap_or("unknown");
    pos = copy_to(buf, pos, proto.as_bytes());

    pos = copy_to(buf, pos, b"\",\"forwarded_host\":\"");
    let host = forwarded_host.unwrap_or("unknown");
    pos = copy_to(buf, pos, host.as_bytes());

    pos = copy_to(buf, pos, b"\",\"forwarded_port\":\"");
    let port = forwarded_port.unwrap_or("unknown");
    pos = copy_to(buf, pos, port.as_bytes());

    pos = copy_to(buf, pos, b"\"");

    if let Some(xff_val) = xff {
        pos = copy_to(buf, pos, b",\"x_forwarded_for\":\"");
        let xff_trunc = if xff_val.len() > 256 { &xff_val.as_bytes()[..256] } else { xff_val.as_bytes() };
        pos = copy_to(buf, pos, xff_trunc);
        pos = copy_to(buf, pos, b"\"");
    }

    pos = copy_to(buf, pos, b",\"trust_warning\":");
    if hops > 5 {
        pos = copy_to(buf, pos, b"\"Unusually long proxy chain detected\"");
    } else if is_private {
        pos = copy_to(buf, pos, b"\"Client IP is a private address\"");
    } else {
        pos = copy_to(buf, pos, b"null");
    }

    pos = copy_to(buf, pos, b"}");

    host_log(0, "proxy_headers: client info extracted");
    let body = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
    respond(200, body, "application/json");
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
