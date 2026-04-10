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

fn make_msg_key(id: u64) -> &'static str {
    static mut KEY_BUF: [u8; 32] = [0u8; 32];
    unsafe {
        let prefix = b"msg_";
        let mut kp = 0;
        let mut ki = 0;
        while ki < prefix.len() { KEY_BUF[kp] = prefix[ki]; kp += 1; ki += 1; }
        let mut num_buf = [0u8; 20];
        let num_len = write_u64(&mut num_buf, id);
        ki = 0;
        while ki < num_len { KEY_BUF[kp] = num_buf[ki]; kp += 1; ki += 1; }
        core::str::from_utf8(&KEY_BUF[..kp]).unwrap_or("msg_0")
    }
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };
    let method = find_json_str(request, b"method").unwrap_or("GET");
    let body = find_json_str(request, b"body").unwrap_or("");

    host_log(0, "message_board: handling request");

    if bytes_equal(method.as_bytes(), b"POST") {
        let author = find_json_str(body.as_bytes(), b"author").unwrap_or("Anonymous");
        let message = find_json_str(body.as_bytes(), b"message").unwrap_or("");

        if message.is_empty() {
            respond(400, r#"{"error":"message is required"}"#, "application/json");
            return;
        }

        let count = match kv_read("msg_count") {
            Some(s) => parse_u64(s),
            None => 0,
        };
        let new_id = count + 1;

        // Store as "author|message"
        let msg_key = make_msg_key(new_id);
        let mut val_buf = [0u8; 4096];
        let mut vp = 0;
        let ab = author.as_bytes();
        let mut i = 0;
        while i < ab.len() && vp < val_buf.len() { val_buf[vp] = ab[i]; vp += 1; i += 1; }
        if vp < val_buf.len() { val_buf[vp] = b'|'; vp += 1; }
        let mb = message.as_bytes();
        i = 0;
        while i < mb.len() && vp < val_buf.len() { val_buf[vp] = mb[i]; vp += 1; i += 1; }
        let entry_val = unsafe { core::str::from_utf8(&val_buf[..vp]).unwrap_or("") };
        kv_write(msg_key, entry_val);

        let mut num_buf = [0u8; 20];
        let num_len = write_u64(&mut num_buf, new_id);
        let count_str = unsafe { core::str::from_utf8(&num_buf[..num_len]).unwrap_or("0") };
        kv_write("msg_count", count_str);

        respond(201, r#"{"posted":true}"#, "application/json");
        return;
    }

    // GET: show last 10 messages as HTML
    let count = match kv_read("msg_count") {
        Some(s) => parse_u64(s),
        None => 0,
    };

    let mut pos = 0;
    pos = append(pos, b"<html><head><title>Message Board</title>");
    pos = append(pos, b"<style>body{font-family:sans-serif;max-width:700px;margin:40px auto;background:#161b22;color:#c9d1d9}");
    pos = append(pos, b"h1{color:#58a6ff;border-bottom:1px solid #30363d;padding-bottom:10px}");
    pos = append(pos, b".msg{background:#0d1117;border:1px solid #30363d;border-radius:6px;padding:12px;margin:10px 0}");
    pos = append(pos, b".author{color:#58a6ff;font-weight:bold}.text{margin-top:6px}.num{color:#484f58;font-size:12px}");
    pos = append(pos, b"</style></head><body>");
    pos = append(pos, b"<h1>Message Board</h1>");

    if count == 0 {
        pos = append(pos, b"<p>No messages yet. POST with {\"author\":\"...\",\"message\":\"...\"} to add one.</p>");
    } else {
        // Show last 10 messages (newest first)
        let start = if count > 10 { count - 9 } else { 1 };
        let mut idx = count;
        while idx >= start {
            let msg_key = make_msg_key(idx);
            if let Some(entry) = kv_read(msg_key) {
                let eb = entry.as_bytes();
                let mut split = 0;
                while split < eb.len() && eb[split] != b'|' { split += 1; }
                let author_part = &eb[..split];
                let text_part = if split + 1 < eb.len() { &eb[split + 1..] } else { b"" as &[u8] };

                pos = append(pos, b"<div class=\"msg\"><span class=\"num\">#");
                let mut num_buf = [0u8; 20];
                let num_len = write_u64(&mut num_buf, idx);
                pos = append(pos, &num_buf[..num_len]);
                pos = append(pos, b"</span> <span class=\"author\">");
                pos = append(pos, author_part);
                pos = append(pos, b"</span><div class=\"text\">");
                pos = append(pos, text_part);
                pos = append(pos, b"</div></div>");
            }
            if idx == 0 { break; }
            idx -= 1;
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
