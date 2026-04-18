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
    unsafe { response(status, body.as_ptr(), body.len() as i32, content_type.as_ptr(), content_type.len() as i32); }
}

fn host_log(level: i32, msg: &str) {
    unsafe { log(level, msg.as_ptr(), msg.len() as i32); }
}

fn find_json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let kb = key.as_bytes();
    let jb = json.as_bytes();
    let mut i = 0;
    while i + kb.len() + 3 < jb.len() {
        if jb[i] == b'"' {
            let s = i + 1;
            if s + kb.len() < jb.len() && &jb[s..s + kb.len()] == kb && jb[s + kb.len()] == b'"' {
                let mut j = s + kb.len() + 1;
                while j < jb.len() && (jb[j] == b':' || jb[j] == b' ') { j += 1; }
                if j < jb.len() && jb[j] == b'"' {
                    let vs = j + 1;
                    let mut ve = vs;
                    while ve < jb.len() && jb[ve] != b'"' { ve += 1; }
                    return core::str::from_utf8(&jb[vs..ve]).ok();
                }
            }
        }
        i += 1;
    }
    None
}

fn kv_read(key: &str) -> Option<&'static str> {
    unsafe {
        let r = kv_get(key.as_ptr(), key.len() as i32);
        if r < 0 { return None; }
        let ptr = (r >> 32) as *const u8;
        let len = (r & 0xFFFFFFFF) as usize;
        core::str::from_utf8(core::slice::from_raw_parts(ptr, len)).ok()
    }
}

fn kv_write(key: &str, value: &str) {
    unsafe { kv_set(key.as_ptr(), key.len() as i32, value.as_ptr(), value.len() as i32); }
}

static mut BUF: [u8; 32768] = [0u8; 32768];

fn buf_write(pos: usize, s: &str) -> usize {
    let b = s.as_bytes();
    let end = (pos + b.len()).min(unsafe { BUF.len() });
    unsafe { BUF[pos..end].copy_from_slice(&b[..end - pos]); }
    end
}

fn buf_as_str(len: usize) -> &'static str {
    unsafe { core::str::from_utf8_unchecked(&BUF[..len]) }
}

fn parse_u32(s: &str) -> u32 {
    let mut n: u32 = 0;
    for b in s.as_bytes() {
        if *b >= b'0' && *b <= b'9' {
            n = n.wrapping_mul(10).wrapping_add((*b - b'0') as u32);
        }
    }
    n
}

fn u32_to_str(mut n: u32, buf: &mut [u8; 10]) -> &str {
    if n == 0 { buf[0] = b'0'; return unsafe { core::str::from_utf8_unchecked(&buf[..1]) }; }
    let mut i = 10;
    while n > 0 { i -= 1; buf[i] = b'0' + (n % 10) as u8; n /= 10; }
    unsafe { core::str::from_utf8_unchecked(&buf[i..]) }
}

fn make_key(prefix: &str, suffix: &str) -> &'static str {
    static mut KEY_BUF: [u8; 128] = [0u8; 128];
    unsafe {
        let mut kp = 0;
        for b in prefix.as_bytes() { if kp < KEY_BUF.len() { KEY_BUF[kp] = *b; kp += 1; } }
        for b in suffix.as_bytes() { if kp < KEY_BUF.len() { KEY_BUF[kp] = *b; kp += 1; } }
        core::str::from_utf8_unchecked(&KEY_BUF[..kp])
    }
}

/// Generate a simple paste ID from the paste count using base-36 encoding.
fn make_paste_id(num: u32) -> &'static str {
    static mut ID_BUF: [u8; 8] = [0u8; 8];
    unsafe {
        let chars = b"0123456789abcdefghijklmnopqrstuvwxyz";
        let mut n = num;
        let mut i = 7;
        // Always produce at least 4 chars
        loop {
            ID_BUF[i] = chars[(n % 36) as usize];
            n /= 36;
            if i == 0 || (n == 0 && i <= 4) { break; }
            i -= 1;
        }
        core::str::from_utf8_unchecked(&ID_BUF[i..8])
    }
}

