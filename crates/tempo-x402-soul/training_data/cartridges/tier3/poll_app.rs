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

const POLL_OPTIONS: &[&str] = &["Rust", "Python", "TypeScript", "Go"];
const POLL_COLORS: &[&str] = &["#e94560", "#58a6ff", "#3fb950", "#d29922"];

fn make_vote_key(option: &str) -> &'static str {
    static mut VK_BUF: [u8; 64] = [0u8; 64];
    unsafe {
        let prefix = b"vote_";
        let mut kp = 0;
        let mut ki = 0;
        while ki < prefix.len() { VK_BUF[kp] = prefix[ki]; kp += 1; ki += 1; }
        let ob = option.as_bytes();
        ki = 0;
        while ki < ob.len() && kp < VK_BUF.len() { VK_BUF[kp] = ob[ki]; kp += 1; ki += 1; }
        core::str::from_utf8(&VK_BUF[..kp]).unwrap_or("vote_")
    }
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };
    let method = find_json_str(request, b"method").unwrap_or("GET");
    let path = find_json_str(request, b"path").unwrap_or("/");
    let path_bytes = path.as_bytes();

    host_log(0, "poll_app: handling request");

    // POST /vote/{option} — cast a vote
    if bytes_equal(method.as_bytes(), b"POST") && starts_with(path_bytes, b"/vote/") {
        let option = unsafe { core::str::from_utf8(&path_bytes[6..]).unwrap_or("") };

        // Validate option
        let mut valid = false;
        let mut oi = 0;
        while oi < POLL_OPTIONS.len() {
            if bytes_equal(option.as_bytes(), POLL_OPTIONS[oi].as_bytes()) {
                valid = true;
            }
            oi += 1;
        }

        if !valid {
            respond(400, r#"{"error":"invalid option","valid":["Rust","Python","TypeScript","Go"]}"#, "application/json");
            return;
        }

        let vote_key = make_vote_key(option);
        let count = match kv_read(vote_key) {
            Some(s) => parse_u64(s),
            None => 0,
        };
        let new_count = count + 1;
        let mut nb = [0u8; 20];
        let nl = write_u64(&mut nb, new_count);
        let ns = unsafe { core::str::from_utf8(&nb[..nl]).unwrap_or("0") };
        kv_write(vote_key, ns);

        let mut pos = 0;
        pos = append(pos, b"{\"voted\":\"");
        pos = append(pos, option.as_bytes());
        pos = append(pos, b"\",\"count\":");
        pos = append(pos, &nb[..nl]);
        pos = append(pos, b"}");
        unsafe {
            let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
            respond(200, resp, "application/json");
        }
        return;
    }

    // GET: show poll results as HTML bar chart
    let mut votes = [0u64; 4];
    let mut total: u64 = 0;
    let mut oi = 0;
    while oi < POLL_OPTIONS.len() {
        let vote_key = make_vote_key(POLL_OPTIONS[oi]);
        votes[oi] = match kv_read(vote_key) {
            Some(s) => parse_u64(s),
            None => 0,
        };
        total += votes[oi];
        oi += 1;
    }

    let mut pos = 0;
    pos = append(pos, b"<html><head><title>Poll: Favorite Language</title>");
    pos = append(pos, b"<style>body{font-family:sans-serif;max-width:600px;margin:40px auto;background:#0d1117;color:#c9d1d9}");
    pos = append(pos, b"h1{color:#58a6ff}h2{color:#8b949e;font-weight:normal}");
    pos = append(pos, b".option{margin:16px 0}.label{display:flex;justify-content:space-between;margin-bottom:4px}");
    pos = append(pos, b".bar-bg{background:#21262d;border-radius:4px;height:32px;overflow:hidden}");
    pos = append(pos, b".bar{height:100%;border-radius:4px;transition:width 0.3s}");
    pos = append(pos, b".vote-btn{display:inline-block;margin-top:4px;padding:4px 12px;background:#21262d;color:#58a6ff;border:1px solid #30363d;border-radius:4px;cursor:pointer;text-decoration:none;font-size:12px}");
    pos = append(pos, b".vote-btn:hover{background:#30363d}");
    pos = append(pos, b".total{color:#8b949e;margin-top:20px}");
    pos = append(pos, b"</style></head><body>");
    pos = append(pos, b"<h1>What's your favorite language?</h1>");
    pos = append(pos, b"<h2>Click to vote, results update in real-time</h2>");

    oi = 0;
    while oi < POLL_OPTIONS.len() {
        let pct = if total > 0 { (votes[oi] * 100) / total } else { 0 };
        let mut pct_buf = [0u8; 20];
        let pct_len = write_u64(&mut pct_buf, pct);
        let mut count_buf = [0u8; 20];
        let count_len = write_u64(&mut count_buf, votes[oi]);

        pos = append(pos, b"<div class=\"option\"><div class=\"label\"><span>");
        pos = append(pos, POLL_OPTIONS[oi].as_bytes());
        pos = append(pos, b"</span><span>");
        pos = append(pos, &count_buf[..count_len]);
        pos = append(pos, b" votes (");
        pos = append(pos, &pct_buf[..pct_len]);
        pos = append(pos, b"%)</span></div>");
        pos = append(pos, b"<div class=\"bar-bg\"><div class=\"bar\" style=\"width:");
        pos = append(pos, &pct_buf[..pct_len]);
        pos = append(pos, b"%;background:");
        pos = append(pos, POLL_COLORS[oi].as_bytes());
        pos = append(pos, b"\"></div></div>");
        pos = append(pos, b"<a class=\"vote-btn\" href=\"javascript:void(0)\" onclick=\"vote('");
        pos = append(pos, POLL_OPTIONS[oi].as_bytes());
        pos = append(pos, b"')\">Vote</a></div>");

        oi += 1;
    }

    let mut total_buf = [0u8; 20];
    let total_len = write_u64(&mut total_buf, total);
    pos = append(pos, b"<p class=\"total\">Total votes: ");
    pos = append(pos, &total_buf[..total_len]);
    pos = append(pos, b"</p>");

    pos = append(pos, b"<script>function vote(opt){fetch('/vote/'+opt,{method:'POST'}).then(()=>location.reload())}</script>");
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
