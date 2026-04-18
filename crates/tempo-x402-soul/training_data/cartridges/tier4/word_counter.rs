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

fn respond(status: i32, body: &str, ct: &str) { unsafe { response(status, body.as_ptr(), body.len() as i32, ct.as_ptr(), ct.len() as i32); } }
fn host_log(level: i32, msg: &str) { unsafe { log(level, msg.as_ptr(), msg.len() as i32); } }
fn kv_read(key: &str) -> Option<&'static str> {
    unsafe { let r = kv_get(key.as_ptr(), key.len() as i32); if r < 0 { return None; } let p = (r >> 32) as *const u8; let l = (r & 0xFFFFFFFF) as usize; core::str::from_utf8(core::slice::from_raw_parts(p, l)).ok() }
}
fn kv_write(key: &str, val: &str) { unsafe { kv_set(key.as_ptr(), key.len() as i32, val.as_ptr(), val.len() as i32); } }

fn find_json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let kb = key.as_bytes(); let jb = json.as_bytes(); let mut i = 0;
    while i + kb.len() + 3 < jb.len() {
        if jb[i] == b'"' { let s = i + 1;
            if s + kb.len() < jb.len() && &jb[s..s + kb.len()] == kb && jb[s + kb.len()] == b'"' {
                let mut j = s + kb.len() + 1; while j < jb.len() && (jb[j] == b':' || jb[j] == b' ') { j += 1; }
                if j < jb.len() && jb[j] == b'"' { let vs = j + 1; let mut ve = vs; while ve < jb.len() && jb[ve] != b'"' { ve += 1; } return core::str::from_utf8(&jb[vs..ve]).ok(); }
            }
        } i += 1;
    } None
}

static mut BUF: [u8; 65536] = [0u8; 65536];
struct W { pos: usize }
impl W {
    fn new() -> Self { Self { pos: 0 } }
    fn s(&mut self, s: &str) { let b = s.as_bytes(); unsafe { let e = (self.pos + b.len()).min(BUF.len()); BUF[self.pos..e].copy_from_slice(&b[..e - self.pos]); self.pos = e; } }
    fn n(&mut self, mut n: u32) { if n == 0 { self.s("0"); return; } let mut d = [0u8; 10]; let mut i = 0; while n > 0 { d[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; } while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = d[i]; self.pos += 1; } } } }
    fn out(&self) -> &str { unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) } }
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];
#[no_mangle] pub extern "C" fn x402_alloc(size: i32) -> *mut u8 { unsafe { SCRATCH.as_mut_ptr() } }

