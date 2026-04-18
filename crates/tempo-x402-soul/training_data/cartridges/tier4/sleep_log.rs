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
fn kv_read(key: &str) -> Option<&'static str> { unsafe { let r = kv_get(key.as_ptr(), key.len() as i32); if r < 0 { return None; } let p = (r >> 32) as *const u8; let l = (r & 0xFFFFFFFF) as usize; core::str::from_utf8(core::slice::from_raw_parts(p, l)).ok() } }
fn kv_write(key: &str, val: &str) { unsafe { kv_set(key.as_ptr(), key.len() as i32, val.as_ptr(), val.len() as i32); } }
fn find_json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let kb = key.as_bytes(); let jb = json.as_bytes(); let mut i = 0;
    while i + kb.len() + 3 < jb.len() { if jb[i] == b'"' { let s = i + 1; if s + kb.len() < jb.len() && &jb[s..s + kb.len()] == kb && jb[s + kb.len()] == b'"' { let mut j = s + kb.len() + 1; while j < jb.len() && (jb[j] == b':' || jb[j] == b' ') { j += 1; } if j < jb.len() && jb[j] == b'"' { let vs = j + 1; let mut ve = vs; while ve < jb.len() && jb[ve] != b'"' { ve += 1; } return core::str::from_utf8(&jb[vs..ve]).ok(); } } } i += 1; } None
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

