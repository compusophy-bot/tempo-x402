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

/// Validate config key: only alphanumeric, underscore, dot, dash. Max 64 chars.
fn is_valid_key(key: &str) -> bool {
    if key.is_empty() || key.len() > 64 { return false; }
    for b in key.as_bytes() {
        let valid = (*b >= b'a' && *b <= b'z')
            || (*b >= b'A' && *b <= b'Z')
            || (*b >= b'0' && *b <= b'9')
            || *b == b'_' || *b == b'.' || *b == b'-';
        if !valid { return false; }
    }
    true
}

/// Validate config value: max 1024 chars, no control chars.
fn is_valid_value(val: &str) -> bool {
    if val.len() > 1024 { return false; }
    for b in val.as_bytes() {
        if *b < 0x20 && *b != b'\t' { return false; }
    }
    true
}

/// Default values for well-known config keys.
fn get_default(key: &str) -> Option<&'static str> {
    if key == "app.name" { return Some("MyApp"); }
    if key == "app.version" { return Some("1.0.0"); }
    if key == "app.debug" { return Some("false"); }
    if key == "app.max_items" { return Some("100"); }
    if key == "app.theme" { return Some("dark"); }
    None
}

const MAX_KEYS: usize = 200;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe {
        let bytes = core::slice::from_raw_parts(request_ptr, request_len as usize);
        core::str::from_utf8(bytes).unwrap_or("{}")
    };

    let method = find_json_str(request, "method").unwrap_or("GET");
    let path = find_json_str(request, "path").unwrap_or("/");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(1, "config_store: handling request");

    // Route: /config/{key}
    if starts_with(path, "/config/") && path.len() > 8 {
        let key = &path[8..];

        if !is_valid_key(key) {
            respond(400, "{\"error\":\"invalid key format\"}", "application/json");
            return;
        }

        if method == "GET" {
            let cfg_key = make_key("cfg_", key);
            let value = kv_read(cfg_key).or_else(|| get_default(key));
            match value {
                Some(val) => {
                    let is_default = kv_read(cfg_key).is_none();
                    let mut p = 0;
                    p = buf_write(p, "{\"key\":\"");
                    p = buf_write(p, key);
                    p = buf_write(p, "\",\"value\":\"");
                    p = buf_write(p, val);
                    p = buf_write(p, "\",\"source\":\"");
                    if is_default { p = buf_write(p, "default"); } else { p = buf_write(p, "stored"); }
                    p = buf_write(p, "\"}");
                    respond(200, buf_as_str(p), "application/json");
                }
                None => {
                    respond(404, "{\"error\":\"config key not found\"}", "application/json");
                }
            }
            return;
        }

        if method == "PUT" || method == "POST" {
            let value = find_json_str(body, "value").unwrap_or(body);
            if value.is_empty() {
                respond(400, "{\"error\":\"value is required\"}", "application/json");
                return;
            }
            if !is_valid_value(value) {
                respond(400, "{\"error\":\"invalid value\"}", "application/json");
                return;
            }

            let cfg_key = make_key("cfg_", key);

            // Track key in index if new
            if kv_read(cfg_key).is_none() {
                let count = parse_u32(kv_read("cfg_count").unwrap_or("0"));
                if count as usize >= MAX_KEYS {
                    respond(400, "{\"error\":\"config key limit reached\"}", "application/json");
                    return;
                }
                let new_count = count + 1;
                let mut tmp = [0u8; 10];
                let idx_key = make_key("cfg_idx_", u32_to_str(new_count, &mut tmp));
                kv_write(idx_key, key);
                let mut tmp2 = [0u8; 10];
                kv_write("cfg_count", u32_to_str(new_count, &mut tmp2));
            }

            kv_write(cfg_key, value);

            let mut p = 0;
            p = buf_write(p, "{\"key\":\"");
            p = buf_write(p, key);
            p = buf_write(p, "\",\"value\":\"");
            p = buf_write(p, value);
            p = buf_write(p, "\",\"stored\":true}");
            respond(200, buf_as_str(p), "application/json");
            return;
        }

        if method == "DELETE" {
            let cfg_key = make_key("cfg_", key);
            kv_write(cfg_key, "");
            let mut p = 0;
            p = buf_write(p, "{\"key\":\"");
            p = buf_write(p, key);
            p = buf_write(p, "\",\"deleted\":true}");
            respond(200, buf_as_str(p), "application/json");
            return;
        }

        respond(405, "{\"error\":\"method not allowed\"}", "application/json");
        return;
    }

    // GET / — list all config keys
    let count = parse_u32(kv_read("cfg_count").unwrap_or("0"));
    let mut p = 0;
    p = buf_write(p, "{\"configs\":[");
    let mut first = true;
    let mut idx: u32 = 1;
    while idx <= count {
        let mut tmp = [0u8; 10];
        let idx_key = make_key("cfg_idx_", u32_to_str(idx, &mut tmp));
        if let Some(key) = kv_read(idx_key) {
            let cfg_key = make_key("cfg_", key);
            if let Some(val) = kv_read(cfg_key) {
                if !val.is_empty() {
                    if !first { p = buf_write(p, ","); }
                    p = buf_write(p, "{\"key\":\"");
                    p = buf_write(p, key);
                    p = buf_write(p, "\",\"value\":\"");
                    p = buf_write(p, val);
                    p = buf_write(p, "\"}");
                    first = false;
                }
            }
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
