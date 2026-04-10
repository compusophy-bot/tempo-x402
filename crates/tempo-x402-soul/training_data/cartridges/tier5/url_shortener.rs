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

fn find_json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let key_bytes = key.as_bytes();
    let json_bytes = json.as_bytes();
    let mut i = 0;
    while i + key_bytes.len() + 3 < json_bytes.len() {
        if json_bytes[i] == b'"' {
            let start = i + 1;
            if start + key_bytes.len() < json_bytes.len()
                && &json_bytes[start..start + key_bytes.len()] == key_bytes
                && json_bytes[start + key_bytes.len()] == b'"'
            {
                let mut j = start + key_bytes.len() + 1;
                while j < json_bytes.len() && (json_bytes[j] == b':' || json_bytes[j] == b' ') { j += 1; }
                if j < json_bytes.len() && json_bytes[j] == b'"' {
                    let val_start = j + 1;
                    let mut val_end = val_start;
                    while val_end < json_bytes.len() && json_bytes[val_end] != b'"' { val_end += 1; }
                    return core::str::from_utf8(&json_bytes[val_start..val_end]).ok();
                }
            }
        }
        i += 1;
    }
    None
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

static mut BUF: [u8; 65536] = [0u8; 65536];

struct BufWriter { pos: usize }
impl BufWriter {
    fn new() -> Self { Self { pos: 0 } }
    fn push_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        unsafe {
            let end = (self.pos + bytes.len()).min(BUF.len());
            BUF[self.pos..end].copy_from_slice(&bytes[..end - self.pos]);
            self.pos = end;
        }
    }
    fn push_num(&mut self, mut n: u32) {
        if n == 0 { self.push_str("0"); return; }
        let mut d = [0u8; 10];
        let mut i = 0;
        while n > 0 { d[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
        while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = d[i]; self.pos += 1; } } }
    }
    fn as_str(&self) -> &str { unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) } }
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}

fn parse_u32(s: &str) -> u32 {
    let b = s.as_bytes();
    let mut r: u32 = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] >= b'0' && b[i] <= b'9' { r = r * 10 + (b[i] - b'0') as u32; }
        i += 1;
    }
    r
}

/// Simple hash to generate short code from counter
fn num_to_code(buf: &mut [u8; 8], mut n: u32) -> usize {
    static CHARS: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    if n == 0 { buf[0] = b'a'; return 1; }
    let mut len = 0;
    while n > 0 && len < 7 {
        buf[len] = CHARS[(n % 31) as usize];
        n /= 31;
        len += 1;
    }
    len
}

fn make_key(prefix: &str, code: &str, buf: &mut [u8; 64]) -> &str {
    let pb = prefix.as_bytes();
    let cb = code.as_bytes();
    let mut p = 0;
    let mut i = 0;
    while i < pb.len() && p < 64 { buf[p] = pb[i]; p += 1; i += 1; }
    i = 0;
    while i < cb.len() && p < 64 { buf[p] = cb[i]; p += 1; i += 1; }
    unsafe { core::str::from_utf8_unchecked(&buf[..p]) }
}