fn write_key(buf: &mut [u8], prefix: &[u8], num: u32) -> usize {
    let mut pos = 0; for &b in prefix { buf[pos] = b; pos += 1; }
    if num == 0 { buf[pos] = b'0'; return pos + 1; }
    let mut d = [0u8; 10]; let mut di = 0; let mut n = num;
    while n > 0 { d[di] = b'0' + (n % 10) as u8; n /= 10; di += 1; }
    while di > 0 { di -= 1; buf[pos] = d[di]; pos += 1; } pos
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize)) };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let body = find_json_str(request, "body").unwrap_or("");
    host_log(0, "sleep_log: handling request");

    if method == "POST" {
        let date = find_json_str(body, "date").unwrap_or("");
        let bedtime = find_json_str(body, "bedtime").unwrap_or("");
        let waketime = find_json_str(body, "waketime").unwrap_or("");
        let quality = find_json_str(body, "quality").unwrap_or("3");
        if !date.is_empty() && !bedtime.is_empty() && !waketime.is_empty() {
            let count = kv_read("sl_count").map(|s| parse_u32(s)).unwrap_or(0);
            let mut w = W::new();
            w.s(date); w.s("|"); w.s(bedtime); w.s("|"); w.s(waketime); w.s("|"); w.s(quality);
            let mut kb = [0u8; 16]; let kl = write_key(&mut kb, b"sl_", count);
            let key = unsafe { core::str::from_utf8_unchecked(&kb[..kl]) };
            kv_write(key, w.out());
            let mut cw = W::new(); cw.n(count + 1); kv_write("sl_count", cw.out());
            respond(200, r#"{"ok":true}"#, "application/json");
        } else { respond(400, r#"{"error":"missing fields"}"#, "application/json"); }
        return;
    }

    let count = kv_read("sl_count").map(|s| parse_u32(s)).unwrap_or(0);
    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Sleep Log</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#0a0a1e;color:#c9d1d9;font-family:'Segoe UI',sans-serif;padding:30px 20px;display:flex;justify-content:center}");
    w.s(".c{max-width:600px;width:100%}h1{text-align:center;color:#a78bfa;margin-bottom:20px}");
    w.s(".form{background:#161b2e;padding:18px;border-radius:12px;margin-bottom:20px;display:grid;grid-template-columns:1fr 1fr;gap:10px}");
    w.s(".form label{font-size:12px;color:#888;text-transform:uppercase}");
    w.s(".form input,.form select{padding:10px;background:#0d1117;border:1px solid #30363d;color:#c9d1d9;border-radius:6px;font-size:14px;width:100%}");
    w.s(".full{grid-column:1/-1}button{padding:12px;background:#7c3aed;color:#fff;border:none;border-radius:6px;cursor:pointer;font-size:15px}");
    w.s(".entry{background:#161b2e;padding:14px;border-radius:10px;margin-bottom:8px;display:flex;justify-content:space-between;align-items:center}");
    w.s(".entry .date{font-weight:600;color:#a78bfa}.entry .times{font-size:13px;color:#888}.entry .quality{font-size:20px}");
    w.s(".avg{text-align:center;background:#1a1040;padding:16px;border-radius:10px;margin-bottom:16px}");
    w.s(".avg .val{font-size:28px;color:#a78bfa;font-weight:bold}.avg .label{font-size:12px;color:#888}");
    w.s("</style></head><body><div class='c'><h1>Sleep Log</h1>");

    // Compute average quality
    if count > 0 {
        let mut total_q: u32 = 0;
        let start = if count > 7 { count - 7 } else { 0 };
        let mut ci = start;
        let entries = count - start;
        while ci < count {
            let mut kb = [0u8; 16]; let kl = write_key(&mut kb, b"sl_", ci);
            let key = unsafe { core::str::from_utf8_unchecked(&kb[..kl]) };
            if let Some(data) = kv_read(key) {
                let db = data.as_bytes();
                let mut pipes = [0usize; 3]; let mut pi = 0; let mut di = 0;
                while di < db.len() && pi < 3 { if db[di] == b'|' { pipes[pi] = di; pi += 1; } di += 1; }
                if pi >= 3 { let q = parse_u32(&data[pipes[2] + 1..]); total_q += q; }
            }
            ci += 1;
        }
        let avg_q = if entries > 0 { total_q / entries } else { 0 };
        w.s("<div class='avg'><div class='val'>"); w.n(avg_q); w.s("/5</div><div class='label'>Avg Quality (last 7)</div></div>");
    }

    w.s("<div class='form'><div><label>Date</label><input type='date' id='date'></div><div><label>Quality</label><select id='quality'><option value='1'>1 - Terrible</option><option value='2'>2 - Poor</option><option value='3' selected>3 - OK</option><option value='4'>4 - Good</option><option value='5'>5 - Excellent</option></select></div>");
    w.s("<div><label>Bedtime</label><input type='time' id='bed' value='23:00'></div><div><label>Wake Time</label><input type='time' id='wake' value='07:00'></div>");
    w.s("<button class='full' onclick='logSleep()'>Log Sleep</button></div>");
    w.s("<div id='entries'>");

    // Show entries newest first
    if count > 0 {
        let mut i = count;
        let show = if count > 14 { count - 14 } else { 0 };
        while i > show {
            i -= 1;
            let mut kb = [0u8; 16]; let kl = write_key(&mut kb, b"sl_", i);
            let key = unsafe { core::str::from_utf8_unchecked(&kb[..kl]) };
            if let Some(data) = kv_read(key) {
                let db = data.as_bytes();
                let mut pipes = [0usize; 3]; let mut pi = 0; let mut di = 0;
                while di < db.len() && pi < 3 { if db[di] == b'|' { pipes[pi] = di; pi += 1; } di += 1; }
                if pi >= 3 {
                    let date = &data[..pipes[0]];
                    let bed = &data[pipes[0] + 1..pipes[1]];
                    let wake = &data[pipes[1] + 1..pipes[2]];
                    let quality = parse_u32(&data[pipes[2] + 1..]);
                    let stars = ["", "*", "**", "***", "****", "*****"];
                    let star_str = if (quality as usize) < stars.len() { stars[quality as usize] } else { "*****" };
                    w.s("<div class='entry'><div><div class='date'>"); w.s(date);
                    w.s("</div><div class='times'>"); w.s(bed); w.s(" - "); w.s(wake);
                    w.s("</div></div><div class='quality'>"); w.s(star_str); w.s("</div></div>");
                }
            }
        }
    }

    w.s("</div></div><script>const B=location.pathname;document.getElementById('date').valueAsDate=new Date();");
    w.s("async function logSleep(){const d=document.getElementById('date').value;const b=document.getElementById('bed').value;const wk=document.getElementById('wake').value;const q=document.getElementById('quality').value;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({date:d,bedtime:b,waketime:wk,quality:q})});location.reload();}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
