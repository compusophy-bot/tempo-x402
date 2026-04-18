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

fn make_note_key(id: u64) -> (&'static str, usize) {
    static mut KEY_BUF: [u8; 32] = [0u8; 32];
    unsafe {
        let prefix = b"note_";
        let mut kp = 0;
        let mut ki = 0;
        while ki < prefix.len() {
            KEY_BUF[kp] = prefix[ki];
            kp += 1;
            ki += 1;
        }
        let mut num_buf = [0u8; 20];
        let num_len = write_u64(&mut num_buf, id);
        ki = 0;
        while ki < num_len {
            KEY_BUF[kp] = num_buf[ki];
            kp += 1;
            ki += 1;
        }
        (core::str::from_utf8(&KEY_BUF[..kp]).unwrap_or("note_0"), kp)
    }
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };
    let method = find_json_str(request, b"method").unwrap_or("GET");
    let path = find_json_str(request, b"path").unwrap_or("/");
    let body = find_json_str(request, b"body").unwrap_or("");
    let path_bytes = path.as_bytes();

    host_log(0, "note_store: handling request");

    // POST /notes — create a note
    if bytes_equal(method.as_bytes(), b"POST") && bytes_equal(path_bytes, b"/notes") {
        let title = find_json_str(body.as_bytes(), b"title").unwrap_or("Untitled");
        let content = find_json_str(body.as_bytes(), b"content").unwrap_or("");

        let count = match kv_read("note_count") {
            Some(s) => parse_u64(s),
            None => 0,
        };
        let new_id = count + 1;

        let (note_key, _) = make_note_key(new_id);

        // Store as "title|content"
        let mut val_buf = [0u8; 8192];
        let mut vp = 0;
        let tb = title.as_bytes();
        let mut i = 0;
        while i < tb.len() && vp < val_buf.len() { val_buf[vp] = tb[i]; vp += 1; i += 1; }
        if vp < val_buf.len() { val_buf[vp] = b'|'; vp += 1; }
        let cb = content.as_bytes();
        i = 0;
        while i < cb.len() && vp < val_buf.len() { val_buf[vp] = cb[i]; vp += 1; i += 1; }
        let entry_val = unsafe { core::str::from_utf8(&val_buf[..vp]).unwrap_or("") };

        kv_write(note_key, entry_val);

        let mut num_buf = [0u8; 20];
        let num_len = write_u64(&mut num_buf, new_id);
        let count_str = unsafe { core::str::from_utf8(&num_buf[..num_len]).unwrap_or("0") };
        kv_write("note_count", count_str);

        let mut pos = 0;
        pos = append(pos, b"{\"created\":true,\"id\":");
        pos = append(pos, &num_buf[..num_len]);
        pos = append(pos, b",\"title\":\"");
        pos = append(pos, title.as_bytes());
        pos = append(pos, b"\"}");
        unsafe {
            let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
            respond(201, resp, "application/json");
        }
        return;
    }

    // GET /notes — list all notes
    if bytes_equal(method.as_bytes(), b"GET") && bytes_equal(path_bytes, b"/notes") {
        let count = match kv_read("note_count") {
            Some(s) => parse_u64(s),
            None => 0,
        };

        let mut pos = 0;
        pos = append(pos, b"{\"notes\":[");

        let mut idx: u64 = 1;
        let mut first = true;
        while idx <= count {
            let (note_key, _) = make_note_key(idx);
            if let Some(entry) = kv_read(note_key) {
                let eb = entry.as_bytes();
                let mut split = 0;
                while split < eb.len() && eb[split] != b'|' { split += 1; }
                let title_part = &eb[..split];

                if !first { pos = append(pos, b","); }
                first = false;

                pos = append(pos, b"{\"id\":");
                let mut num_buf = [0u8; 20];
                let num_len = write_u64(&mut num_buf, idx);
                pos = append(pos, &num_buf[..num_len]);
                pos = append(pos, b",\"title\":\"");
                pos = append(pos, title_part);
                pos = append(pos, b"\"}");
            }
            idx += 1;
        }

        pos = append(pos, b"]}");
        unsafe {
            let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
            respond(200, resp, "application/json");
        }
        return;
    }

    // GET /notes/{id} — read a single note
    if bytes_equal(method.as_bytes(), b"GET") && starts_with(path_bytes, b"/notes/") {
        let id_str = unsafe { core::str::from_utf8(&path_bytes[7..]).unwrap_or("0") };
        let id = parse_u64(id_str);

        if id == 0 {
            respond(400, r#"{"error":"invalid note id"}"#, "application/json");
            return;
        }

        let (note_key, _) = make_note_key(id);
        match kv_read(note_key) {
            Some(entry) => {
                let eb = entry.as_bytes();
                let mut split = 0;
                while split < eb.len() && eb[split] != b'|' { split += 1; }
                let title_part = &eb[..split];
                let content_part = if split + 1 < eb.len() { &eb[split + 1..] } else { b"" as &[u8] };

                let mut pos = 0;
                pos = append(pos, b"{\"id\":");
                let mut num_buf = [0u8; 20];
                let num_len = write_u64(&mut num_buf, id);
                pos = append(pos, &num_buf[..num_len]);
                pos = append(pos, b",\"title\":\"");
                pos = append(pos, title_part);
                pos = append(pos, b"\",\"content\":\"");
                pos = append(pos, content_part);
                pos = append(pos, b"\"}");
                unsafe {
                    let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
                    respond(200, resp, "application/json");
                }
            }
            None => {
                respond(404, r#"{"error":"note not found"}"#, "application/json");
            }
        }
        return;
    }

    // DELETE /notes/{id} — delete a note
    if bytes_equal(method.as_bytes(), b"DELETE") && starts_with(path_bytes, b"/notes/") {
        let id_str = unsafe { core::str::from_utf8(&path_bytes[7..]).unwrap_or("0") };
        let id = parse_u64(id_str);
        let (note_key, _) = make_note_key(id);
        kv_write(note_key, "");
        respond(200, r#"{"deleted":true}"#, "application/json");
        return;
    }

    respond(200, r#"{"service":"note-store","endpoints":["POST /notes","GET /notes","GET /notes/{id}","DELETE /notes/{id}"]}"#, "application/json");
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