const MAX_PASTES: usize = 500;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe {
        let bytes = core::slice::from_raw_parts(request_ptr, request_len as usize);
        core::str::from_utf8(bytes).unwrap_or("{}")
    };

    let method = find_json_str(request, "method").unwrap_or("GET");
    let path = find_json_str(request, "path").unwrap_or("/");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(1, "paste_bin: handling request");

    // POST / — create a new paste
    if method == "POST" {
        let content = find_json_str(body, "content").unwrap_or(body);
        if content.is_empty() {
            respond(400, "{\"error\":\"content is required\"}", "application/json");
            return;
        }

        let count = parse_u32(kv_read("paste_count").unwrap_or("0"));
        if count as usize >= MAX_PASTES {
            respond(400, "{\"error\":\"paste limit reached\"}", "application/json");
            return;
        }

        let new_id = count + 1;
        let paste_id = make_paste_id(new_id);

        let paste_key = make_key("paste_", paste_id);
        kv_write(paste_key, content);

        // Store id in index
        let mut tmp = [0u8; 10];
        let idx_key = make_key("paste_idx_", u32_to_str(new_id, &mut tmp));
        kv_write(idx_key, paste_id);

        let mut tmp2 = [0u8; 10];
        kv_write("paste_count", u32_to_str(new_id, &mut tmp2));

        let mut p = 0;
        p = buf_write(p, "{\"id\":\"");
        p = buf_write(p, paste_id);
        p = buf_write(p, "\",\"url\":\"/");
        p = buf_write(p, paste_id);
        p = buf_write(p, "\"}");
        respond(201, buf_as_str(p), "application/json");
        return;
    }

    // GET /{id} — retrieve a paste
    if path.len() > 1 && path.as_bytes()[0] == b'/' {
        let paste_id = &path[1..];

        // Skip if it looks like a known route
        if paste_id == "recent" {
            // GET /recent — list recent paste IDs
            let count = parse_u32(kv_read("paste_count").unwrap_or("0"));
            let mut p = 0;
            p = buf_write(p, "{\"pastes\":[");
            let start = if count > 20 { count - 20 } else { 0 };
            let mut first = true;
            let mut idx = count;
            while idx > start {
                let mut tmp = [0u8; 10];
                let idx_key = make_key("paste_idx_", u32_to_str(idx, &mut tmp));
                if let Some(pid) = kv_read(idx_key) {
                    if !first { p = buf_write(p, ","); }
                    p = buf_write(p, "\"");
                    p = buf_write(p, pid);
                    p = buf_write(p, "\"");
                    first = false;
                }
                idx -= 1;
            }
            p = buf_write(p, "],\"total\":");
            let mut tb = [0u8; 10];
            p = buf_write(p, u32_to_str(count, &mut tb));
            p = buf_write(p, "}");
            respond(200, buf_as_str(p), "application/json");
            return;
        }

        let paste_key = make_key("paste_", paste_id);
        match kv_read(paste_key) {
            Some(content) if !content.is_empty() => {
                // Render as HTML with syntax-highlighted-ish view
                let mut p = 0;
                p = buf_write(p, "<html><head><title>Paste ");
                p = buf_write(p, paste_id);
                p = buf_write(p, "</title>");
                p = buf_write(p, "<style>body{font-family:monospace;background:#0d1117;color:#c9d1d9;padding:20px;margin:0}");
                p = buf_write(p, ".header{background:#161b22;padding:12px 20px;border-radius:8px 8px 0 0;border:1px solid #30363d;color:#58a6ff;font-size:14px}");
                p = buf_write(p, "pre{background:#0d1117;padding:20px;border:1px solid #30363d;border-top:none;border-radius:0 0 8px 8px;overflow-x:auto;line-height:1.6;white-space:pre-wrap;word-wrap:break-word}");
                p = buf_write(p, ".container{max-width:900px;margin:40px auto}");
                p = buf_write(p, "</style></head><body><div class=\"container\">");
                p = buf_write(p, "<div class=\"header\">Paste: ");
                p = buf_write(p, paste_id);
                p = buf_write(p, "</div><pre>");
                p = buf_write(p, content);
                p = buf_write(p, "</pre></div></body></html>");
                respond(200, buf_as_str(p), "text/html");
            }
            _ => {
                respond(404, "{\"error\":\"paste not found\"}", "application/json");
            }
        }
        return;
    }

    // GET / — landing page
    let count = parse_u32(kv_read("paste_count").unwrap_or("0"));
    let mut p = 0;
    p = buf_write(p, "<html><head><title>Paste Bin</title>");
    p = buf_write(p, "<style>body{font-family:sans-serif;background:#0d1117;color:#c9d1d9;text-align:center;padding:60px 20px}");
    p = buf_write(p, "h1{color:#58a6ff;font-size:36px}.info{color:#8b949e;margin:16px}</style></head>");
    p = buf_write(p, "<body><h1>Paste Bin</h1>");
    p = buf_write(p, "<p class=\"info\">POST {\"content\":\"...\"} to create a paste</p>");
    p = buf_write(p, "<p class=\"info\">Total pastes: ");
    let mut tb = [0u8; 10];
    p = buf_write(p, u32_to_str(count, &mut tb));
    p = buf_write(p, "</p></body></html>");
    respond(200, buf_as_str(p), "text/html");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