fn push_escaped(w: &mut BufWriter, s: &str) {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'<' => w.push_str("&lt;"),
            b'>' => w.push_str("&gt;"),
            b'&' => w.push_str("&amp;"),
            b'"' => w.push_str("&quot;"),
            _ => unsafe {
                if w.pos < BUF.len() { BUF[w.pos] = b[i]; w.pos += 1; }
            }
        }
        i += 1;
    }
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize))
    };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let path = find_json_str(request, "path").unwrap_or("/");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(0, "url_shortener: handling request");

    // POST /create — create new short link
    if method == "POST" {
        let url = find_json_str(body, "url").unwrap_or("");
        let custom_code = find_json_str(body, "code");

        if url.is_empty() {
            respond(400, r#"{"error":"url required"}"#, "application/json");
            return;
        }

        // Get or generate code
        let count = match kv_read("url_count") { Some(s) => parse_u32(s), None => 0 };
        let new_count = count + 1;

        let mut code_buf = [0u8; 8];
        let code_len;
        let code: &str;

        if let Some(c) = custom_code {
            // Use custom code
            let cb = c.as_bytes();
            let mut i = 0;
            while i < cb.len() && i < 8 { code_buf[i] = cb[i]; i += 1; }
            code_len = i;
            code = unsafe { core::str::from_utf8_unchecked(&code_buf[..code_len]) };

            // Check if already taken
            let mut key_buf = [0u8; 64];
            let key = make_key("url_", code, &mut key_buf);
            if kv_read(key).is_some() {
                respond(409, r#"{"error":"code already taken"}"#, "application/json");
                return;
            }
        } else {
            code_len = num_to_code(&mut code_buf, new_count);
            code = unsafe { core::str::from_utf8_unchecked(&code_buf[..code_len]) };
        }

        // Store URL under url_{code}
        let mut key_buf = [0u8; 64];
        let key = make_key("url_", code, &mut key_buf);
        kv_write(key, url);

        // Store click count under clicks_{code}
        let mut ck_buf = [0u8; 64];
        let ck_key = make_key("clicks_", code, &mut ck_buf);
        kv_write(ck_key, "0");

        // Store in recent list
        let mut idx_key_buf = [0u8; 32];
        let slot = (new_count - 1) % 10; // circular buffer of last 10
        let mut ikp = 0;
        let pfix = b"url_recent_";
        let mut pi = 0;
        while pi < pfix.len() { idx_key_buf[ikp] = pfix[pi]; ikp += 1; pi += 1; }
        idx_key_buf[ikp] = b'0' + slot as u8; ikp += 1;
        let idx_key = unsafe { core::str::from_utf8_unchecked(&idx_key_buf[..ikp]) };

        // Store as "code|url"
        let mut entry_buf = [0u8; 512];
        let mut ep = 0;
        let mut i = 0;
        while i < code_len { entry_buf[ep] = code_buf[i]; ep += 1; i += 1; }
        entry_buf[ep] = b'|'; ep += 1;
        let ub = url.as_bytes();
        i = 0;
        while i < ub.len() && ep < 510 { entry_buf[ep] = ub[i]; ep += 1; i += 1; }
        let entry = unsafe { core::str::from_utf8_unchecked(&entry_buf[..ep]) };
        kv_write(idx_key, entry);

        // Update count
        let mut num_buf = [0u8; 12];
        let mut np = 0;
        let mut n = new_count;
        let mut d = [0u8; 10]; let mut di = 0;
        while n > 0 { d[di] = b'0' + (n % 10) as u8; n /= 10; di += 1; }
        while di > 0 { di -= 1; num_buf[np] = d[di]; np += 1; }
        let cs = unsafe { core::str::from_utf8_unchecked(&num_buf[..np]) };
        kv_write("url_count", cs);

        // Return short code
        let mut w = BufWriter::new();
        w.push_str(r#"{"code":""#);
        w.push_str(code);
        w.push_str(r#"","url":""#);
        w.push_str(url);
        w.push_str(r#"","clicks":0}"#);
        respond(200, w.as_str(), "application/json");
        return;
    }

    // GET /stats/{code} — analytics
    let pb = path.as_bytes();
    if pb.len() > 7 && pb[0] == b'/' && pb[1] == b's' && pb[2] == b't' && pb[3] == b'a' && pb[4] == b't' && pb[5] == b's' && pb[6] == b'/' {
        let code = unsafe { core::str::from_utf8_unchecked(&pb[7..]) };
        let mut key_buf = [0u8; 64];
        let key = make_key("url_", code, &mut key_buf);

        if let Some(url) = kv_read(key) {
            let mut ck_buf = [0u8; 64];
            let ck_key = make_key("clicks_", code, &mut ck_buf);
            let clicks = kv_read(ck_key).unwrap_or("0");

            let mut w = BufWriter::new();
            w.push_str(r#"{"code":""#);
            w.push_str(code);
            w.push_str(r#"","url":""#);
            w.push_str(url);
            w.push_str(r#"","clicks":"#);
            w.push_str(clicks);
            w.push_str("}");
            respond(200, w.as_str(), "application/json");
        } else {
            respond(404, r#"{"error":"not found"}"#, "application/json");
        }
        return;
    }

    // GET /{code} — redirect (any path that is not / and not /stats/)
    if path.len() > 1 && !path.contains('/') || (pb.len() > 1 && pb[0] == b'/' && !path[1..].contains('/')) {
        let code = if pb[0] == b'/' { unsafe { core::str::from_utf8_unchecked(&pb[1..]) } } else { path };

        if !code.is_empty() && code != "api" && code != "stats" {
            let mut key_buf = [0u8; 64];
            let key = make_key("url_", code, &mut key_buf);

            if let Some(url) = kv_read(key) {
                // Increment click count
                let mut ck_buf = [0u8; 64];
                let ck_key = make_key("clicks_", code, &mut ck_buf);
                let clicks = match kv_read(ck_key) { Some(s) => parse_u32(s), None => 0 };
                let new_clicks = clicks + 1;
                let mut nb = [0u8; 12]; let mut np = 0;
                let mut n = new_clicks;
                let mut dd = [0u8; 10]; let mut di = 0;
                while n > 0 { dd[di] = b'0' + (n % 10) as u8; n /= 10; di += 1; }
                while di > 0 { di -= 1; nb[np] = dd[di]; np += 1; }
                let cs = unsafe { core::str::from_utf8_unchecked(&nb[..np]) };
                kv_write(ck_key, cs);

                // Return redirect HTML
                let mut w = BufWriter::new();
                w.push_str("<!DOCTYPE html><html><head><meta http-equiv='refresh' content='0;url=");
                w.push_str(url);
                w.push_str("'><title>Redirecting...</title></head><body><p>Redirecting to <a href='");
                w.push_str(url);
                w.push_str("'>");
                push_escaped(&mut w, url);
                w.push_str("</a>...</p></body></html>");
                respond(200, w.as_str(), "text/html");
                return;
            }
        }
    }

    // GET / — main page
    let mut w = BufWriter::new();
    w.push_str("<!DOCTYPE html><html><head><meta charset='utf-8'><title>URL Shortener</title><style>");
    w.push_str("*{margin:0;padding:0;box-sizing:border-box}");
    w.push_str("body{font-family:'Segoe UI',system-ui,sans-serif;background:#0a0a1a;color:#e0e0e0;min-height:100vh;display:flex;flex-direction:column;align-items:center;padding:40px 20px}");
    w.push_str("h1{font-size:2.5rem;color:#7c4dff;margin-bottom:8px;text-shadow:0 0 30px rgba(124,77,255,0.3)}");
    w.push_str(".subtitle{color:#888;margin-bottom:30px;font-size:1rem}");
    w.push_str(".card{background:#111;border:1px solid #333;border-radius:12px;padding:24px;width:100%;max-width:600px;margin-bottom:20px}");
    w.push_str(".card h2{color:#e0e0e0;font-size:1.1rem;margin-bottom:16px}");
    w.push_str(".form-row{display:flex;gap:10px;margin-bottom:12px}");
    w.push_str("input[type=text]{flex:1;padding:10px 14px;border:1px solid #333;border-radius:8px;background:#1a1a3e;color:#e0e0e0;font-size:0.9rem}");
    w.push_str("input:focus{outline:none;border-color:#7c4dff}");
    w.push_str("button{padding:10px 20px;border:none;border-radius:8px;background:#7c4dff;color:#fff;font-weight:600;cursor:pointer;transition:background 0.2s;font-size:0.9rem}");
    w.push_str("button:hover{background:#651fff}");
    w.push_str(".result{display:none;background:#1a1a3e;border:1px solid #4caf50;border-radius:8px;padding:16px;margin-top:12px}");
    w.push_str(".result .short-url{font-size:1.2rem;color:#4caf50;font-weight:bold;word-break:break-all}");
    w.push_str(".result .copy-btn{background:#4caf50;padding:6px 14px;font-size:0.8rem;margin-top:8px}");
    w.push_str(".recent{width:100%;max-width:600px}");
    w.push_str(".recent h2{color:#e0e0e0;font-size:1.1rem;margin-bottom:12px}");
    w.push_str(".link-row{display:flex;justify-content:space-between;align-items:center;padding:10px 14px;background:#111;border:1px solid #222;border-radius:8px;margin-bottom:6px}");
    w.push_str(".link-row .code{color:#7c4dff;font-weight:bold;font-family:monospace}");
    w.push_str(".link-row .url{color:#888;font-size:0.85rem;flex:1;margin:0 12px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}");
    w.push_str(".link-row .clicks{color:#4caf50;font-size:0.85rem;min-width:60px;text-align:right}");
    w.push_str(".stats-box{display:flex;gap:20px;margin-bottom:24px}");
    w.push_str(".stat{background:#111;border:1px solid #333;border-radius:8px;padding:16px;flex:1;text-align:center}");
    w.push_str(".stat .num{font-size:2rem;color:#7c4dff;font-weight:bold}.stat .label{color:#888;font-size:0.8rem;margin-top:4px}");
    w.push_str("</style></head><body>");

    w.push_str("<h1>x402.link</h1><p class='subtitle'>Shorten any URL, track every click</p>");

    // Stats
    w.push_str("<div class='stats-box' style='width:100%;max-width:600px'>");
    w.push_str("<div class='stat'><div class='num' id='totalLinks'>0</div><div class='label'>Links Created</div></div>");
    w.push_str("<div class='stat'><div class='num' id='totalClicks'>0</div><div class='label'>Total Clicks</div></div></div>");

    // Create form
    w.push_str("<div class='card'><h2>Shorten a URL</h2>");
    w.push_str("<div class='form-row'><input type='text' id='urlInput' placeholder='https://example.com/very-long-url'></div>");
    w.push_str("<div class='form-row'><input type='text' id='codeInput' placeholder='Custom code (optional)' style='max-width:200px'><button onclick='shorten()'>Shorten</button></div>");
    w.push_str("<div class='result' id='result'><div>Short URL:</div><div class='short-url' id='shortUrl'></div>");
    w.push_str("<button class='copy-btn' onclick='copyUrl()'>Copy</button></div></div>");

    // Recent links
    w.push_str("<div class='recent'><h2>Recent Links</h2><div id='recentList'>Loading...</div></div>");

    w.push_str("<script>");
    w.push_str("const base=window.location.origin+window.location.pathname;");

    w.push_str("async function shorten(){");
    w.push_str("const url=document.getElementById('urlInput').value.trim();if(!url)return;");
    w.push_str("const code=document.getElementById('codeInput').value.trim();");
    w.push_str("const body={url};if(code)body.code=code;");
    w.push_str("try{const r=await fetch(window.location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});");
    w.push_str("const d=await r.json();if(d.error){alert(d.error);return;}");
    w.push_str("const short=base+'/'+d.code;document.getElementById('shortUrl').textContent=short;");
    w.push_str("document.getElementById('result').style.display='block';loadRecent();}catch(e){console.error(e);}}");

    w.push_str("function copyUrl(){const t=document.getElementById('shortUrl').textContent;navigator.clipboard.writeText(t);}");

    w.push_str("async function loadRecent(){try{");
    w.push_str("const count=parseInt(document.getElementById('totalLinks').textContent)||0;");
    // Load recent entries by iterating the circular buffer
    w.push_str("let html='';let totalC=0;for(let i=0;i<10;i++){");
    w.push_str("try{const r=await fetch(window.location.pathname+'/stats/'+i);if(!r.ok)continue;");
    w.push_str("}catch(e){}}");
    // Simpler: just show placeholder since we can't easily list from client
    w.push_str("document.getElementById('recentList').innerHTML=html||'<div style=\"color:#555\">Create your first short link above</div>';");
    w.push_str("}catch(e){}}");

    w.push_str("document.getElementById('totalLinks').textContent='");
    let count = match kv_read("url_count") { Some(s) => s, None => "0" };
    w.push_str(count);
    w.push_str("';");

    // Load recent links from KV on page render
    let total_count = parse_u32(count);
    if total_count > 0 {
        w.push_str("document.getElementById('recentList').innerHTML='");
        let start = if total_count > 10 { total_count - 10 } else { 0 };
        let mut slot = start;
        while slot < total_count && slot < start + 10 {
            let idx = slot % 10;
            let mut idx_key_buf = [0u8; 32];
            let mut ikp = 0;
            let pfix = b"url_recent_";
            let mut pi = 0;
            while pi < pfix.len() { idx_key_buf[ikp] = pfix[pi]; ikp += 1; pi += 1; }
            idx_key_buf[ikp] = b'0' + idx as u8; ikp += 1;
            let idx_key = unsafe { core::str::from_utf8_unchecked(&idx_key_buf[..ikp]) };

            if let Some(entry) = kv_read(idx_key) {
                let eb = entry.as_bytes();
                let mut sp = 0;
                while sp < eb.len() && eb[sp] != b'|' { sp += 1; }
                let code_part = unsafe { core::str::from_utf8_unchecked(&eb[..sp]) };
                let url_part = if sp + 1 < eb.len() { unsafe { core::str::from_utf8_unchecked(&eb[sp+1..]) } } else { "" };

                w.push_str("<div class=\"link-row\"><span class=\"code\">");
                push_escaped(&mut w, code_part);
                w.push_str("</span><span class=\"url\">");
                push_escaped(&mut w, url_part);
                w.push_str("</span></div>");
            }
            slot += 1;
        }
        w.push_str("';");
    }

    w.push_str("document.getElementById('urlInput').addEventListener('keydown',e=>{if(e.key==='Enter')shorten();});");
    w.push_str("</script></body></html>");

    respond(200, w.as_str(), "text/html");
}
