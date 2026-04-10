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

fn bytes_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] { return false; }
        i += 1;
    }
    true
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

fn make_bm_key(id: u64) -> &'static str {
    static mut BK_BUF: [u8; 32] = [0u8; 32];
    unsafe {
        let prefix = b"bm_";
        let mut kp = 0;
        let mut ki = 0;
        while ki < prefix.len() { BK_BUF[kp] = prefix[ki]; kp += 1; ki += 1; }
        let mut num_buf = [0u8; 20];
        let num_len = write_u64(&mut num_buf, id);
        ki = 0;
        while ki < num_len { BK_BUF[kp] = num_buf[ki]; kp += 1; ki += 1; }
        core::str::from_utf8(&BK_BUF[..kp]).unwrap_or("bm_0")
    }
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };
    let method = find_json_str(request, b"method").unwrap_or("GET");
    let path = find_json_str(request, b"path").unwrap_or("/");
    let body = find_json_str(request, b"body").unwrap_or("");
    let path_bytes = path.as_bytes();

    host_log(0, "bookmark_store: handling request");

    // POST /bookmarks — save a new bookmark
    if bytes_equal(method.as_bytes(), b"POST") && bytes_equal(path_bytes, b"/bookmarks") {
        let url = find_json_str(body.as_bytes(), b"url").unwrap_or("");
        let title = find_json_str(body.as_bytes(), b"title").unwrap_or("Untitled");

        if url.is_empty() {
            respond(400, r#"{"error":"url is required"}"#, "application/json");
            return;
        }

        let count = match kv_read("bm_count") {
            Some(s) => parse_u64(s),
            None => 0,
        };
        let new_id = count + 1;

        let bm_key = make_bm_key(new_id);

        // Store as "title|url"
        let mut val_buf = [0u8; 4096];
        let mut vp = 0;
        let tb = title.as_bytes();
        let mut i = 0;
        while i < tb.len() && vp < val_buf.len() { val_buf[vp] = tb[i]; vp += 1; i += 1; }
        if vp < val_buf.len() { val_buf[vp] = b'|'; vp += 1; }
        let ub = url.as_bytes();
        i = 0;
        while i < ub.len() && vp < val_buf.len() { val_buf[vp] = ub[i]; vp += 1; i += 1; }
        let entry_val = unsafe { core::str::from_utf8(&val_buf[..vp]).unwrap_or("") };
        kv_write(bm_key, entry_val);

        let mut nb = [0u8; 20];
        let nl = write_u64(&mut nb, new_id);
        let ns = unsafe { core::str::from_utf8(&nb[..nl]).unwrap_or("0") };
        kv_write("bm_count", ns);

        let mut pos = 0;
        pos = append(pos, b"{\"saved\":true,\"id\":");
        pos = append(pos, &nb[..nl]);
        pos = append(pos, b",\"title\":\"");
        pos = append(pos, title.as_bytes());
        pos = append(pos, b"\"}");
        unsafe {
            let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
            respond(201, resp, "application/json");
        }
        return;
    }

    // DELETE /bookmarks/{id}
    if bytes_equal(method.as_bytes(), b"DELETE") && starts_with(path_bytes, b"/bookmarks/") {
        let id_str = unsafe { core::str::from_utf8(&path_bytes[11..]).unwrap_or("0") };
        let id = parse_u64(id_str);
        if id > 0 {
            let bm_key = make_bm_key(id);
            kv_write(bm_key, "");
        }
        respond(200, r#"{"deleted":true}"#, "application/json");
        return;
    }

    // GET /bookmarks — list all bookmarks as HTML
    let count = match kv_read("bm_count") {
        Some(s) => parse_u64(s),
        None => 0,
    };

    let mut pos = 0;
    pos = append(pos, b"<html><head><title>Bookmarks</title>");
    pos = append(pos, b"<style>body{font-family:sans-serif;max-width:700px;margin:40px auto;background:#0d1117;color:#c9d1d9}");
    pos = append(pos, b"h1{color:#58a6ff}");
    pos = append(pos, b".bm{padding:10px;margin:8px 0;background:#161b22;border:1px solid #30363d;border-radius:6px}");
    pos = append(pos, b".bm a{color:#58a6ff;text-decoration:none;font-size:16px}.bm a:hover{text-decoration:underline}");
    pos = append(pos, b".bm .url{color:#8b949e;font-size:12px;margin-top:2px}");
    pos = append(pos, b".empty{color:#8b949e;font-style:italic}");
    pos = append(pos, b"</style></head><body>");
    pos = append(pos, b"<h1>Bookmarks</h1>");

    if count == 0 {
        pos = append(pos, b"<p class=\"empty\">No bookmarks saved yet. POST to /bookmarks with {\"url\":\"...\",\"title\":\"...\"}</p>");
    } else {
        let mut idx: u64 = 1;
        while idx <= count {
            let bm_key = make_bm_key(idx);
            if let Some(entry) = kv_read(bm_key) {
                if !entry.is_empty() {
                    let eb = entry.as_bytes();
                    let mut split = 0;
                    while split < eb.len() && eb[split] != b'|' { split += 1; }
                    let title_part = &eb[..split];
                    let url_part = if split + 1 < eb.len() { &eb[split + 1..] } else { b"#" as &[u8] };

                    pos = append(pos, b"<div class=\"bm\"><a href=\"");
                    pos = append(pos, url_part);
                    pos = append(pos, b"\" target=\"_blank\">");
                    pos = append(pos, title_part);
                    pos = append(pos, b"</a><div class=\"url\">");
                    pos = append(pos, url_part);
                    pos = append(pos, b"</div></div>");
                }
            }
            idx += 1;
        }
    }

    pos = append(pos, b"</body></html>");

    unsafe {
        let html = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("error");
        respond(200, html, "text/html");
    }
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
