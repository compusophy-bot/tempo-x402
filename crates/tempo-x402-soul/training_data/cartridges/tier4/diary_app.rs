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

fn respond(status: i32, body: &str, ct: &str) {
    unsafe { response(status, body.as_ptr(), body.len() as i32, ct.as_ptr(), ct.len() as i32); }
}
fn host_log(level: i32, msg: &str) { unsafe { log(level, msg.as_ptr(), msg.len() as i32); } }

fn kv_read(key: &str) -> Option<&'static str> {
    unsafe {
        let r = kv_get(key.as_ptr(), key.len() as i32);
        if r < 0 { return None; }
        let ptr = (r >> 32) as *const u8;
        let len = (r & 0xFFFFFFFF) as usize;
        core::str::from_utf8(core::slice::from_raw_parts(ptr, len)).ok()
    }
}
fn kv_write(key: &str, val: &str) {
    unsafe { kv_set(key.as_ptr(), key.len() as i32, val.as_ptr(), val.len() as i32); }
}

fn find_json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let kb = key.as_bytes(); let jb = json.as_bytes();
    let mut i = 0;
    while i + kb.len() + 3 < jb.len() {
        if jb[i] == b'"' {
            let s = i + 1;
            if s + kb.len() < jb.len() && &jb[s..s + kb.len()] == kb && jb[s + kb.len()] == b'"' {
                let mut j = s + kb.len() + 1;
                while j < jb.len() && (jb[j] == b':' || jb[j] == b' ') { j += 1; }
                if j < jb.len() && jb[j] == b'"' {
                    let vs = j + 1; let mut ve = vs;
                    while ve < jb.len() && jb[ve] != b'"' { ve += 1; }
                    return core::str::from_utf8(&jb[vs..ve]).ok();
                }
            }
        }
        i += 1;
    }
    None
}

static mut BUF: [u8; 65536] = [0u8; 65536];
struct BufWriter { pos: usize }
impl BufWriter {
    fn new() -> Self { Self { pos: 0 } }
    fn push(&mut self, s: &str) {
        let b = s.as_bytes();
        unsafe { let e = (self.pos + b.len()).min(BUF.len()); BUF[self.pos..e].copy_from_slice(&b[..e - self.pos]); self.pos = e; }
    }
    fn push_num(&mut self, mut n: u32) {
        if n == 0 { self.push("0"); return; }
        let mut d = [0u8; 10]; let mut i = 0;
        while n > 0 { d[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
        while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = d[i]; self.pos += 1; } } }
    }
    fn as_str(&self) -> &str { unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) } }
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];
#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 { unsafe { SCRATCH.as_mut_ptr() } }

