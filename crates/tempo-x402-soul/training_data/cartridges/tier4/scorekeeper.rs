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
    fn n(&mut self, mut n: i32) {
        if n < 0 { self.s("-"); n = -n; }
        if n == 0 { self.s("0"); return; }
        let mut d = [0u8; 10]; let mut i = 0;
        while n > 0 { d[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
        while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = d[i]; self.pos += 1; } } }
    }
    fn out(&self) -> &str { unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) } }
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];
#[no_mangle] pub extern "C" fn x402_alloc(size: i32) -> *mut u8 { unsafe { SCRATCH.as_mut_ptr() } }

fn parse_i32(s: &str) -> i32 {
    let b = s.as_bytes(); let mut n: i32 = 0; let mut neg = false; let mut i = 0;
    if i < b.len() && b[i] == b'-' { neg = true; i += 1; }
    while i < b.len() { if b[i] >= b'0' && b[i] <= b'9' { n = n * 10 + (b[i] - b'0') as i32; } i += 1; }
    if neg { -n } else { n }
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize)) };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(0, "scorekeeper: handling request");

    if method == "POST" {
        if let Some(action) = find_json_str(body, "action") {
            if action == "add_player" {
                if let Some(name) = find_json_str(body, "name") {
                    let players = kv_read("sk_players").unwrap_or("");
                    let mut w = W::new();
                    if !players.is_empty() { w.s(players); w.s(","); }
                    w.s(name);
                    kv_write("sk_players", w.out());
                    // Init score to 0
                    let mut key = [0u8; 40]; let mut kp = 0;
                    for &b in b"sk_score_" { key[kp] = b; kp += 1; }
                    for &b in name.as_bytes() { if kp < 40 { key[kp] = b; kp += 1; } }
                    let k = unsafe { core::str::from_utf8_unchecked(&key[..kp]) };
                    kv_write(k, "0");
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"missing name"}"#, "application/json"); }
            } else if action == "score" {
                if let Some(name) = find_json_str(body, "name") {
                    let delta = find_json_str(body, "delta").map(|s| parse_i32(s)).unwrap_or(1);
                    let mut key = [0u8; 40]; let mut kp = 0;
                    for &b in b"sk_score_" { key[kp] = b; kp += 1; }
                    for &b in name.as_bytes() { if kp < 40 { key[kp] = b; kp += 1; } }
                    let k = unsafe { core::str::from_utf8_unchecked(&key[..kp]) };
                    let cur = kv_read(k).map(|s| parse_i32(s)).unwrap_or(0);
                    let new_score = cur + delta;
                    let mut w = W::new();
                    w.n(new_score);
                    kv_write(k, w.out());
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"missing name"}"#, "application/json"); }
            } else if action == "reset" {
                let players = kv_read("sk_players").unwrap_or("");
                let pb = players.as_bytes();
                let mut p = 0;
                while p <= pb.len() {
                    let s = p;
                    while p < pb.len() && pb[p] != b',' { p += 1; }
                    let name = unsafe { core::str::from_utf8_unchecked(&pb[s..p]) };
                    if !name.is_empty() {
                        let mut key = [0u8; 40]; let mut kp = 0;
                        for &b in b"sk_score_" { key[kp] = b; kp += 1; }
                        for &b in name.as_bytes() { if kp < 40 { key[kp] = b; kp += 1; } }
                        let k = unsafe { core::str::from_utf8_unchecked(&key[..kp]) };
                        kv_write(k, "0");
                    }
                    p += 1;
                }
                respond(200, r#"{"ok":true}"#, "application/json");
            } else {
                respond(400, r#"{"error":"unknown action"}"#, "application/json");
            }
        } else { respond(400, r#"{"error":"missing action"}"#, "application/json"); }
        return;
    }

    // GET — render scoreboard
    let players = kv_read("sk_players").unwrap_or("");
    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Scorekeeper</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#0a0a1a;color:#e0e0e0;font-family:'Segoe UI',sans-serif;padding:30px 20px;display:flex;justify-content:center}");
    w.s(".c{max-width:600px;width:100%}h1{text-align:center;color:#7c4dff;margin-bottom:24px}");
    w.s(".add{display:flex;gap:8px;margin-bottom:24px}input{padding:10px;background:#111;border:1px solid #333;color:#e0e0e0;border-radius:6px;font-size:14px;flex:1}");
    w.s("button{padding:10px 18px;background:#7c4dff;color:#fff;border:none;border-radius:6px;cursor:pointer;font-size:14px}button:hover{background:#651fff}");
    w.s(".player{display:flex;align-items:center;gap:12px;background:#111;padding:16px;border-radius:10px;margin-bottom:8px}");
    w.s(".player .name{flex:1;font-size:18px;font-weight:bold}.player .score{font-size:32px;color:#7c4dff;min-width:60px;text-align:center}");
    w.s(".btns{display:flex;gap:4px;flex-direction:column}.btns button{padding:6px 14px;font-size:16px}");
    w.s(".minus{background:#e94560}.reset-btn{margin-top:16px;background:#333;width:100%}");
    w.s("</style></head><body><div class='c'><h1>Scorekeeper</h1>");
    w.s("<div class='add'><input id='name' placeholder='Player name'><button onclick='addPlayer()'>Add Player</button></div>");
    w.s("<div id='board'>");

    if !players.is_empty() {
        let pb = players.as_bytes();
        let mut p = 0;
        while p <= pb.len() {
            let s = p;
            while p < pb.len() && pb[p] != b',' { p += 1; }
            let name = unsafe { core::str::from_utf8_unchecked(&pb[s..p]) };
            p += 1;
            if name.is_empty() { continue; }
            let mut key = [0u8; 40]; let mut kp = 0;
            for &b in b"sk_score_" { key[kp] = b; kp += 1; }
            for &b in name.as_bytes() { if kp < 40 { key[kp] = b; kp += 1; } }
            let k = unsafe { core::str::from_utf8_unchecked(&key[..kp]) };
            let score = kv_read(k).map(|s| parse_i32(s)).unwrap_or(0);
            w.s("<div class='player'><span class='name'>");
            w.s(name);
            w.s("</span><div class='btns'><button onclick=\"sc('");
            w.s(name);
            w.s("',1)\">+</button><button class='minus' onclick=\"sc('");
            w.s(name);
            w.s("',-1)\">-</button></div><span class='score'>");
            w.n(score);
            w.s("</span></div>");
        }
    }

    w.s("</div><button class='reset-btn' onclick='resetAll()'>Reset All Scores</button></div>");
    w.s("<script>const B=location.pathname;");
    w.s("async function addPlayer(){const n=document.getElementById('name').value.trim();if(!n)return;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add_player',name:n})});location.reload();}");
    w.s("async function sc(name,d){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'score',name:name,delta:String(d)})});location.reload();}");
    w.s("async function resetAll(){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'reset'})});location.reload();}");
    w.s("document.getElementById('name').addEventListener('keydown',e=>{if(e.key==='Enter')addPlayer();});");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
