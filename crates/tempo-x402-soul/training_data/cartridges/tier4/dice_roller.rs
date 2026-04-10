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
    host_log(0, "dice_roller: handling request");

    if method == "POST" {
        if let Some(action) = find_json_str(body, "action") {
            if action == "roll" {
                let num_dice = find_json_str(body, "count").map(|s| parse_u32(s)).unwrap_or(1);
                let sides = find_json_str(body, "sides").map(|s| parse_u32(s)).unwrap_or(6);
                let count = if num_dice == 0 || num_dice > 10 { 1 } else { num_dice };
                let sides_c = if sides == 0 || sides > 100 { 6 } else { sides };

                // Use LCG from counter
                let counter = kv_read("dice_counter").map(|s| parse_u32(s)).unwrap_or(0);
                let mut seed = counter.wrapping_mul(1103515245).wrapping_add(12345);
                let mut results = [0u32; 10];
                let mut total: u32 = 0;
                let mut i: u32 = 0;
                while i < count {
                    seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                    let val = ((seed >> 16) % sides_c) + 1;
                    results[i as usize] = val;
                    total += val;
                    i += 1;
                }

                // Store result
                let mut w = W::new();
                i = 0;
                while i < count {
                    if i > 0 { w.s(","); }
                    w.n(results[i as usize]);
                    i += 1;
                }
                w.s("|"); w.n(total); w.s("|"); w.n(count); w.s("d"); w.n(sides_c);
                kv_write("dice_last", w.out());

                // Update counter
                let mut cw = W::new(); cw.n(counter + 1); kv_write("dice_counter", cw.out());

                // Update history (last 20)
                let hist = kv_read("dice_hist").unwrap_or("");
                let mut hw = W::new();
                hw.n(count); hw.s("d"); hw.n(sides_c); hw.s("="); hw.n(total);
                if !hist.is_empty() { hw.s("\n"); hw.s(hist); }
                let hb = hw.out().as_bytes();
                let mut lines = 0; let mut trunc = hb.len(); let mut p = 0;
                while p < hb.len() { if hb[p] == b'\n' { lines += 1; if lines >= 20 { trunc = p; break; } } p += 1; }
                let trimmed = unsafe { core::str::from_utf8_unchecked(&hb[..trunc]) };
                kv_write("dice_hist", trimmed);

                // Update total rolls stat
                let total_rolls = kv_read("dice_total_rolls").map(|s| parse_u32(s)).unwrap_or(0);
                let mut tw = W::new(); tw.n(total_rolls + 1); kv_write("dice_total_rolls", tw.out());

                respond(200, r#"{"ok":true}"#, "application/json");
            } else { respond(400, r#"{"error":"unknown"}"#, "application/json"); }
        } else { respond(400, r#"{"error":"missing action"}"#, "application/json"); }
        return;
    }

    let last = kv_read("dice_last").unwrap_or("");
    let hist = kv_read("dice_hist").unwrap_or("");
    let total_rolls = kv_read("dice_total_rolls").map(|s| parse_u32(s)).unwrap_or(0);

    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Dice Roller</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#1a0a2e;color:#e0e0e0;font-family:'Segoe UI',sans-serif;display:flex;justify-content:center;padding:40px 20px}");
    w.s(".c{max-width:500px;width:100%;text-align:center}h1{color:#ff6b6b;margin-bottom:24px}");
    w.s(".dice-area{min-height:120px;display:flex;gap:12px;justify-content:center;align-items:center;flex-wrap:wrap;margin-bottom:20px}");
    w.s(".die{width:70px;height:70px;background:#2d1b69;border:3px solid #7c4dff;border-radius:12px;display:flex;align-items:center;justify-content:center;font-size:32px;font-weight:bold;color:#ff6b6b}");
    w.s(".total-val{font-size:48px;color:#ffd700;font-weight:bold;margin-bottom:20px}");
    w.s(".config{background:#16213e;padding:16px;border-radius:10px;margin-bottom:20px;display:flex;gap:12px;justify-content:center;align-items:center;flex-wrap:wrap}");
    w.s("label{font-size:14px;color:#888}input[type=number]{width:60px;padding:8px;background:#0a0a1a;border:1px solid #333;color:#e0e0e0;border-radius:6px;text-align:center;font-size:16px}");
    w.s("button{padding:14px 36px;background:#ff6b6b;color:#fff;border:none;border-radius:10px;cursor:pointer;font-size:18px;font-weight:bold}button:hover{background:#ee5a5a}");
    w.s(".presets{display:flex;gap:8px;margin:16px 0;flex-wrap:wrap;justify-content:center}");
    w.s(".preset{padding:8px 16px;background:#2d1b69;border:1px solid #7c4dff;color:#7c4dff;border-radius:20px;cursor:pointer;font-size:13px;font-weight:bold}");
    w.s(".preset:hover{background:#7c4dff;color:#fff}");
    w.s(".stat{color:#888;font-size:13px;margin-bottom:16px}");
    w.s(".history{background:#16213e;padding:12px;border-radius:8px;max-height:200px;overflow-y:auto;text-align:left}");
    w.s(".history h3{color:#888;font-size:12px;margin-bottom:6px;text-transform:uppercase}");
    w.s(".hist-item{padding:4px 0;font-size:13px;color:#666;border-bottom:1px solid #111}");
    w.s("</style></head><body><div class='c'><h1>Dice Roller</h1>");
    w.s("<div class='dice-area' id='dice'>");

    // Show last roll
    if !last.is_empty() {
        let lb = last.as_bytes();
        let mut pipe1 = 0; while pipe1 < lb.len() && lb[pipe1] != b'|' { pipe1 += 1; }
        let dice_str = &last[..pipe1];
        let db = dice_str.as_bytes();
        let mut p = 0;
        while p <= db.len() {
            let s = p;
            while p < db.len() && db[p] != b',' { p += 1; }
            let val = unsafe { core::str::from_utf8_unchecked(&db[s..p]) };
            p += 1;
            if !val.is_empty() {
                w.s("<div class='die'>"); w.s(val); w.s("</div>");
            }
        }
    } else {
        w.s("<div class='die'>?</div>");
    }

    w.s("</div>");
    // Show total
    if !last.is_empty() {
        let lb = last.as_bytes();
        let mut pipe1 = 0; while pipe1 < lb.len() && lb[pipe1] != b'|' { pipe1 += 1; }
        let mut pipe2 = pipe1 + 1; while pipe2 < lb.len() && lb[pipe2] != b'|' { pipe2 += 1; }
        let total_s = if pipe1 + 1 < pipe2 { &last[pipe1 + 1..pipe2] } else { "" };
        let config_s = if pipe2 + 1 < lb.len() { &last[pipe2 + 1..] } else { "" };
        w.s("<div class='total-val'>"); w.s(total_s); w.s("</div>");
    }

    w.s("<div class='config'><label>Count</label><input type='number' id='count' value='2' min='1' max='10'><label>Sides</label><input type='number' id='sides' value='6' min='2' max='100'></div>");
    w.s("<button onclick='roll()'>ROLL!</button>");
    w.s("<div class='presets'><div class='preset' onclick='preset(1,6)'>1d6</div><div class='preset' onclick='preset(2,6)'>2d6</div><div class='preset' onclick='preset(1,20)'>1d20</div><div class='preset' onclick='preset(3,6)'>3d6</div><div class='preset' onclick='preset(1,100)'>1d100</div><div class='preset' onclick='preset(4,6)'>4d6</div></div>");
    w.s("<div class='stat'>Total rolls: "); w.n(total_rolls); w.s("</div>");

    if !hist.is_empty() {
        w.s("<div class='history'><h3>History</h3>");
        let hb = hist.as_bytes();
        let mut p = 0;
        while p < hb.len() {
            let ls = p;
            while p < hb.len() && hb[p] != b'\n' { p += 1; }
            let line = unsafe { core::str::from_utf8_unchecked(&hb[ls..p]) };
            if p < hb.len() { p += 1; }
            w.s("<div class='hist-item'>"); w.s(line); w.s("</div>");
        }
        w.s("</div>");
    }

    w.s("</div><script>const B=location.pathname;");
    w.s("function preset(c,s){document.getElementById('count').value=c;document.getElementById('sides').value=s;roll();}");
    w.s("async function roll(){const c=document.getElementById('count').value;const s=document.getElementById('sides').value;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'roll',count:c,sides:s})});location.reload();}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