fn parse_u32(s: &str) -> u32 {
    let mut n: u32 = 0;
    for &b in s.as_bytes() { if b >= b'0' && b <= b'9' { n = n * 10 + (b - b'0') as u32; } }
    n
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize)) };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let path = find_json_str(request, "path").unwrap_or("/");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(0, "diary: handling request");

    if method == "POST" {
        if let Some(date) = find_json_str(body, "date") {
            if let Some(entry) = find_json_str(body, "entry") {
                let mood = find_json_str(body, "mood").unwrap_or("neutral");
                // Store entry: date|mood|text
                let idx = kv_read("diary_count").map(|s| parse_u32(s)).unwrap_or(0);
                let mut w = BufWriter::new();
                w.push(date); w.push("|"); w.push(mood); w.push("|"); w.push(entry);
                // Key: diary_N
                let mut key_buf = [0u8; 20];
                let klen = write_key(&mut key_buf, b"diary_", idx);
                let key = unsafe { core::str::from_utf8_unchecked(&key_buf[..klen]) };
                kv_write(key, w.as_str());
                // Increment count
                let mut w2 = BufWriter::new();
                w2.push_num(idx + 1);
                kv_write("diary_count", w2.as_str());
                respond(200, r#"{"ok":true}"#, "application/json");
            } else {
                respond(400, r#"{"error":"missing entry"}"#, "application/json");
            }
        } else {
            respond(400, r#"{"error":"missing date"}"#, "application/json");
        }
        return;
    }

    // GET — render diary
    let count = kv_read("diary_count").map(|s| parse_u32(s)).unwrap_or(0);
    let mut w = BufWriter::new();
    w.push("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>My Diary</title><style>");
    w.push("*{margin:0;padding:0;box-sizing:border-box}body{background:#1a1a2e;color:#e0e0e0;font-family:Georgia,serif;padding:30px 20px;display:flex;justify-content:center}");
    w.push(".c{max-width:650px;width:100%}h1{text-align:center;color:#ffd700;margin-bottom:24px;font-style:italic}");
    w.push(".form{background:#16213e;padding:20px;border-radius:12px;margin-bottom:24px}");
    w.push(".form input,.form textarea,.form select{width:100%;padding:10px;background:#0f3460;border:1px solid #333;color:#e0e0e0;border-radius:6px;margin-bottom:10px;font-family:Georgia,serif;font-size:14px}");
    w.push(".form textarea{height:120px;resize:vertical}");
    w.push("button{padding:10px 24px;background:#e94560;color:#fff;border:none;border-radius:6px;cursor:pointer;font-size:14px}");
    w.push(".entry{background:#16213e;padding:18px;border-radius:12px;margin-bottom:12px;border-left:4px solid #e94560}");
    w.push(".entry-header{display:flex;justify-content:space-between;margin-bottom:8px;font-size:13px;color:#888}");
    w.push(".mood{padding:2px 8px;border-radius:10px;font-size:12px}");
    w.push(".mood-happy{background:#2d5a2d;color:#90ee90}.mood-sad{background:#5a2d2d;color:#ee9090}.mood-neutral{background:#4a4a2d;color:#eeee90}.mood-excited{background:#2d4a5a;color:#90d0ee}.mood-anxious{background:#5a3d2d;color:#eeb090}");
    w.push(".entry-text{line-height:1.6;font-size:15px}");
    w.push("</style></head><body><div class='c'><h1>My Diary</h1>");
    w.push("<div class='form'><input type='date' id='date'><select id='mood'><option value='happy'>Happy</option><option value='sad'>Sad</option><option value='neutral' selected>Neutral</option><option value='excited'>Excited</option><option value='anxious'>Anxious</option></select>");
    w.push("<textarea id='entry' placeholder='Write about your day...'></textarea><button onclick='save()'>Save Entry</button></div>");
    w.push("<div id='entries'>");

    // Render entries in reverse order (newest first)
    if count > 0 {
        let mut i = count;
        while i > 0 {
            i -= 1;
            let mut key_buf = [0u8; 20];
            let klen = write_key(&mut key_buf, b"diary_", i);
            let key = unsafe { core::str::from_utf8_unchecked(&key_buf[..klen]) };
            if let Some(data) = kv_read(key) {
                let db = data.as_bytes();
                // Parse date|mood|text
                let mut p1 = 0;
                while p1 < db.len() && db[p1] != b'|' { p1 += 1; }
                let date = &data[..p1];
                let mut p2 = p1 + 1;
                while p2 < db.len() && db[p2] != b'|' { p2 += 1; }
                let mood = if p1 + 1 < p2 { &data[p1 + 1..p2] } else { "neutral" };
                let text = if p2 + 1 < db.len() { &data[p2 + 1..] } else { "" };

                w.push("<div class='entry'><div class='entry-header'><span>");
                w.push(date);
                w.push("</span><span class='mood mood-");
                w.push(mood);
                w.push("'>");
                w.push(mood);
                w.push("</span></div><div class='entry-text'>");
                w.push(text);
                w.push("</div></div>");
            }
        }
    }

    w.push("</div></div><script>const B=location.pathname;");
    w.push("document.getElementById('date').valueAsDate=new Date();");
    w.push("async function save(){const d=document.getElementById('date').value;const m=document.getElementById('mood').value;const e=document.getElementById('entry').value.trim();if(!e)return;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({date:d,mood:m,entry:e})});location.reload();}");
    w.push("</script></body></html>");
    respond(200, w.as_str(), "text/html");
}

fn write_key(buf: &mut [u8], prefix: &[u8], num: u32) -> usize {
    let mut pos = 0;
    let mut i = 0;
    while i < prefix.len() { buf[pos] = prefix[i]; pos += 1; i += 1; }
    if num == 0 { buf[pos] = b'0'; return pos + 1; }
    let mut d = [0u8; 10]; let mut di = 0; let mut n = num;
    while n > 0 { d[di] = b'0' + (n % 10) as u8; n /= 10; di += 1; }
    while di > 0 { di -= 1; buf[pos] = d[di]; pos += 1; }
    pos
}
