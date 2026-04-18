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

/// Build a KV key from prefix + suffix into a static buffer.
fn make_key(prefix: &str, suffix: &str) -> &'static str {
    static mut KEY_BUF: [u8; 128] = [0u8; 128];
    unsafe {
        let mut kp = 0;
        for b in prefix.as_bytes() { if kp < KEY_BUF.len() { KEY_BUF[kp] = *b; kp += 1; } }
        for b in suffix.as_bytes() { if kp < KEY_BUF.len() { KEY_BUF[kp] = *b; kp += 1; } }
        core::str::from_utf8_unchecked(&KEY_BUF[..kp])
    }
}

const MAX_TAGS: usize = 200;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe {
        let bytes = core::slice::from_raw_parts(request_ptr, request_len as usize);
        core::str::from_utf8(bytes).unwrap_or("{}")
    };

    let method = find_json_str(request, "method").unwrap_or("GET");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(1, "tag_cloud: handling request");

    if method == "POST" {
        // Body: {"tag":"rust"} — add/increment a tag
        let tag = find_json_str(body, "tag").unwrap_or("");
        if tag.is_empty() {
            respond(400, "{\"error\":\"tag is required\"}", "application/json");
            return;
        }

        // Check if tag already exists by scanning tag list
        let count = parse_u32(kv_read("tag_count").unwrap_or("0"));
        let mut found = false;

        let mut idx: u32 = 1;
        while idx <= count {
            let mut tmp = [0u8; 10];
            let num_s = u32_to_str(idx, &mut tmp);
            let name_key = make_key("tag_name_", num_s);
            if let Some(existing) = kv_read(name_key) {
                if existing.as_bytes() == tag.as_bytes() {
                    // Increment frequency
                    let mut tmp2 = [0u8; 10];
                    let freq_key = make_key("tag_freq_", num_s);
                    let freq = parse_u32(kv_read(freq_key).unwrap_or("0")) + 1;
                    let freq_s = u32_to_str(freq, &mut tmp2);
                    kv_write(freq_key, freq_s);
                    found = true;

                    let mut p = 0;
                    p = buf_write(p, "{\"tag\":\"");
                    p = buf_write(p, tag);
                    p = buf_write(p, "\",\"count\":");
                    p = buf_write(p, freq_s);
                    p = buf_write(p, "}");
                    respond(200, buf_as_str(p), "application/json");
                    return;
                }
            }
            idx += 1;
        }

        if !found {
            if count >= MAX_TAGS as u32 {
                respond(400, "{\"error\":\"tag limit reached\"}", "application/json");
                return;
            }
            let new_id = count + 1;
            let mut tmp = [0u8; 10];
            let num_s = u32_to_str(new_id, &mut tmp);

            let name_key = make_key("tag_name_", num_s);
            kv_write(name_key, tag);

            let freq_key = make_key("tag_freq_", num_s);
            kv_write(freq_key, "1");

            let mut tmp2 = [0u8; 10];
            let count_s = u32_to_str(new_id, &mut tmp2);
            kv_write("tag_count", count_s);

            let mut p = 0;
            p = buf_write(p, "{\"tag\":\"");
            p = buf_write(p, tag);
            p = buf_write(p, "\",\"count\":1}");
            respond(201, buf_as_str(p), "application/json");
        }
        return;
    }

    // GET — render tag cloud as HTML
    let count = parse_u32(kv_read("tag_count").unwrap_or("0"));

    let mut p = 0;
    p = buf_write(p, "<html><head><title>Tag Cloud</title>");
    p = buf_write(p, "<style>body{font-family:sans-serif;max-width:800px;margin:40px auto;background:#1a1a2e;color:#eee;text-align:center}");
    p = buf_write(p, "h1{color:#e94560}.cloud{padding:30px;line-height:2.5}");
    p = buf_write(p, ".tag{display:inline-block;margin:4px 8px;padding:4px 12px;background:#16213e;border-radius:16px;color:#0ff;text-decoration:none}");
    p = buf_write(p, ".empty{color:#888;font-style:italic}</style></head><body>");
    p = buf_write(p, "<h1>Tag Cloud</h1><div class=\"cloud\">");

    if count == 0 {
        p = buf_write(p, "<p class=\"empty\">No tags yet. POST {\"tag\":\"...\"} to add one.</p>");
    } else {
        // Find max frequency for scaling
        let mut max_freq: u32 = 1;
        let mut idx: u32 = 1;
        while idx <= count {
            let mut tmp = [0u8; 10];
            let num_s = u32_to_str(idx, &mut tmp);
            let freq_key = make_key("tag_freq_", num_s);
            let freq = parse_u32(kv_read(freq_key).unwrap_or("1"));
            if freq > max_freq { max_freq = freq; }
            idx += 1;
        }

        // Render each tag with font-size proportional to frequency (12px..48px)
        idx = 1;
        while idx <= count {
            let mut tmp = [0u8; 10];
            let num_s = u32_to_str(idx, &mut tmp);
            let name_key = make_key("tag_name_", num_s);
            let freq_key = make_key("tag_freq_", num_s);

            if let Some(name) = kv_read(name_key) {
                let freq = parse_u32(kv_read(freq_key).unwrap_or("1"));
                // Scale: size = 12 + (freq * 36) / max_freq
                let size = 12 + (freq * 36) / max_freq;

                p = buf_write(p, "<span class=\"tag\" style=\"font-size:");
                let mut sb = [0u8; 10];
                p = buf_write(p, u32_to_str(size, &mut sb));
                p = buf_write(p, "px\">");
                p = buf_write(p, name);
                p = buf_write(p, " (");
                let mut fb = [0u8; 10];
                p = buf_write(p, u32_to_str(freq, &mut fb));
                p = buf_write(p, ")</span>");
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
