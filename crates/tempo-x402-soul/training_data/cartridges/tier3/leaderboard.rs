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

/// Leaderboard stores entries as:
/// - "lb_count" = total entries
/// - "lb_name_{n}" = player name
/// - "lb_score_{n}" = player score
/// On GET, we load all entries, sort top 10 by score (insertion sort on static arrays).
const MAX_ENTRIES: usize = 100;

fn make_name_key(id: u64) -> &'static str {
    static mut NK_BUF: [u8; 32] = [0u8; 32];
    unsafe {
        let prefix = b"lb_name_";
        let mut kp = 0;
        let mut ki = 0;
        while ki < prefix.len() { NK_BUF[kp] = prefix[ki]; kp += 1; ki += 1; }
        let mut nb = [0u8; 20];
        let nl = write_u64(&mut nb, id);
        ki = 0;
        while ki < nl { NK_BUF[kp] = nb[ki]; kp += 1; ki += 1; }
        core::str::from_utf8(&NK_BUF[..kp]).unwrap_or("lb_name_0")
    }
}

fn make_score_key(id: u64) -> &'static str {
    static mut SK_BUF: [u8; 32] = [0u8; 32];
    unsafe {
        let prefix = b"lb_score_";
        let mut kp = 0;
        let mut ki = 0;
        while ki < prefix.len() { SK_BUF[kp] = prefix[ki]; kp += 1; ki += 1; }
        let mut nb = [0u8; 20];
        let nl = write_u64(&mut nb, id);
        ki = 0;
        while ki < nl { SK_BUF[kp] = nb[ki]; kp += 1; ki += 1; }
        core::str::from_utf8(&SK_BUF[..kp]).unwrap_or("lb_score_0")
    }
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };
    let method = find_json_str(request, b"method").unwrap_or("GET");
    let path = find_json_str(request, b"path").unwrap_or("/");
    let body = find_json_str(request, b"body").unwrap_or("");
    let path_bytes = path.as_bytes();

    host_log(0, "leaderboard: handling request");

    // POST /score — submit a score
    if bytes_equal(method.as_bytes(), b"POST") && bytes_equal(path_bytes, b"/score") {
        let name = find_json_str(body.as_bytes(), b"name").unwrap_or("");
        let score_str = find_json_str(body.as_bytes(), b"score").unwrap_or("0");

        if name.is_empty() {
            respond(400, r#"{"error":"name is required"}"#, "application/json");
            return;
        }

        let score = parse_u64(score_str);
        let count = match kv_read("lb_count") {
            Some(s) => parse_u64(s),
            None => 0,
        };

        // Check if player already has an entry, update if higher score
        let mut existing_idx: u64 = 0;
        let mut idx: u64 = 1;
        while idx <= count {
            let nk = make_name_key(idx);
            if let Some(existing_name) = kv_read(nk) {
                if bytes_equal(existing_name.as_bytes(), name.as_bytes()) {
                    existing_idx = idx;
                }
            }
            idx += 1;
        }

        if existing_idx > 0 {
            // Update existing entry if score is higher
            let sk = make_score_key(existing_idx);
            let old_score = match kv_read(sk) {
                Some(s) => parse_u64(s),
                None => 0,
            };

            if score > old_score {
                let mut sb = [0u8; 20];
                let sl = write_u64(&mut sb, score);
                let ss = unsafe { core::str::from_utf8(&sb[..sl]).unwrap_or("0") };
                kv_write(sk, ss);

                let mut pos = 0;
                pos = append(pos, b"{\"updated\":true,\"name\":\"");
                pos = append(pos, name.as_bytes());
                pos = append(pos, b"\",\"score\":");
                pos = append(pos, &sb[..sl]);
                pos = append(pos, b",\"previous\":");
                let mut ob = [0u8; 20];
                let ol = write_u64(&mut ob, old_score);
                pos = append(pos, &ob[..ol]);
                pos = append(pos, b"}");
                unsafe {
                    let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
                    respond(200, resp, "application/json");
                }
            } else {
                respond(200, r#"{"updated":false,"reason":"existing score is higher or equal"}"#, "application/json");
            }
        } else {
            // New entry
            if count >= MAX_ENTRIES as u64 {
                respond(400, r#"{"error":"leaderboard is full"}"#, "application/json");
                return;
            }

            let new_id = count + 1;
            let nk = make_name_key(new_id);
            kv_write(nk, name);

            let sk = make_score_key(new_id);
            let mut sb = [0u8; 20];
            let sl = write_u64(&mut sb, score);
            let ss = unsafe { core::str::from_utf8(&sb[..sl]).unwrap_or("0") };
            kv_write(sk, ss);

            let mut cb = [0u8; 20];
            let cl = write_u64(&mut cb, new_id);
            let cs = unsafe { core::str::from_utf8(&cb[..cl]).unwrap_or("0") };
            kv_write("lb_count", cs);

            let mut pos = 0;
            pos = append(pos, b"{\"submitted\":true,\"name\":\"");
            pos = append(pos, name.as_bytes());
            pos = append(pos, b"\",\"score\":");
            pos = append(pos, &sb[..sl]);
            pos = append(pos, b",\"rank\":\"pending\"}");
            unsafe {
                let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
                respond(201, resp, "application/json");
            }
        }
        return;
    }

    // GET /leaderboard — show top 10 as HTML table
    let count = match kv_read("lb_count") {
        Some(s) => parse_u64(s),
        None => 0,
    };

    // Load all scores into static arrays, then sort
    static mut SCORES: [u64; 100] = [0u64; 100];
    static mut INDICES: [u64; 100] = [0u64; 100];
    let mut loaded: usize = 0;

    unsafe {
        let mut idx: u64 = 1;
        while idx <= count && loaded < MAX_ENTRIES {
            let sk = make_score_key(idx);
            if let Some(s) = kv_read(sk) {
                let sc = parse_u64(s);
                // Check name isn't empty (deleted entry)
                let nk = make_name_key(idx);
                if let Some(nm) = kv_read(nk) {
                    if !nm.is_empty() {
                        SCORES[loaded] = sc;
                        INDICES[loaded] = idx;
                        loaded += 1;
                    }
                }
            }
            idx += 1;
        }

        // Insertion sort descending by score
        let mut i = 1;
        while i < loaded {
            let score_i = SCORES[i];
            let idx_i = INDICES[i];
            let mut j = i;
            while j > 0 && SCORES[j - 1] < score_i {
                SCORES[j] = SCORES[j - 1];
                INDICES[j] = INDICES[j - 1];
                j -= 1;
            }
            SCORES[j] = score_i;
            INDICES[j] = idx_i;
            i += 1;
        }
    }

    let mut pos = 0;
    pos = append(pos, b"<html><head><title>Leaderboard</title>");
    pos = append(pos, b"<style>body{font-family:sans-serif;max-width:600px;margin:40px auto;background:#0d1117;color:#c9d1d9}");
    pos = append(pos, b"h1{color:#f0883e;text-align:center}");
    pos = append(pos, b"table{width:100%;border-collapse:collapse;margin-top:20px}");
    pos = append(pos, b"th{background:#161b22;color:#58a6ff;padding:12px;text-align:left;border-bottom:2px solid #30363d}");
    pos = append(pos, b"td{padding:10px 12px;border-bottom:1px solid #21262d}");
    pos = append(pos, b"tr:hover{background:#161b22}");
    pos = append(pos, b".rank{font-weight:bold;color:#f0883e;width:50px}");
    pos = append(pos, b".gold{color:#ffd700}.silver{color:#c0c0c0}.bronze{color:#cd7f32}");
    pos = append(pos, b".score{text-align:right;font-family:monospace;font-size:16px;color:#3fb950}");
    pos = append(pos, b".empty{text-align:center;color:#8b949e;padding:40px}");
    pos = append(pos, b"</style></head><body>");
    pos = append(pos, b"<h1>Leaderboard</h1>");

    if loaded == 0 {
        pos = append(pos, b"<p class=\"empty\">No scores yet. POST to /score with {\"name\":\"...\",\"score\":\"...\"}</p>");
    } else {
        pos = append(pos, b"<table><tr><th>Rank</th><th>Player</th><th class=\"score\">Score</th></tr>");

        let show = if loaded > 10 { 10 } else { loaded };
        let mut i = 0;
        while i < show {
            let rank = i + 1;
            let entry_idx = unsafe { INDICES[i] };
            let entry_score = unsafe { SCORES[i] };

            let nk = make_name_key(entry_idx);
            let name = kv_read(nk).unwrap_or("???");

            pos = append(pos, b"<tr><td class=\"rank ");
            if rank == 1 { pos = append(pos, b"gold"); }
            else if rank == 2 { pos = append(pos, b"silver"); }
            else if rank == 3 { pos = append(pos, b"bronze"); }
            pos = append(pos, b"\">");

            let mut rb = [0u8; 20];
            let rl = write_u64(&mut rb, rank as u64);
            pos = append(pos, &rb[..rl]);

            pos = append(pos, b"</td><td>");
            pos = append(pos, name.as_bytes());
            pos = append(pos, b"</td><td class=\"score\">");

            let mut sb = [0u8; 20];
            let sl = write_u64(&mut sb, entry_score);
            pos = append(pos, &sb[..sl]);

            pos = append(pos, b"</td></tr>");
            i += 1;
        }

        pos = append(pos, b"</table>");

        // Show total entries
        let mut tb = [0u8; 20];
        let tl = write_u64(&mut tb, loaded as u64);
        pos = append(pos, b"<p style=\"color:#8b949e;text-align:center;margin-top:16px\">Total players: ");
        pos = append(pos, &tb[..tl]);
        pos = append(pos, b"</p>");
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
