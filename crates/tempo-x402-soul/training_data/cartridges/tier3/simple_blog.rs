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

/// Posts stored as:
/// - "post_count" = number of posts
/// - "post_slug_{n}" = the slug for post n
/// - "post_title_{slug}" = title
/// - "post_body_{slug}" = body content
fn make_kv_key(prefix: &str, suffix: &str) -> &'static str {
    static mut KK_BUF: [u8; 128] = [0u8; 128];
    unsafe {
        let mut kp = 0;
        let pb = prefix.as_bytes();
        let mut i = 0;
        while i < pb.len() && kp < KK_BUF.len() { KK_BUF[kp] = pb[i]; kp += 1; i += 1; }
        let sb = suffix.as_bytes();
        i = 0;
        while i < sb.len() && kp < KK_BUF.len() { KK_BUF[kp] = sb[i]; kp += 1; i += 1; }
        core::str::from_utf8(&KK_BUF[..kp]).unwrap_or("")
    }
}

fn make_slug(title: &str) -> &'static str {
    static mut SLUG_BUF: [u8; 64] = [0u8; 64];
    unsafe {
        let tb = title.as_bytes();
        let mut sp = 0;
        let mut i = 0;
        while i < tb.len() && sp < 60 {
            let c = tb[i];
            if c >= b'a' && c <= b'z' {
                SLUG_BUF[sp] = c;
                sp += 1;
            } else if c >= b'A' && c <= b'Z' {
                SLUG_BUF[sp] = c + 32; // lowercase
                sp += 1;
            } else if c >= b'0' && c <= b'9' {
                SLUG_BUF[sp] = c;
                sp += 1;
            } else if c == b' ' || c == b'-' || c == b'_' {
                if sp > 0 && SLUG_BUF[sp - 1] != b'-' {
                    SLUG_BUF[sp] = b'-';
                    sp += 1;
                }
            }
            i += 1;
        }
        // Trim trailing dash
        if sp > 0 && SLUG_BUF[sp - 1] == b'-' { sp -= 1; }
        core::str::from_utf8(&SLUG_BUF[..sp]).unwrap_or("post")
    }
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };
    let method = find_json_str(request, b"method").unwrap_or("GET");
    let path = find_json_str(request, b"path").unwrap_or("/");
    let body = find_json_str(request, b"body").unwrap_or("");
    let path_bytes = path.as_bytes();

    host_log(0, "simple_blog: handling request");

    // POST /posts — create a new post
    if bytes_equal(method.as_bytes(), b"POST") && bytes_equal(path_bytes, b"/posts") {
        let title = find_json_str(body.as_bytes(), b"title").unwrap_or("");
        let content = find_json_str(body.as_bytes(), b"content").unwrap_or("");

        if title.is_empty() {
            respond(400, r#"{"error":"title is required"}"#, "application/json");
            return;
        }

        let slug = make_slug(title);
        let count = match kv_read("post_count") {
            Some(s) => parse_u64(s),
            None => 0,
        };
        let new_count = count + 1;

        // Store slug in ordered list
        let mut idx_key_buf = [0u8; 32];
        let mut ikp = 0;
        let ik_prefix = b"post_slug_";
        let mut i = 0;
        while i < ik_prefix.len() { idx_key_buf[ikp] = ik_prefix[i]; ikp += 1; i += 1; }
        let mut nb = [0u8; 20];
        let nl = write_u64(&mut nb, new_count);
        i = 0;
        while i < nl { idx_key_buf[ikp] = nb[i]; ikp += 1; i += 1; }
        let idx_key = unsafe { core::str::from_utf8(&idx_key_buf[..ikp]).unwrap_or("") };
        kv_write(idx_key, slug);

        // Store title and body
        let title_key = make_kv_key("post_title_", slug);
        kv_write(title_key, title);

        let body_key = make_kv_key("post_body_", slug);
        kv_write(body_key, content);

        // Update count
        let count_str = unsafe { core::str::from_utf8(&nb[..nl]).unwrap_or("0") };
        kv_write("post_count", count_str);

        let mut pos = 0;
        pos = append(pos, b"{\"created\":true,\"slug\":\"");
        pos = append(pos, slug.as_bytes());
        pos = append(pos, b"\"}");
        unsafe {
            let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
            respond(201, resp, "application/json");
        }
        return;
    }

    // GET /posts — list all posts as HTML
    if bytes_equal(path_bytes, b"/posts") || bytes_equal(path_bytes, b"/") {
        let count = match kv_read("post_count") {
            Some(s) => parse_u64(s),
            None => 0,
        };

        let mut pos = 0;
        pos = append(pos, b"<html><head><title>Blog</title>");
        pos = append(pos, b"<style>body{font-family:Georgia,serif;max-width:700px;margin:40px auto;background:#fafafa;color:#333;padding:0 20px}");
        pos = append(pos, b"h1{color:#222;border-bottom:2px solid #222;padding-bottom:10px}");
        pos = append(pos, b".post-link{display:block;padding:12px 0;border-bottom:1px solid #ddd;color:#0066cc;text-decoration:none;font-size:18px}");
        pos = append(pos, b".post-link:hover{color:#004499}");
        pos = append(pos, b".empty{color:#999;font-style:italic}");
        pos = append(pos, b"</style></head><body>");
        pos = append(pos, b"<h1>Blog</h1>");

        if count == 0 {
            pos = append(pos, b"<p class=\"empty\">No posts yet.</p>");
        } else {
            let mut idx = count;
            while idx >= 1 {
                let mut idx_key_buf = [0u8; 32];
                let mut ikp = 0;
                let ik_prefix = b"post_slug_";
                let mut i = 0;
                while i < ik_prefix.len() { idx_key_buf[ikp] = ik_prefix[i]; ikp += 1; i += 1; }
                let mut nb = [0u8; 20];
                let nl = write_u64(&mut nb, idx);
                i = 0;
                while i < nl { idx_key_buf[ikp] = nb[i]; ikp += 1; i += 1; }
                let idx_key = unsafe { core::str::from_utf8(&idx_key_buf[..ikp]).unwrap_or("") };

                if let Some(slug) = kv_read(idx_key) {
                    let title_key = make_kv_key("post_title_", slug);
                    let title = kv_read(title_key).unwrap_or(slug);

                    pos = append(pos, b"<a class=\"post-link\" href=\"/posts/");
                    pos = append(pos, slug.as_bytes());
                    pos = append(pos, b"\">");
                    pos = append(pos, title.as_bytes());
                    pos = append(pos, b"</a>");
                }
                idx -= 1;
            }
        }

        pos = append(pos, b"</body></html>");
        unsafe {
            let html = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("error");
            respond(200, html, "text/html");
        }
        return;
    }

    // GET /posts/{slug} — show a single post
    if starts_with(path_bytes, b"/posts/") {
        let slug = unsafe { core::str::from_utf8(&path_bytes[7..]).unwrap_or("") };

        let title_key = make_kv_key("post_title_", slug);
        let body_key = make_kv_key("post_body_", slug);

        match kv_read(title_key) {
            Some(title) => {
                let content = kv_read(body_key).unwrap_or("");

                let mut pos = 0;
                pos = append(pos, b"<html><head><title>");
                pos = append(pos, title.as_bytes());
                pos = append(pos, b"</title>");
                pos = append(pos, b"<style>body{font-family:Georgia,serif;max-width:700px;margin:40px auto;background:#fafafa;color:#333;padding:0 20px;line-height:1.7}");
                pos = append(pos, b"h1{color:#222}a{color:#0066cc}.back{margin-bottom:20px;display:block}</style></head><body>");
                pos = append(pos, b"<a class=\"back\" href=\"/posts\">&larr; All Posts</a>");
                pos = append(pos, b"<h1>");
                pos = append(pos, title.as_bytes());
                pos = append(pos, b"</h1><article>");
                pos = append(pos, content.as_bytes());
                pos = append(pos, b"</article></body></html>");

                unsafe {
                    let html = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("error");
                    respond(200, html, "text/html");
                }
            }
            None => {
                respond(404, r#"{"error":"post not found"}"#, "application/json");
            }
        }
        return;
    }

    respond(404, r#"{"error":"not found"}"#, "application/json");
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
