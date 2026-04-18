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

fn parse_u64(s: &str) -> u64 {
    let bytes = s.as_bytes();
    let mut result: u64 = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] >= b'0' && bytes[i] <= b'9' {
            result = result * 10 + (bytes[i] - b'0') as u64;
        }
        i += 1;
    }
    result
}

fn write_u64(buf: &mut [u8], val: u64) -> usize {
    if val == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 20];
    let mut n = val;
    let mut len = 0;
    while n > 0 {
        tmp[len] = b'0' + (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    let mut i = 0;
    while i < len {
        buf[i] = tmp[len - 1 - i];
        i += 1;
    }
    len
}

/// Simple hash: generate a short code from a counter
const CODE_CHARS: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";

fn make_code(id: u64, buf: &mut [u8]) -> usize {
    let mut n = id;
    let base = CODE_CHARS.len() as u64;
    let mut len = 0;
    if n == 0 {
        buf[0] = CODE_CHARS[0];
        return 1;
    }
    // Generate at least 4 chars
    let mut tmp = [0u8; 8];
    let mut tlen = 0;
    while n > 0 && tlen < 8 {
        tmp[tlen] = CODE_CHARS[(n % base) as usize];
        n /= base;
        tlen += 1;
    }
    // Pad to 4 chars minimum
    while tlen < 4 {
        tmp[tlen] = CODE_CHARS[(tlen as u64 * 7 % base) as usize];
        tlen += 1;
    }
    // Reverse
    let mut i = 0;
    while i < tlen {
        buf[i] = tmp[tlen - 1 - i];
        i += 1;
    }
    tlen
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
    let path_bytes = path.as_bytes();

    host_log(0, "short_url: handling request");

    // POST /shorten — create short URL
    if bytes_equal(method.as_bytes(), b"POST") && bytes_equal(path_bytes, b"/shorten") {
        let url = find_json_str(body.as_bytes(), b"url").unwrap_or(body);

        if url.is_empty() {
            respond(400, r#"{"error":"url is required"}"#, "application/json");
            return;
        }

        // Get next ID
        let count = match kv_read("url_count") {
            Some(s) => parse_u64(s),
            None => 0,
        };
        let new_id = count + 1;

        // Generate short code
        let mut code_buf = [0u8; 8];
        let code_len = make_code(new_id, &mut code_buf);
        let code = unsafe { core::str::from_utf8(&code_buf[..code_len]).unwrap_or("aaaa") };

        // Store: "short_{code}" -> url
        let mut key_buf = [0u8; 32];
        let mut kp = 0;
        let prefix = b"short_";
        let mut ki = 0;
        while ki < prefix.len() { key_buf[kp] = prefix[ki]; kp += 1; ki += 1; }
        ki = 0;
        while ki < code_len { key_buf[kp] = code_buf[ki]; kp += 1; ki += 1; }
        let short_key = unsafe { core::str::from_utf8(&key_buf[..kp]).unwrap_or("") };

        kv_write(short_key, url);

        // Update count
        let mut num_buf = [0u8; 20];
        let num_len = write_u64(&mut num_buf, new_id);
        let count_str = unsafe { core::str::from_utf8(&num_buf[..num_len]).unwrap_or("0") };
        kv_write("url_count", count_str);

        // Also store visit count for the code
        let mut vc_key = [0u8; 40];
        let mut vcp = 0;
        let vc_prefix = b"visits_";
        ki = 0;
        while ki < vc_prefix.len() { vc_key[vcp] = vc_prefix[ki]; vcp += 1; ki += 1; }
        ki = 0;
        while ki < code_len { vc_key[vcp] = code_buf[ki]; vcp += 1; ki += 1; }
        let visits_key = unsafe { core::str::from_utf8(&vc_key[..vcp]).unwrap_or("") };
        kv_write(visits_key, "0");

        let mut pos = 0;
        pos = append(pos, b"{\"code\":\"");
        pos = append(pos, &code_buf[..code_len]);
        pos = append(pos, b"\",\"short_path\":\"/s/");
        pos = append(pos, &code_buf[..code_len]);
        pos = append(pos, b"\",\"url\":\"");
        pos = append(pos, url.as_bytes());
        pos = append(pos, b"\"}");
        unsafe {
            let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
            respond(201, resp, "application/json");
        }
        return;
    }

    // GET /s/{code} — redirect
    if bytes_equal(method.as_bytes(), b"GET") && starts_with(path_bytes, b"/s/") {
        let code = unsafe { core::str::from_utf8(&path_bytes[3..]).unwrap_or("") };

        // Look up short_{code}
        let mut key_buf = [0u8; 32];
        let mut kp = 0;
        let prefix = b"short_";
        let mut ki = 0;
        while ki < prefix.len() { key_buf[kp] = prefix[ki]; kp += 1; ki += 1; }
        let cb = code.as_bytes();
        ki = 0;
        while ki < cb.len() && kp < key_buf.len() { key_buf[kp] = cb[ki]; kp += 1; ki += 1; }
        let short_key = unsafe { core::str::from_utf8(&key_buf[..kp]).unwrap_or("") };

        match kv_read(short_key) {
            Some(url) => {
                // Increment visit count
                let mut vc_key = [0u8; 40];
                let mut vcp = 0;
                let vc_prefix = b"visits_";
                let mut vi = 0;
                while vi < vc_prefix.len() { vc_key[vcp] = vc_prefix[vi]; vcp += 1; vi += 1; }
                vi = 0;
                while vi < cb.len() && vcp < vc_key.len() { vc_key[vcp] = cb[vi]; vcp += 1; vi += 1; }
                let visits_key = unsafe { core::str::from_utf8(&vc_key[..vcp]).unwrap_or("") };
                let visits = match kv_read(visits_key) {
                    Some(s) => parse_u64(s),
                    None => 0,
                };
                let new_visits = visits + 1;
                let mut vn_buf = [0u8; 20];
                let vn_len = write_u64(&mut vn_buf, new_visits);
                let vn_str = unsafe { core::str::from_utf8(&vn_buf[..vn_len]).unwrap_or("0") };
                kv_write(visits_key, vn_str);

                // Return redirect HTML (meta refresh + JS)
                let mut pos = 0;
                pos = append(pos, b"<html><head><meta http-equiv=\"refresh\" content=\"0;url=");
                pos = append(pos, url.as_bytes());
                pos = append(pos, b"\"><script>window.location.href=\"");
                pos = append(pos, url.as_bytes());
                pos = append(pos, b"\";</script></head><body>Redirecting to <a href=\"");
                pos = append(pos, url.as_bytes());
                pos = append(pos, b"\">");
                pos = append(pos, url.as_bytes());
                pos = append(pos, b"</a>...</body></html>");
                unsafe {
                    let body = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("error");
                    respond(200, body, "text/html");
                }
            }
            None => {
                respond(404, r#"{"error":"short URL not found"}"#, "application/json");
            }
        }
        return;
    }

    // Default: usage info
    respond(200, r#"{"service":"url-shortener","endpoints":["POST /shorten {\"url\":\"...\"}","GET /s/{code}"]}"#, "application/json");
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
