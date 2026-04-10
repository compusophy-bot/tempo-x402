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

const MAX_LINKS: usize = 50;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe {
        let bytes = core::slice::from_raw_parts(request_ptr, request_len as usize);
        core::str::from_utf8(bytes).unwrap_or("{}")
    };

    let method = find_json_str(request, "method").unwrap_or("GET");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(1, "link_tree: handling request");

    // POST — add a link: {"title":"...", "url":"..."}
    if method == "POST" {
        let title = find_json_str(body, "title").unwrap_or("");
        let url = find_json_str(body, "url").unwrap_or("");

        if title.is_empty() || url.is_empty() {
            respond(400, "{\"error\":\"title and url are required\"}", "application/json");
            return;
        }

        let count = parse_u32(kv_read("link_count").unwrap_or("0"));
        if count as usize >= MAX_LINKS {
            respond(400, "{\"error\":\"link limit reached\"}", "application/json");
            return;
        }

        let new_id = count + 1;
        let mut tmp = [0u8; 10];
        let num_s = u32_to_str(new_id, &mut tmp);

        // Store title and url separately
        let title_key = make_key("link_title_", num_s);
        kv_write(title_key, title);

        let url_key = make_key("link_url_", num_s);
        kv_write(url_key, url);

        let mut tmp2 = [0u8; 10];
        kv_write("link_count", u32_to_str(new_id, &mut tmp2));

        let mut p = 0;
        p = buf_write(p, "{\"added\":true,\"id\":");
        let mut tb = [0u8; 10];
        p = buf_write(p, u32_to_str(new_id, &mut tb));
        p = buf_write(p, ",\"title\":\"");
        p = buf_write(p, title);
        p = buf_write(p, "\"}");
        respond(201, buf_as_str(p), "application/json");
        return;
    }

    // DELETE — remove a link: {"id":"3"}
    if method == "DELETE" {
        let id_str = find_json_str(body, "id").unwrap_or("");
        if id_str.is_empty() {
            respond(400, "{\"error\":\"id is required\"}", "application/json");
            return;
        }
        let title_key = make_key("link_title_", id_str);
        let url_key = make_key("link_url_", id_str);
        kv_write(title_key, "");
        kv_write(url_key, "");
        respond(200, "{\"deleted\":true}", "application/json");
        return;
    }

    // GET — render link tree page
    let count = parse_u32(kv_read("link_count").unwrap_or("0"));
    let profile_name = kv_read("profile_name").unwrap_or("My Links");

    let mut p = 0;
    p = buf_write(p, "<html><head><title>");
    p = buf_write(p, profile_name);
    p = buf_write(p, "</title>");
    p = buf_write(p, "<style>*{margin:0;padding:0;box-sizing:border-box}");
    p = buf_write(p, "body{font-family:sans-serif;background:linear-gradient(135deg,#667eea 0%,#764ba2 100%);min-height:100vh;display:flex;justify-content:center;padding:40px 20px}");
    p = buf_write(p, ".container{max-width:480px;width:100%;text-align:center}");
    p = buf_write(p, "h1{color:#fff;font-size:28px;margin-bottom:8px}");
    p = buf_write(p, ".subtitle{color:rgba(255,255,255,0.7);margin-bottom:32px}");
    p = buf_write(p, ".link{display:block;background:rgba(255,255,255,0.15);backdrop-filter:blur(10px);border:1px solid rgba(255,255,255,0.2);border-radius:12px;padding:16px 24px;margin:12px 0;color:#fff;text-decoration:none;font-size:16px;font-weight:500;transition:transform 0.2s,background 0.2s}");
    p = buf_write(p, ".link:hover{transform:translateY(-2px);background:rgba(255,255,255,0.25)}");
    p = buf_write(p, ".empty{color:rgba(255,255,255,0.6);margin-top:40px;font-style:italic}");
    p = buf_write(p, "</style></head><body><div class=\"container\">");
    p = buf_write(p, "<h1>");
    p = buf_write(p, profile_name);
    p = buf_write(p, "</h1>");
    p = buf_write(p, "<p class=\"subtitle\">Links</p>");

    if count == 0 {
        p = buf_write(p, "<p class=\"empty\">No links yet. POST {\"title\":\"...\",\"url\":\"...\"} to add one.</p>");
    } else {
        let mut idx: u32 = 1;
        while idx <= count {
            let mut tmp = [0u8; 10];
            let num_s = u32_to_str(idx, &mut tmp);
            let title_key = make_key("link_title_", num_s);
            let url_key = make_key("link_url_", num_s);

            if let Some(title) = kv_read(title_key) {
                if !title.is_empty() {
                    let url = kv_read(url_key).unwrap_or("#");
                    p = buf_write(p, "<a class=\"link\" href=\"");
                    p = buf_write(p, url);
                    p = buf_write(p, "\" target=\"_blank\">");
                    p = buf_write(p, title);
                    p = buf_write(p, "</a>");
                }
            }
            idx += 1;
        }
    }

    p = buf_write(p, "</div></body></html>");
    respond(200, buf_as_str(p), "text/html");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
