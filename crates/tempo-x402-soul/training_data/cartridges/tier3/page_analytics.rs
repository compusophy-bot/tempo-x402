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

fn starts_with(haystack: &str, needle: &str) -> bool {
    let hb = haystack.as_bytes();
    let nb = needle.as_bytes();
    if hb.len() < nb.len() { return false; }
    let mut i = 0;
    while i < nb.len() {
        if hb[i] != nb[i] { return false; }
        i += 1;
    }
    true
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

const MAX_PAGES: usize = 200;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe {
        let bytes = core::slice::from_raw_parts(request_ptr, request_len as usize);
        core::str::from_utf8(bytes).unwrap_or("{}")
    };

    let method = find_json_str(request, "method").unwrap_or("GET");
    let path = find_json_str(request, "path").unwrap_or("/");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(1, "page_analytics: handling request");

    // POST /track — record a page view: {"page":"/about"}
    if method == "POST" && path == "/track" {
        let page = find_json_str(body, "page").unwrap_or("/");
        if page.is_empty() {
            respond(400, "{\"error\":\"page is required\"}", "application/json");
            return;
        }

        // Increment page view count
        let view_key = make_key("pv_", page);
        let current = parse_u32(kv_read(view_key).unwrap_or("0"));
        let new_count = current + 1;
        let mut tmp = [0u8; 10];
        kv_write(view_key, u32_to_str(new_count, &mut tmp));

        // Increment global total
        let total = parse_u32(kv_read("pv_total").unwrap_or("0")) + 1;
        let mut tt = [0u8; 10];
        kv_write("pv_total", u32_to_str(total, &mut tt));

        // Track page in index if new
        if current == 0 {
            let page_count = parse_u32(kv_read("pv_page_count").unwrap_or("0"));
            if (page_count as usize) < MAX_PAGES {
                let new_page_count = page_count + 1;
                let mut tmp2 = [0u8; 10];
                let idx_key = make_key("pv_page_", u32_to_str(new_page_count, &mut tmp2));
                kv_write(idx_key, page);
                let mut tmp3 = [0u8; 10];
                kv_write("pv_page_count", u32_to_str(new_page_count, &mut tmp3));
            }
        }

        let mut p = 0;
        p = buf_write(p, "{\"tracked\":true,\"page\":\"");
        p = buf_write(p, page);
        p = buf_write(p, "\",\"views\":");
        let mut vb = [0u8; 10];
        p = buf_write(p, u32_to_str(new_count, &mut vb));
        p = buf_write(p, "}");
        respond(200, buf_as_str(p), "application/json");
        return;
    }

    // GET /stats/json — return all stats as JSON
    if path == "/stats/json" {
        let page_count = parse_u32(kv_read("pv_page_count").unwrap_or("0"));
        let total = kv_read("pv_total").unwrap_or("0");

        let mut p = 0;
        p = buf_write(p, "{\"total_views\":");
        p = buf_write(p, total);
        p = buf_write(p, ",\"pages\":[");

        let mut first = true;
        let mut idx: u32 = 1;
        while idx <= page_count {
            let mut tmp = [0u8; 10];
            let idx_key = make_key("pv_page_", u32_to_str(idx, &mut tmp));
            if let Some(page) = kv_read(idx_key) {
                let view_key = make_key("pv_", page);
                let views = kv_read(view_key).unwrap_or("0");
                if !first { p = buf_write(p, ","); }
                p = buf_write(p, "{\"page\":\"");
                p = buf_write(p, page);
                p = buf_write(p, "\",\"views\":");
                p = buf_write(p, views);
                p = buf_write(p, "}");
                first = false;
            }
            idx += 1;
        }

        p = buf_write(p, "]}");
        respond(200, buf_as_str(p), "application/json");
        return;
    }

    // GET /stats — show view counts per page as HTML dashboard
    let page_count = parse_u32(kv_read("pv_page_count").unwrap_or("0"));
    let total = kv_read("pv_total").unwrap_or("0");

    // Find max views for bar chart scaling
    let mut max_views: u32 = 1;
    let mut idx: u32 = 1;
    while idx <= page_count {
        let mut tmp = [0u8; 10];
        let idx_key = make_key("pv_page_", u32_to_str(idx, &mut tmp));
        if let Some(page) = kv_read(idx_key) {
            let view_key = make_key("pv_", page);
            let v = parse_u32(kv_read(view_key).unwrap_or("0"));
            if v > max_views { max_views = v; }
        }
        idx += 1;
    }

    let mut p = 0;
    p = buf_write(p, "<html><head><title>Page Analytics</title>");
    p = buf_write(p, "<style>body{font-family:sans-serif;max-width:800px;margin:40px auto;background:#0d1117;color:#c9d1d9;padding:0 20px}");
    p = buf_write(p, "h1{color:#58a6ff;text-align:center}");
    p = buf_write(p, ".total{text-align:center;font-size:48px;color:#3fb950;margin:20px 0}");
    p = buf_write(p, ".total-label{text-align:center;color:#8b949e;margin-bottom:32px}");
    p = buf_write(p, ".row{display:flex;align-items:center;margin:8px 0;padding:8px 12px;background:#161b22;border-radius:6px}");
    p = buf_write(p, ".page-name{width:200px;color:#58a6ff;font-family:monospace;overflow:hidden;text-overflow:ellipsis}");
    p = buf_write(p, ".bar-container{flex:1;margin:0 16px;height:20px;background:#21262d;border-radius:3px;overflow:hidden}");
    p = buf_write(p, ".bar{height:100%;background:linear-gradient(90deg,#238636,#3fb950);border-radius:3px;transition:width 0.3s}");
    p = buf_write(p, ".view-count{width:80px;text-align:right;font-family:monospace;color:#3fb950}");
    p = buf_write(p, ".empty{text-align:center;color:#8b949e;margin-top:40px;font-style:italic}</style></head>");
    p = buf_write(p, "<body><h1>Page Analytics</h1>");
    p = buf_write(p, "<div class=\"total\">");
    p = buf_write(p, total);
    p = buf_write(p, "</div><div class=\"total-label\">Total Page Views</div>");

    if page_count == 0 {
        p = buf_write(p, "<p class=\"empty\">No data yet. POST /track with {\"page\":\"/path\"} to record views.</p>");
    } else {
        idx = 1;
        while idx <= page_count {
            let mut tmp = [0u8; 10];
            let idx_key = make_key("pv_page_", u32_to_str(idx, &mut tmp));
            if let Some(page) = kv_read(idx_key) {
                let view_key = make_key("pv_", page);
                let views = parse_u32(kv_read(view_key).unwrap_or("0"));
                // Bar width as percentage of max
                let bar_pct = (views * 100) / max_views;

                p = buf_write(p, "<div class=\"row\"><div class=\"page-name\">");
                p = buf_write(p, page);
                p = buf_write(p, "</div><div class=\"bar-container\"><div class=\"bar\" style=\"width:");
                let mut pb = [0u8; 10];
                p = buf_write(p, u32_to_str(bar_pct, &mut pb));
                p = buf_write(p, "%\"></div></div><div class=\"view-count\">");
                let mut vb = [0u8; 10];
                p = buf_write(p, u32_to_str(views, &mut vb));
                p = buf_write(p, "</div></div>");
            }
            idx += 1;
        }
    }

    p = buf_write(p, "</body></html>");
    respond(200, buf_as_str(p), "text/html");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
