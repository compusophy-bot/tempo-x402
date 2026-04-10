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

/// Compute the pixel width of the count text (approximate: 7px per digit).
fn text_width(s: &str) -> u32 {
    (s.len() as u32) * 7 + 10
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe {
        let bytes = core::slice::from_raw_parts(request_ptr, request_len as usize);
        core::str::from_utf8(bytes).unwrap_or("{}")
    };

    let method = find_json_str(request, "method").unwrap_or("GET");
    let path = find_json_str(request, "path").unwrap_or("/");

    host_log(1, "badge_counter: handling request");

    // POST /increment/{name} — increment a named counter
    if method == "POST" && starts_with(path, "/increment/") && path.len() > 11 {
        let name = &path[11..];
        let counter_key = make_key("badge_", name);
        let current = parse_u32(kv_read(counter_key).unwrap_or("0"));
        let new_val = current + 1;
        let mut tmp = [0u8; 10];
        let val_s = u32_to_str(new_val, &mut tmp);
        kv_write(counter_key, val_s);

        let mut p = 0;
        p = buf_write(p, "{\"name\":\"");
        p = buf_write(p, name);
        p = buf_write(p, "\",\"count\":");
        p = buf_write(p, val_s);
        p = buf_write(p, "}");
        respond(200, buf_as_str(p), "application/json");
        return;
    }

    // POST /set/{name} — set counter to specific value: {"value":"42"}
    if method == "POST" && starts_with(path, "/set/") && path.len() > 5 {
        let name = &path[5..];
        let body = find_json_str(request, "body").unwrap_or("");
        let value = find_json_str(body, "value").unwrap_or("0");
        let counter_key = make_key("badge_", name);
        kv_write(counter_key, value);

        let mut p = 0;
        p = buf_write(p, "{\"name\":\"");
        p = buf_write(p, name);
        p = buf_write(p, "\",\"count\":");
        p = buf_write(p, value);
        p = buf_write(p, "}");
        respond(200, buf_as_str(p), "application/json");
        return;
    }

    // GET /badge/{name} — return SVG badge
    if starts_with(path, "/badge/") && path.len() > 7 {
        let name = &path[7..];
        let counter_key = make_key("badge_", name);
        let count_str = kv_read(counter_key).unwrap_or("0");

        // Also increment view count (self-counting badge)
        let view_key = make_key("badge_views_", name);
        let views = parse_u32(kv_read(view_key).unwrap_or("0")) + 1;
        let mut vtmp = [0u8; 10];
        kv_write(view_key, u32_to_str(views, &mut vtmp));

        let label_w: u32 = text_width(name) + 10;
        let count_w: u32 = text_width(count_str) + 10;
        let total_w = label_w + count_w;

        let mut p = 0;
        p = buf_write(p, "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"");
        let mut tw = [0u8; 10];
        p = buf_write(p, u32_to_str(total_w, &mut tw));
        p = buf_write(p, "\" height=\"20\">");
        p = buf_write(p, "<linearGradient id=\"b\" x2=\"0\" y2=\"100%\"><stop offset=\"0\" stop-color=\"#bbb\" stop-opacity=\".1\"/><stop offset=\"1\" stop-opacity=\".1\"/></linearGradient>");

        // Label background (dark)
        p = buf_write(p, "<rect rx=\"3\" width=\"");
        p = buf_write(p, u32_to_str(total_w, &mut tw));
        p = buf_write(p, "\" height=\"20\" fill=\"#555\"/>");

        // Count background (colored)
        p = buf_write(p, "<rect rx=\"3\" x=\"");
        let mut lw = [0u8; 10];
        p = buf_write(p, u32_to_str(label_w, &mut lw));
        p = buf_write(p, "\" width=\"");
        let mut cw = [0u8; 10];
        p = buf_write(p, u32_to_str(count_w, &mut cw));
        p = buf_write(p, "\" height=\"20\" fill=\"#4c1\"/>");

        // Cover left corners of count rect
        p = buf_write(p, "<rect x=\"");
        p = buf_write(p, u32_to_str(label_w, &mut lw));
        p = buf_write(p, "\" width=\"4\" height=\"20\" fill=\"#4c1\"/>");

        // Gradient overlay
        p = buf_write(p, "<rect rx=\"3\" width=\"");
        p = buf_write(p, u32_to_str(total_w, &mut tw));
        p = buf_write(p, "\" height=\"20\" fill=\"url(#b)\"/>");

        // Label text (shadow + white)
        let label_x = label_w / 2;
        p = buf_write(p, "<g fill=\"#fff\" text-anchor=\"middle\" font-family=\"sans-serif\" font-size=\"11\">");
        p = buf_write(p, "<text x=\"");
        let mut lx = [0u8; 10];
        p = buf_write(p, u32_to_str(label_x, &mut lx));
        p = buf_write(p, "\" y=\"15\" fill=\"#010101\" fill-opacity=\".3\">");
        p = buf_write(p, name);
        p = buf_write(p, "</text><text x=\"");
        p = buf_write(p, u32_to_str(label_x, &mut lx));
        p = buf_write(p, "\" y=\"14\">");
        p = buf_write(p, name);
        p = buf_write(p, "</text>");

        // Count text
        let count_x = label_w + count_w / 2;
        p = buf_write(p, "<text x=\"");
        let mut cx = [0u8; 10];
        p = buf_write(p, u32_to_str(count_x, &mut cx));
        p = buf_write(p, "\" y=\"15\" fill=\"#010101\" fill-opacity=\".3\">");
        p = buf_write(p, count_str);
        p = buf_write(p, "</text><text x=\"");
        p = buf_write(p, u32_to_str(count_x, &mut cx));
        p = buf_write(p, "\" y=\"14\">");
        p = buf_write(p, count_str);
        p = buf_write(p, "</text></g></svg>");

        respond(200, buf_as_str(p), "image/svg+xml");
        return;
    }

    // GET / — info page
    let mut p = 0;
    p = buf_write(p, "{\"service\":\"badge-counter\",\"usage\":{");
    p = buf_write(p, "\"get_badge\":\"GET /badge/{name}\",");
    p = buf_write(p, "\"increment\":\"POST /increment/{name}\",");
    p = buf_write(p, "\"set\":\"POST /set/{name} with {\\\"value\\\":\\\"N\\\"}\"");
    p = buf_write(p, "}}");
    respond(200, buf_as_str(p), "application/json");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
