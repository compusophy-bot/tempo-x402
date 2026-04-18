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
    host_log(0, "water_tracker: handling request");

    if method == "POST" {
        if let Some(action) = find_json_str(body, "action") {
            if action == "drink" {
                let amount = find_json_str(body, "amount").map(|s| parse_u32(s)).unwrap_or(250);
                let current = kv_read("water_today").map(|s| parse_u32(s)).unwrap_or(0);
                let new_total = current + amount;
                let mut w = W::new(); w.n(new_total); kv_write("water_today", w.out());
                // Update streak
                let goal: u32 = 2000;
                if new_total >= goal {
                    let streak = kv_read("water_streak").map(|s| parse_u32(s)).unwrap_or(0);
                    let mut sw = W::new(); sw.n(streak + 1); kv_write("water_streak", sw.out());
                }
                // Add to log
                let log_data = kv_read("water_log").unwrap_or("");
                let mut lw = W::new();
                if !log_data.is_empty() { lw.s(log_data); lw.s(","); }
                lw.n(amount);
                kv_write("water_log", lw.out());
                respond(200, r#"{"ok":true}"#, "application/json");
            } else if action == "reset" {
                kv_write("water_today", "0");
                kv_write("water_log", "");
                respond(200, r#"{"ok":true}"#, "application/json");
            } else if action == "set_goal" {
                if let Some(g) = find_json_str(body, "goal") {
                    kv_write("water_goal", g);
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"missing goal"}"#, "application/json"); }
            } else { respond(400, r#"{"error":"unknown"}"#, "application/json"); }
        } else { respond(400, r#"{"error":"missing action"}"#, "application/json"); }
        return;
    }

    let current = kv_read("water_today").map(|s| parse_u32(s)).unwrap_or(0);
    let goal = kv_read("water_goal").map(|s| parse_u32(s)).unwrap_or(2000);
    let streak = kv_read("water_streak").map(|s| parse_u32(s)).unwrap_or(0);
    let pct = if goal > 0 { (current * 100) / goal } else { 0 };
    let pct_clamped = if pct > 100 { 100 } else { pct };

    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Water Tracker</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#0a1628;color:#e0e0e0;font-family:'Segoe UI',sans-serif;display:flex;justify-content:center;padding:40px 20px}");
    w.s(".c{max-width:400px;width:100%;text-align:center}h1{color:#00b4d8;margin-bottom:24px}");
    w.s(".glass{width:200px;height:300px;margin:0 auto 24px;border:4px solid #0077b6;border-top:none;border-radius:0 0 20px 20px;position:relative;overflow:hidden;background:#051923}");
    w.s(".water{position:absolute;bottom:0;width:100%;background:linear-gradient(180deg,#48cae4,#0077b6);transition:height 0.5s ease}");
    w.s(".pct{position:absolute;top:50%;left:50%;transform:translate(-50%,-50%);font-size:36px;font-weight:bold;color:#fff;text-shadow:0 2px 8px rgba(0,0,0,0.5);z-index:1}");
    w.s(".info{margin-bottom:20px;font-size:16px}.info span{color:#48cae4;font-weight:bold}");
    w.s(".btns{display:flex;gap:8px;justify-content:center;flex-wrap:wrap;margin-bottom:16px}");
    w.s("button{padding:12px 20px;border:2px solid #0077b6;background:#051923;color:#48cae4;border-radius:20px;cursor:pointer;font-size:15px;font-weight:600}");
    w.s("button:hover{background:#0077b6;color:#fff}");
    w.s(".streak{background:#0a2a3e;padding:12px;border-radius:10px;margin-bottom:16px}.streak .val{font-size:28px;color:#48cae4;font-weight:bold}.streak .label{font-size:12px;color:#888}");
    w.s(".settings{margin-top:16px;font-size:13px;color:#888}");
    w.s(".settings input{width:80px;padding:6px;background:#051923;border:1px solid #333;color:#e0e0e0;border-radius:4px;text-align:center}");
    w.s("button.reset{background:transparent;border-color:#666;color:#666;font-size:13px;padding:8px 16px}");
    w.s("</style></head><body><div class='c'><h1>Water Tracker</h1>");
    w.s("<div class='glass'><div class='water' style='height:"); w.n(pct_clamped); w.s("%'></div>");
    w.s("<div class='pct'>"); w.n(pct); w.s("%</div></div>");
    w.s("<div class='info'>"); w.n(current); w.s(" / <span>"); w.n(goal); w.s("</span> mL</div>");
    w.s("<div class='btns'><button onclick='drink(150)'>Small (150mL)</button><button onclick='drink(250)'>Medium (250mL)</button><button onclick='drink(500)'>Large (500mL)</button></div>");
    w.s("<div class='streak'><div class='val'>"); w.n(streak); w.s("</div><div class='label'>Day Streak</div></div>");
    w.s("<button class='reset' onclick='resetDay()'>Reset Day</button>");
    w.s("<div class='settings'>Daily goal: <input type='number' id='goal' value='"); w.n(goal); w.s("' min='500' step='100'> mL <button onclick='setGoal()' style='font-size:12px;padding:4px 10px'>Set</button></div>");
    w.s("</div><script>const B=location.pathname;");
    w.s("async function drink(ml){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'drink',amount:String(ml)})});location.reload();}");
    w.s("async function resetDay(){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'reset'})});location.reload();}");
    w.s("async function setGoal(){const g=document.getElementById('goal').value;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'set_goal',goal:g})});location.reload();}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