fn parse_u32(s: &str) -> u32 { let mut n: u32 = 0; for &b in s.as_bytes() { if b >= b'0' && b <= b'9' { n = n * 10 + (b - b'0') as u32; } } n }

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize)) };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(0, "word_counter: handling request");

    // POST — analyze text and save result
    if method == "POST" {
        if let Some(text) = find_json_str(body, "text") {
            let tb = text.as_bytes();
            let chars = tb.len() as u32;
            let mut words: u32 = 0;
            let mut sentences: u32 = 0;
            let mut in_word = false;
            let mut i = 0;
            while i < tb.len() {
                if tb[i] == b' ' || tb[i] == b'\n' || tb[i] == b'\t' { in_word = false; }
                else { if !in_word { words += 1; } in_word = true; }
                if tb[i] == b'.' || tb[i] == b'!' || tb[i] == b'?' { sentences += 1; }
                i += 1;
            }
            let chars_no_space = tb.iter().filter(|&&b| b != b' ' && b != b'\n' && b != b'\t').count() as u32;

            // Store analysis count
            let count = kv_read("wc_count").map(|s| parse_u32(s)).unwrap_or(0);
            let mut w = W::new();
            w.n(chars); w.s("|"); w.n(chars_no_space); w.s("|"); w.n(words); w.s("|"); w.n(sentences);
            // Truncate text for history (first 50 chars)
            w.s("|");
            let preview_len = if tb.len() > 50 { 50 } else { tb.len() };
            let preview = unsafe { core::str::from_utf8_unchecked(&tb[..preview_len]) };
            w.s(preview);

            let mut key = [0u8; 16];
            let klen = write_key(&mut key, b"wc_", count);
            let k = unsafe { core::str::from_utf8_unchecked(&key[..klen]) };
            kv_write(k, w.out());

            let mut cw = W::new();
            cw.n(count + 1);
            kv_write("wc_count", cw.out());

            // Return JSON result
            let mut rw = W::new();
            rw.s(r#"{"chars":"#); rw.n(chars);
            rw.s(r#","chars_no_space":"#); rw.n(chars_no_space);
            rw.s(r#","words":"#); rw.n(words);
            rw.s(r#","sentences":"#); rw.n(sentences);
            rw.s("}");
            respond(200, rw.out(), "application/json");
        } else {
            respond(400, r#"{"error":"missing text"}"#, "application/json");
        }
        return;
    }

    // GET — render UI
    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Word Counter</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#0d1117;color:#c9d1d9;font-family:'Segoe UI',sans-serif;padding:30px 20px;display:flex;justify-content:center}");
    w.s(".c{max-width:700px;width:100%}h1{text-align:center;color:#58a6ff;margin-bottom:24px}");
    w.s("textarea{width:100%;height:200px;padding:16px;background:#161b22;border:1px solid #30363d;color:#c9d1d9;border-radius:8px;font-size:15px;resize:vertical;font-family:inherit}");
    w.s(".stats{display:grid;grid-template-columns:repeat(4,1fr);gap:12px;margin:20px 0}");
    w.s(".stat{background:#161b22;padding:16px;border-radius:8px;text-align:center}.stat .val{font-size:28px;color:#58a6ff;font-weight:bold}.stat .label{font-size:12px;color:#8b949e;margin-top:4px;text-transform:uppercase}");
    w.s("button{padding:12px 24px;background:#238636;color:#fff;border:none;border-radius:6px;cursor:pointer;font-size:15px;width:100%;margin-top:12px}");
    w.s(".history{margin-top:24px}.history h3{color:#8b949e;font-size:13px;text-transform:uppercase;margin-bottom:10px}");
    w.s(".hist{background:#161b22;padding:12px;border-radius:6px;margin-bottom:6px;display:flex;justify-content:space-between;font-size:13px}");
    w.s(".hist .preview{color:#8b949e;flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;margin-right:12px}");
    w.s("</style></head><body><div class='c'><h1>Word Counter</h1>");
    w.s("<textarea id='txt' placeholder='Paste or type your text here...' oninput='liveCount()'></textarea>");
    w.s("<div class='stats'><div class='stat'><div class='val' id='sc'>0</div><div class='label'>Characters</div></div>");
    w.s("<div class='stat'><div class='val' id='snc'>0</div><div class='label'>No Spaces</div></div>");
    w.s("<div class='stat'><div class='val' id='sw'>0</div><div class='label'>Words</div></div>");
    w.s("<div class='stat'><div class='val' id='ss'>0</div><div class='label'>Sentences</div></div></div>");
    w.s("<button onclick='saveAnalysis()'>Save Analysis</button>");

    // History
    let count = kv_read("wc_count").map(|s| parse_u32(s)).unwrap_or(0);
    if count > 0 {
        w.s("<div class='history'><h3>Previous Analyses</h3>");
        let start = if count > 10 { count - 10 } else { 0 };
        let mut i = count;
        while i > start {
            i -= 1;
            let mut key = [0u8; 16];
            let klen = write_key(&mut key, b"wc_", i);
            let k = unsafe { core::str::from_utf8_unchecked(&key[..klen]) };
            if let Some(data) = kv_read(k) {
                // Parse: chars|chars_no_space|words|sentences|preview
                let db = data.as_bytes();
                let mut parts = [0usize; 5];
                let mut pi = 0; let mut last = 0; let mut di = 0;
                while di < db.len() && pi < 4 { if db[di] == b'|' { parts[pi] = di; pi += 1; } di += 1; }
                parts[4] = db.len();
                if pi >= 4 {
                    let preview = if parts[3] + 1 < db.len() { &data[parts[3] + 1..] } else { "" };
                    w.s("<div class='hist'><span class='preview'>");
                    w.s(preview);
                    w.s("...</span><span>");
                    w.s(&data[parts[1] + 1..parts[2]]);
                    w.s(" words</span></div>");
                }
            }
        }
        w.s("</div>");
    }

    w.s("</div><script>");
    w.s("function liveCount(){const t=document.getElementById('txt').value;document.getElementById('sc').textContent=t.length;document.getElementById('snc').textContent=t.replace(/\\s/g,'').length;document.getElementById('sw').textContent=t.trim()?t.trim().split(/\\s+/).length:0;document.getElementById('ss').textContent=(t.match(/[.!?]+/g)||[]).length;}");
    w.s("async function saveAnalysis(){const t=document.getElementById('txt').value;if(!t.trim())return;await fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({text:t})});location.reload();}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}

fn write_key(buf: &mut [u8], prefix: &[u8], num: u32) -> usize {
    let mut pos = 0;
    for &b in prefix { buf[pos] = b; pos += 1; }
    if num == 0 { buf[pos] = b'0'; return pos + 1; }
    let mut d = [0u8; 10]; let mut di = 0; let mut n = num;
    while n > 0 { d[di] = b'0' + (n % 10) as u8; n /= 10; di += 1; }
    while di > 0 { di -= 1; buf[pos] = d[di]; pos += 1; }
    pos
}
