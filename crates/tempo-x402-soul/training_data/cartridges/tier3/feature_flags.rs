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

/// Flags are stored as a comma-separated list of names in "flag_names",
/// each flag's value stored as "flag_{name}" = "1" or "0".
#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };
    let method = find_json_str(request, b"method").unwrap_or("GET");
    let path = find_json_str(request, b"path").unwrap_or("/");
    let path_bytes = path.as_bytes();

    host_log(0, "feature_flags: handling request");

    // POST /flags/{name}/enable or /flags/{name}/disable
    if bytes_equal(method.as_bytes(), b"POST") && starts_with(path_bytes, b"/flags/") {
        let rest = &path_bytes[7..]; // after "/flags/"

        // Find the flag name and action
        let mut slash_pos = 0;
        while slash_pos < rest.len() && rest[slash_pos] != b'/' { slash_pos += 1; }

        if slash_pos == 0 || slash_pos >= rest.len() {
            respond(400, r#"{"error":"use /flags/{name}/enable or /flags/{name}/disable"}"#, "application/json");
            return;
        }

        let flag_name = unsafe { core::str::from_utf8(&rest[..slash_pos]).unwrap_or("") };
        let action = unsafe { core::str::from_utf8(&rest[slash_pos + 1..]).unwrap_or("") };

        let enabled = if bytes_equal(action.as_bytes(), b"enable") {
            true
        } else if bytes_equal(action.as_bytes(), b"disable") {
            false
        } else {
            respond(400, r#"{"error":"action must be enable or disable"}"#, "application/json");
            return;
        };

        // Build flag key: "flag_{name}"
        let mut fk_buf = [0u8; 128];
        let mut fkp = 0;
        let fk_prefix = b"flag_";
        let mut fi = 0;
        while fi < fk_prefix.len() { fk_buf[fkp] = fk_prefix[fi]; fkp += 1; fi += 1; }
        fi = 0;
        let fnb = flag_name.as_bytes();
        while fi < fnb.len() && fkp < fk_buf.len() { fk_buf[fkp] = fnb[fi]; fkp += 1; fi += 1; }
        let flag_key = unsafe { core::str::from_utf8(&fk_buf[..fkp]).unwrap_or("") };

        if enabled {
            kv_write(flag_key, "1");
        } else {
            kv_write(flag_key, "0");
        }

        // Add to flag_names list if not already present
        let names = kv_read("flag_names").unwrap_or("");
        let mut found = false;
        if !names.is_empty() {
            let nb = names.as_bytes();
            let mut start = 0;
            let mut idx = 0;
            while idx <= nb.len() {
                if idx == nb.len() || nb[idx] == b',' {
                    let segment = &nb[start..idx];
                    if bytes_equal(segment, flag_name.as_bytes()) {
                        found = true;
                    }
                    start = idx + 1;
                }
                idx += 1;
            }
        }

        if !found {
            let mut nl_buf = [0u8; 4096];
            let mut nlp = 0;
            if !names.is_empty() {
                let nb = names.as_bytes();
                let mut i = 0;
                while i < nb.len() && nlp < nl_buf.len() { nl_buf[nlp] = nb[i]; nlp += 1; i += 1; }
                if nlp < nl_buf.len() { nl_buf[nlp] = b','; nlp += 1; }
            }
            let fnb = flag_name.as_bytes();
            let mut i = 0;
            while i < fnb.len() && nlp < nl_buf.len() { nl_buf[nlp] = fnb[i]; nlp += 1; i += 1; }
            let new_names = unsafe { core::str::from_utf8(&nl_buf[..nlp]).unwrap_or("") };
            kv_write("flag_names", new_names);
        }

        let mut pos = 0;
        pos = append(pos, b"{\"flag\":\"");
        pos = append(pos, flag_name.as_bytes());
        pos = append(pos, b"\",\"enabled\":");
        if enabled { pos = append(pos, b"true"); } else { pos = append(pos, b"false"); }
        pos = append(pos, b"}");
        unsafe {
            let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
            respond(200, resp, "application/json");
        }
        return;
    }

    // GET /flags — list all flags
    if bytes_equal(method.as_bytes(), b"GET") {
        let names = kv_read("flag_names").unwrap_or("");

        let mut pos = 0;
        pos = append(pos, b"{\"flags\":{");

        if !names.is_empty() {
            let nb = names.as_bytes();
            let mut start = 0;
            let mut idx = 0;
            let mut first = true;
            while idx <= nb.len() {
                if idx == nb.len() || nb[idx] == b',' {
                    let segment = &nb[start..idx];
                    if segment.len() > 0 {
                        if !first { pos = append(pos, b","); }
                        first = false;

                        // Build flag key
                        let mut fk_buf = [0u8; 128];
                        let mut fkp = 0;
                        let fk_prefix = b"flag_";
                        let mut fi = 0;
                        while fi < fk_prefix.len() { fk_buf[fkp] = fk_prefix[fi]; fkp += 1; fi += 1; }
                        fi = 0;
                        while fi < segment.len() && fkp < fk_buf.len() { fk_buf[fkp] = segment[fi]; fkp += 1; fi += 1; }
                        let flag_key = unsafe { core::str::from_utf8(&fk_buf[..fkp]).unwrap_or("") };

                        let val = kv_read(flag_key).unwrap_or("0");
                        let enabled = bytes_equal(val.as_bytes(), b"1");

                        pos = append(pos, b"\"");
                        pos = append(pos, segment);
                        pos = append(pos, b"\":");
                        if enabled { pos = append(pos, b"true"); } else { pos = append(pos, b"false"); }
                    }
                    start = idx + 1;
                }
                idx += 1;
            }
        }

        pos = append(pos, b"}}");
        unsafe {
            let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
            respond(200, resp, "application/json");
        }
        return;
    }

    respond(405, r#"{"error":"method not allowed"}"#, "application/json");
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
