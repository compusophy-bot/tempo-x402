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
    host_log(0, "stopwatch: handling request");

    // POST — save a lap time
    if method == "POST" {
        if let Some(action) = find_json_str(body, "action") {
            if action == "lap" {
                if let Some(time_s) = find_json_str(body, "time") {
                    let existing = kv_read("sw_laps").unwrap_or("");
                    let mut w = W::new();
                    if !existing.is_empty() { w.s(existing); w.s(","); }
                    w.s(time_s);
                    kv_write("sw_laps", w.out());
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"missing time"}"#, "application/json"); }
            } else if action == "clear" {
                kv_write("sw_laps", "");
                respond(200, r#"{"ok":true}"#, "application/json");
            } else if action == "save_best" {
                if let Some(time_s) = find_json_str(body, "time") {
                    let label = find_json_str(body, "label").unwrap_or("Unnamed");
                    let existing = kv_read("sw_bests").unwrap_or("");
                    let mut w = W::new();
                    if !existing.is_empty() { w.s(existing); w.s("\n"); }
                    w.s(label); w.s("|"); w.s(time_s);
                    kv_write("sw_bests", w.out());
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"missing time"}"#, "application/json"); }
            } else { respond(400, r#"{"error":"unknown"}"#, "application/json"); }
        } else { respond(400, r#"{"error":"missing action"}"#, "application/json"); }
        return;
    }

    // GET — stopwatch UI (timing done client-side)
    let laps = kv_read("sw_laps").unwrap_or("");
    let bests = kv_read("sw_bests").unwrap_or("");
    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Stopwatch</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#000;color:#e0e0e0;font-family:'Courier New',monospace;display:flex;justify-content:center;padding:40px 20px}");
    w.s(".c{max-width:450px;width:100%;text-align:center}h1{color:#00ff88;margin-bottom:30px;font-size:1.5em}");
    w.s(".display{font-size:64px;color:#00ff88;margin-bottom:24px;text-shadow:0 0 30px rgba(0,255,136,0.4);font-weight:bold;letter-spacing:4px}");
    w.s(".btns{display:flex;gap:12px;justify-content:center;margin-bottom:24px}");
    w.s("button{padding:14px 28px;border:2px solid;border-radius:30px;cursor:pointer;font-size:16px;font-family:inherit;font-weight:bold;background:transparent;transition:all 0.2s}");
    w.s(".start{border-color:#00ff88;color:#00ff88}.start:hover{background:#00ff8820}");
    w.s(".stop{border-color:#ff4444;color:#ff4444}.stop:hover{background:#ff444420}");
    w.s(".lap{border-color:#ffaa00;color:#ffaa00}.lap:hover{background:#ffaa0020}");
    w.s(".reset{border-color:#666;color:#666}.reset:hover{background:#66666620}");
    w.s(".laps{max-height:200px;overflow-y:auto;margin-bottom:20px}.lap-item{display:flex;justify-content:space-between;padding:8px 16px;border-bottom:1px solid #111;font-size:14px}");
    w.s(".lap-item .num{color:#666}.lap-item .time{color:#00ff88}");
    w.s(".best{background:#0a0a0a;padding:16px;border-radius:8px;margin-top:16px}.best h3{color:#ffaa00;font-size:13px;margin-bottom:8px}");
    w.s(".best-item{display:flex;justify-content:space-between;padding:4px 0;font-size:13px;color:#888}");
    w.s("</style></head><body><div class='c'><h1>STOPWATCH</h1>");
    w.s("<div class='display' id='time'>00:00.000</div>");
    w.s("<div class='btns'><button class='start' id='startBtn' onclick='toggle()'>START</button><button class='lap' onclick='lap()'>LAP</button><button class='reset' onclick='reset()'>RESET</button></div>");
    w.s("<div class='laps' id='laps'></div>");

    // Best times
    if !bests.is_empty() {
        w.s("<div class='best'><h3>Saved Best Times</h3>");
        let bb = bests.as_bytes();
        let mut p = 0;
        while p < bb.len() {
            let ls = p;
            while p < bb.len() && bb[p] != b'\n' { p += 1; }
            let line = unsafe { core::str::from_utf8_unchecked(&bb[ls..p]) };
            if p < bb.len() { p += 1; }
            let lb = line.as_bytes();
            let mut sep = 0;
            while sep < lb.len() && lb[sep] != b'|' { sep += 1; }
            let label = &line[..sep];
            let time = if sep + 1 < lb.len() { &line[sep + 1..] } else { "0" };
            w.s("<div class='best-item'><span>"); w.s(label);
            w.s("</span><span>"); w.s(time); w.s("ms</span></div>");
        }
        w.s("</div>");
    }

    w.s("</div><script>");
    w.s("let running=false,start=0,elapsed=0,lapCount=0,timer;");
    w.s("function fmt(ms){const m=Math.floor(ms/60000);const s=Math.floor((ms%60000)/1000);const ml=ms%1000;return String(m).padStart(2,'0')+':'+String(s).padStart(2,'0')+'.'+String(ml).padStart(3,'0');}");
    w.s("function update(){if(!running)return;elapsed=Date.now()-start;document.getElementById('time').textContent=fmt(elapsed);timer=requestAnimationFrame(update);}");
    w.s("function toggle(){if(running){running=false;document.getElementById('startBtn').textContent='START';document.getElementById('startBtn').className='start';cancelAnimationFrame(timer);}else{running=true;start=Date.now()-elapsed;document.getElementById('startBtn').textContent='STOP';document.getElementById('startBtn').className='stop';update();}}");
    w.s("async function lap(){if(!running&&elapsed===0)return;lapCount++;const t=fmt(elapsed);const d=document.createElement('div');d.className='lap-item';d.innerHTML='<span class=\"num\">Lap '+lapCount+'</span><span class=\"time\">'+t+'</span>';document.getElementById('laps').prepend(d);await fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'lap',time:String(elapsed)})});}");
    w.s("async function reset(){running=false;elapsed=0;lapCount=0;cancelAnimationFrame(timer);document.getElementById('time').textContent='00:00.000';document.getElementById('startBtn').textContent='START';document.getElementById('startBtn').className='start';document.getElementById('laps').innerHTML='';await fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'clear'})});}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
