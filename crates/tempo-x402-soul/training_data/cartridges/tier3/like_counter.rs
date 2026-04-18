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

const MAX_ITEMS: usize = 200;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe {
        let bytes = core::slice::from_raw_parts(request_ptr, request_len as usize);
        core::str::from_utf8(bytes).unwrap_or("{}")
    };

    let method = find_json_str(request, "method").unwrap_or("GET");
    let path = find_json_str(request, "path").unwrap_or("/");

    host_log(1, "like_counter: handling request");

    // POST /like/{item} — increment like count
    if method == "POST" && starts_with(path, "/like/") && path.len() > 6 {
        let item = &path[6..];

        // Get or create item like count
        let like_key = make_key("likes_", item);
        let current = parse_u32(kv_read(like_key).unwrap_or("0"));
        let new_count = current + 1;

        let mut tmp = [0u8; 10];
        let count_s = u32_to_str(new_count, &mut tmp);
        kv_write(like_key, count_s);

        // Track item in index if new
        if current == 0 {
            let item_count = parse_u32(kv_read("like_item_count").unwrap_or("0"));
            if (item_count as usize) < MAX_ITEMS {
                let new_item_count = item_count + 1;
                let mut tmp2 = [0u8; 10];
                let num_s = u32_to_str(new_item_count, &mut tmp2);
                let idx_key = make_key("like_item_", num_s);
                kv_write(idx_key, item);

                let mut tmp3 = [0u8; 10];
                kv_write("like_item_count", u32_to_str(new_item_count, &mut tmp3));
            }
        }

        let mut p = 0;
        p = buf_write(p, "{\"item\":\"");
        p = buf_write(p, item);
        p = buf_write(p, "\",\"likes\":");
        p = buf_write(p, count_s);
        p = buf_write(p, "}");
        respond(200, buf_as_str(p), "application/json");
        return;
    }

    // GET /likes — return all items and their counts as JSON
    let item_count = parse_u32(kv_read("like_item_count").unwrap_or("0"));
    let mut p = 0;
    p = buf_write(p, "{\"items\":[");

    let mut first = true;
    let mut idx: u32 = 1;
    while idx <= item_count {
        let mut tmp = [0u8; 10];
        let num_s = u32_to_str(idx, &mut tmp);
        let idx_key = make_key("like_item_", num_s);
        if let Some(item_name) = kv_read(idx_key) {
            let like_key = make_key("likes_", item_name);
            let count = kv_read(like_key).unwrap_or("0");

            if !first { p = buf_write(p, ","); }
            p = buf_write(p, "{\"item\":\"");
            p = buf_write(p, item_name);
            p = buf_write(p, "\",\"likes\":");
            p = buf_write(p, count);
            p = buf_write(p, "}");
            first = false;
        }
        idx += 1;
    }

    p = buf_write(p, "]}");
    respond(200, buf_as_str(p), "application/json");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
