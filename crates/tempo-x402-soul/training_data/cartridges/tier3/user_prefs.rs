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

/// Preferences are stored as individual KV keys: "pref_theme", "pref_language", "pref_timezone", "pref_notifications"
const PREF_KEYS: &[&str] = &["theme", "language", "timezone", "notifications"];
const PREF_DEFAULTS: &[&str] = &["dark", "en", "UTC", "true"];

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };
    let method = find_json_str(request, b"method").unwrap_or("GET");
    let body = find_json_str(request, b"body").unwrap_or("");

    host_log(0, "user_prefs: handling request");

    if bytes_equal(method.as_bytes(), b"POST") {
        // Update preferences from body JSON
        let mut updated = 0u32;

        // Check each known preference key
        let mut ki = 0;
        while ki < PREF_KEYS.len() {
            let pref_name = PREF_KEYS[ki];
            if let Some(value) = find_json_str(body.as_bytes(), pref_name.as_bytes()) {
                // Build pref key: "pref_{name}"
                let mut pk_buf = [0u8; 64];
                let mut pkp = 0;
                let prefix = b"pref_";
                let mut pi = 0;
                while pi < prefix.len() { pk_buf[pkp] = prefix[pi]; pkp += 1; pi += 1; }
                let nb = pref_name.as_bytes();
                pi = 0;
                while pi < nb.len() && pkp < pk_buf.len() { pk_buf[pkp] = nb[pi]; pkp += 1; pi += 1; }
                let pref_key = unsafe { core::str::from_utf8(&pk_buf[..pkp]).unwrap_or("") };

                kv_write(pref_key, value);
                updated += 1;
            }
            ki += 1;
        }

        if updated == 0 {
            respond(400, r#"{"error":"no valid preference keys found","valid_keys":["theme","language","timezone","notifications"]}"#, "application/json");
            return;
        }

        let mut pos = 0;
        pos = append(pos, b"{\"updated\":");
        let mut num_buf = [0u8; 10];
        let num_len = {
            if updated == 0 { num_buf[0] = b'0'; 1 }
            else {
                let mut tmp = [0u8; 10];
                let mut n = updated;
                let mut len = 0;
                while n > 0 { tmp[len] = b'0' + (n % 10) as u8; n /= 10; len += 1; }
                let mut i = 0;
                while i < len { num_buf[i] = tmp[len - 1 - i]; i += 1; }
                len
            }
        };
        pos = append(pos, &num_buf[..num_len]);
        pos = append(pos, b"}");
        unsafe {
            let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
            respond(200, resp, "application/json");
        }
        return;
    }

    // GET: return all preferences as JSON
    let mut pos = 0;
    pos = append(pos, b"{\"preferences\":{");

    let mut ki = 0;
    while ki < PREF_KEYS.len() {
        if ki > 0 { pos = append(pos, b","); }

        let pref_name = PREF_KEYS[ki];
        let default_val = PREF_DEFAULTS[ki];

        // Build pref key: "pref_{name}"
        let mut pk_buf = [0u8; 64];
        let mut pkp = 0;
        let prefix = b"pref_";
        let mut pi = 0;
        while pi < prefix.len() { pk_buf[pkp] = prefix[pi]; pkp += 1; pi += 1; }
        let nb = pref_name.as_bytes();
        pi = 0;
        while pi < nb.len() && pkp < pk_buf.len() { pk_buf[pkp] = nb[pi]; pkp += 1; pi += 1; }
        let pref_key = unsafe { core::str::from_utf8(&pk_buf[..pkp]).unwrap_or("") };

        let value = kv_read(pref_key).unwrap_or(default_val);

        pos = append(pos, b"\"");
        pos = append(pos, pref_name.as_bytes());
        pos = append(pos, b"\":\"");
        pos = append(pos, value.as_bytes());
        pos = append(pos, b"\"");

        ki += 1;
    }

    pos = append(pos, b"}}");
    unsafe {
        let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
        respond(200, resp, "application/json");
    }
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
