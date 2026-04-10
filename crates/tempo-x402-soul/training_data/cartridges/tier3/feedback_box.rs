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

const MAX_FEEDBACK: usize = 200;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe {
        let bytes = core::slice::from_raw_parts(request_ptr, request_len as usize);
        core::str::from_utf8(bytes).unwrap_or("{}")
    };

    let method = find_json_str(request, "method").unwrap_or("GET");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(1, "feedback_box: handling request");

    // POST — submit feedback: {"name":"...", "comment":"...", "timestamp":"..."}
    if method == "POST" {
        let name = find_json_str(body, "name").unwrap_or("Anonymous");
        let comment = find_json_str(body, "comment").unwrap_or("");
        let timestamp = find_json_str(body, "timestamp").unwrap_or("unknown");

        if comment.is_empty() {
            respond(400, "{\"error\":\"comment is required\"}", "application/json");
            return;
        }

        let count = parse_u32(kv_read("fb_count").unwrap_or("0"));
        if count as usize >= MAX_FEEDBACK {
            respond(400, "{\"error\":\"feedback limit reached\"}", "application/json");
            return;
        }

        let new_id = count + 1;
        let mut tmp = [0u8; 10];
        let num_s = u32_to_str(new_id, &mut tmp);

        // Store as "name|timestamp|comment"
        let mut val_buf = [0u8; 4096];
        let mut vp = 0;
        for b in name.as_bytes() { if vp < val_buf.len() { val_buf[vp] = *b; vp += 1; } }
        if vp < val_buf.len() { val_buf[vp] = b'|'; vp += 1; }
        for b in timestamp.as_bytes() { if vp < val_buf.len() { val_buf[vp] = *b; vp += 1; } }
        if vp < val_buf.len() { val_buf[vp] = b'|'; vp += 1; }
        for b in comment.as_bytes() { if vp < val_buf.len() { val_buf[vp] = *b; vp += 1; } }
        let entry_val = unsafe { core::str::from_utf8_unchecked(&val_buf[..vp]) };

        let entry_key = make_key("fb_entry_", num_s);
        kv_write(entry_key, entry_val);

        let mut tmp2 = [0u8; 10];
        kv_write("fb_count", u32_to_str(new_id, &mut tmp2));

        let mut p = 0;
        p = buf_write(p, "{\"submitted\":true,\"id\":");
        let mut tb = [0u8; 10];
        p = buf_write(p, u32_to_str(new_id, &mut tb));
        p = buf_write(p, "}");
        respond(201, buf_as_str(p), "application/json");
        return;
    }

    // GET — show all feedback as HTML
    let count = parse_u32(kv_read("fb_count").unwrap_or("0"));

    let mut p = 0;
    p = buf_write(p, "<html><head><title>Feedback Box</title>");
    p = buf_write(p, "<style>body{font-family:sans-serif;max-width:700px;margin:40px auto;background:#0f0f23;color:#ccc;padding:0 20px}");
    p = buf_write(p, "h1{color:#ffcc00;text-align:center}");
    p = buf_write(p, ".fb{background:#1a1a3e;padding:16px;margin:12px 0;border-radius:8px;border-left:4px solid #e94560}");
    p = buf_write(p, ".fb-name{color:#00cc96;font-weight:bold;font-size:14px}");
    p = buf_write(p, ".fb-time{color:#666;font-size:12px;margin-left:8px}");
    p = buf_write(p, ".fb-comment{margin-top:8px;line-height:1.5}");
    p = buf_write(p, ".empty{text-align:center;color:#888;margin-top:40px;font-style:italic}");
    p = buf_write(p, ".count{text-align:center;color:#666;margin-top:20px}</style></head>");
    p = buf_write(p, "<body><h1>Feedback Box</h1>");

    if count == 0 {
        p = buf_write(p, "<p class=\"empty\">No feedback yet. POST {\"name\":\"...\",\"comment\":\"...\",\"timestamp\":\"...\"} to submit.</p>");
    } else {
        // Show newest first
        let mut idx = count;
        while idx >= 1 {
            let mut tmp = [0u8; 10];
            let num_s = u32_to_str(idx, &mut tmp);
            let entry_key = make_key("fb_entry_", num_s);

            if let Some(entry) = kv_read(entry_key) {
                let eb = entry.as_bytes();
                // Split on first '|' for name, second '|' for timestamp, rest is comment
                let mut first_pipe = 0;
                while first_pipe < eb.len() && eb[first_pipe] != b'|' { first_pipe += 1; }
                let mut second_pipe = first_pipe + 1;
                while second_pipe < eb.len() && eb[second_pipe] != b'|' { second_pipe += 1; }

                let name_part = &eb[..first_pipe];
                let time_part = if first_pipe + 1 < second_pipe { &eb[first_pipe + 1..second_pipe] } else { b"" as &[u8] };
                let comment_part = if second_pipe + 1 < eb.len() { &eb[second_pipe + 1..] } else { b"" as &[u8] };

                p = buf_write(p, "<div class=\"fb\"><span class=\"fb-name\">");
                p = buf_write(p, unsafe { core::str::from_utf8_unchecked(name_part) });
                p = buf_write(p, "</span><span class=\"fb-time\">");
                p = buf_write(p, unsafe { core::str::from_utf8_unchecked(time_part) });
                p = buf_write(p, "</span><div class=\"fb-comment\">");
                p = buf_write(p, unsafe { core::str::from_utf8_unchecked(comment_part) });
                p = buf_write(p, "</div></div>");
            }
            idx -= 1;
        }

        p = buf_write(p, "<p class=\"count\">Total feedback: ");
        let mut tb = [0u8; 10];
        p = buf_write(p, u32_to_str(count, &mut tb));
        p = buf_write(p, "</p>");
    }

    p = buf_write(p, "</body></html>");
    respond(200, buf_as_str(p), "text/html");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
