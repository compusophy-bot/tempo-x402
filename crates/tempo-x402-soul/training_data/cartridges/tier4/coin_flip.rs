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
    host_log(0, "coin_flip: handling request");

    if method == "POST" {
        if let Some(action) = find_json_str(body, "action") {
            if action == "flip" {
                // Use a simple counter-based pseudo-random
                let counter = kv_read("cf_counter").map(|s| parse_u32(s)).unwrap_or(0);
                let seed = counter.wrapping_mul(1103515245).wrapping_add(12345);
                let is_heads = (seed >> 16) & 1 == 0;
                let result = if is_heads { "heads" } else { "tails" };

                // Update stats
                let heads = kv_read("cf_heads").map(|s| parse_u32(s)).unwrap_or(0);
                let tails = kv_read("cf_tails").map(|s| parse_u32(s)).unwrap_or(0);
                if is_heads {
                    let mut w = W::new(); w.n(heads + 1); kv_write("cf_heads", w.out());
                } else {
                    let mut w = W::new(); w.n(tails + 1); kv_write("cf_tails", w.out());
                }
                let mut w = W::new(); w.n(counter + 1); kv_write("cf_counter", w.out());

                // Store last result
                kv_write("cf_last", result);

                // Add to history (last 20)
                let hist = kv_read("cf_hist").unwrap_or("");
                let mut hw = W::new();
                hw.s(if is_heads { "H" } else { "T" });
                if !hist.is_empty() { hw.s(hist); }
                // Trim to 20
                let hb = hw.out().as_bytes();
                let trim = if hb.len() > 20 { 20 } else { hb.len() };
                let trimmed = unsafe { core::str::from_utf8_unchecked(&hb[..trim]) };
                kv_write("cf_hist", trimmed);

                let mut rw = W::new();
                rw.s(r#"{"result":""#); rw.s(result); rw.s(r#""}"#);
                respond(200, rw.out(), "application/json");
            } else if action == "reset" {
                kv_write("cf_heads", "0");
                kv_write("cf_tails", "0");
                kv_write("cf_hist", "");
                kv_write("cf_last", "");
                respond(200, r#"{"ok":true}"#, "application/json");
            } else { respond(400, r#"{"error":"unknown"}"#, "application/json"); }
        } else { respond(400, r#"{"error":"missing action"}"#, "application/json"); }
        return;
    }

    let heads = kv_read("cf_heads").map(|s| parse_u32(s)).unwrap_or(0);
    let tails = kv_read("cf_tails").map(|s| parse_u32(s)).unwrap_or(0);
    let last = kv_read("cf_last").unwrap_or("");
    let hist = kv_read("cf_hist").unwrap_or("");

    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Coin Flip</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#0a0a1a;color:#e0e0e0;font-family:'Segoe UI',sans-serif;display:flex;justify-content:center;padding:40px 20px}");
    w.s(".c{max-width:400px;width:100%;text-align:center}h1{color:#ffd700;margin-bottom:24px}");
    w.s(".coin{width:180px;height:180px;border-radius:50%;margin:0 auto 24px;display:flex;align-items:center;justify-content:center;font-size:60px;font-weight:bold;transition:transform 0.5s}");
    w.s(".coin.heads{background:linear-gradient(135deg,#ffd700,#daa520);color:#4a3800;border:4px solid #b8860b}");
    w.s(".coin.tails{background:linear-gradient(135deg,#c0c0c0,#808080);color:#2a2a2a;border:4px solid #696969}");
    w.s(".coin.none{background:#16213e;color:#555;border:4px solid #333}");
    w.s("button{padding:16px 40px;background:#ffd700;color:#000;border:none;border-radius:30px;cursor:pointer;font-size:18px;font-weight:bold;margin:8px}button:hover{background:#ffed4a}");
    w.s("button.reset{background:#333;color:#aaa;font-size:14px;padding:10px 20px}");
    w.s(".stats{display:flex;gap:20px;justify-content:center;margin:24px 0}");
    w.s(".stat{background:#16213e;padding:16px 24px;border-radius:10px}.stat .v{font-size:28px;font-weight:bold}.stat .l{font-size:12px;color:#888;text-transform:uppercase}");
    w.s(".stat.h .v{color:#ffd700}.stat.t .v{color:#c0c0c0}");
    w.s(".history{margin-top:20px}.history h3{font-size:13px;color:#888;margin-bottom:8px}");
    w.s(".hist-dots{display:flex;gap:4px;justify-content:center;flex-wrap:wrap}");
    w.s(".dot{width:28px;height:28px;border-radius:50%;display:flex;align-items:center;justify-content:center;font-size:12px;font-weight:bold}");
    w.s(".dot.H{background:#3a3000;color:#ffd700;border:1px solid #ffd700}.dot.T{background:#2a2a2a;color:#c0c0c0;border:1px solid #666}");
    w.s("</style></head><body><div class='c'><h1>Coin Flip</h1>");
    w.s("<div class='coin ");
    if last == "heads" { w.s("heads'>H"); } else if last == "tails" { w.s("tails'>T"); } else { w.s("none'>?"); }
    w.s("</div>");
    w.s("<div><button onclick='flip()'>FLIP!</button><button class='reset' onclick='reset()'>Reset</button></div>");
    w.s("<div class='stats'><div class='stat h'><div class='v'>"); w.n(heads);
    w.s("</div><div class='l'>Heads</div></div><div class='stat t'><div class='v'>"); w.n(tails);
    w.s("</div><div class='l'>Tails</div></div></div>");

    if !hist.is_empty() {
        w.s("<div class='history'><h3>Recent Flips</h3><div class='hist-dots'>");
        let hb = hist.as_bytes();
        let mut i = 0;
        while i < hb.len() {
            w.s("<div class='dot ");
            if hb[i] == b'H' { w.s("H'>H"); } else { w.s("T'>T"); }
            w.s("</div>");
            i += 1;
        }
        w.s("</div></div>");
    }

    w.s("</div><script>const B=location.pathname;");
    w.s("async function flip(){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'flip'})});location.reload();}");
    w.s("async function reset(){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'reset'})});location.reload();}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
