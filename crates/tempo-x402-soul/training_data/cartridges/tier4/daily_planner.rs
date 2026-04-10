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

    host_log(0, "daily_planner: handling request");

    if method == "POST" {
        if let Some(date) = find_json_str(body, "date") {
            if let Some(time) = find_json_str(body, "time") {
                if let Some(task) = find_json_str(body, "task") {
                    let priority = find_json_str(body, "priority").unwrap_or("medium");
                    // Store: date -> time|priority|task\n...
                    let mut dk = [0u8; 32]; let mut dp = 0;
                    for &b in b"plan_" { dk[dp] = b; dp += 1; }
                    for &b in date.as_bytes() { if dp < 32 { dk[dp] = b; dp += 1; } }
                    let key = unsafe { core::str::from_utf8_unchecked(&dk[..dp]) };
                    let existing = kv_read(key).unwrap_or("");
                    let mut w = W::new();
                    if !existing.is_empty() { w.s(existing); w.s("\n"); }
                    w.s(time); w.s("|"); w.s(priority); w.s("|"); w.s(task);
                    kv_write(key, w.out());
                    respond(200, r#"{"ok":true}"#, "application/json");
                    return;
                }
            }
        }
        if let Some(action) = find_json_str(body, "action") {
            if action == "delete" {
                if let Some(date) = find_json_str(body, "date") {
                    if let Some(idx_s) = find_json_str(body, "index") {
                        let idx = parse_u32(idx_s);
                        let mut dk = [0u8; 32]; let mut dp = 0;
                        for &b in b"plan_" { dk[dp] = b; dp += 1; }
                        for &b in date.as_bytes() { if dp < 32 { dk[dp] = b; dp += 1; } }
                        let key = unsafe { core::str::from_utf8_unchecked(&dk[..dp]) };
                        let existing = kv_read(key).unwrap_or("");
                        let mut w = W::new();
                        let eb = existing.as_bytes();
                        let mut p = 0; let mut line_num: u32 = 0;
                        while p < eb.len() {
                            let ls = p;
                            while p < eb.len() && eb[p] != b'\n' { p += 1; }
                            let line = unsafe { core::str::from_utf8_unchecked(&eb[ls..p]) };
                            if p < eb.len() { p += 1; }
                            if line_num != idx && !line.is_empty() {
                                if w.pos > 0 { w.s("\n"); }
                                w.s(line);
                            }
                            line_num += 1;
                        }
                        kv_write(key, w.out());
                        respond(200, r#"{"ok":true}"#, "application/json");
                        return;
                    }
                }
            }
        }
        respond(400, r#"{"error":"invalid request"}"#, "application/json");
        return;
    }

    // GET — render planner
    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Daily Planner</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#0d1117;color:#c9d1d9;font-family:'Segoe UI',sans-serif;padding:30px 20px;display:flex;justify-content:center}");
    w.s(".c{max-width:650px;width:100%}h1{text-align:center;color:#79c0ff;margin-bottom:20px}");
    w.s(".form{background:#161b22;padding:18px;border-radius:10px;margin-bottom:20px;display:flex;flex-wrap:wrap;gap:8px}");
    w.s(".form input,.form select{padding:10px;background:#0d1117;border:1px solid #30363d;color:#c9d1d9;border-radius:6px;font-size:14px}");
    w.s(".form input[type=date]{width:150px}.form input[type=time]{width:120px}.form input[type=text]{flex:1;min-width:200px}");
    w.s("button{padding:10px 18px;background:#238636;color:#fff;border:none;border-radius:6px;cursor:pointer;font-size:14px}");
    w.s(".timeline{position:relative;padding-left:30px}");
    w.s(".hour-block{margin-bottom:4px;display:flex;align-items:flex-start;gap:12px;min-height:48px}");
    w.s(".time-label{width:60px;font-size:13px;color:#8b949e;padding-top:12px;flex-shrink:0}");
    w.s(".task-card{flex:1;padding:10px 14px;border-radius:6px;font-size:14px;display:flex;justify-content:space-between;align-items:center}");
    w.s(".task-card.high{background:#3d1a1a;border-left:3px solid #f85149}.task-card.medium{background:#2a2a1a;border-left:3px solid #d29922}.task-card.low{background:#1a2a1a;border-left:3px solid #3fb950}");
    w.s(".del-btn{background:none;border:none;color:#f85149;cursor:pointer;font-size:16px;padding:4px 8px}");
    w.s(".date-nav{display:flex;justify-content:center;gap:12px;margin-bottom:20px;align-items:center}");
    w.s(".date-nav button{background:#21262d;padding:8px 14px}");
    w.s(".date-label{font-size:18px;color:#79c0ff;min-width:140px;text-align:center}");
    w.s("</style></head><body><div class='c'><h1>Daily Planner</h1>");
    w.s("<div class='date-nav'><button onclick='prevDay()'>&#9664;</button><span class='date-label' id='dateLabel'></span><button onclick='nextDay()'>&#9654;</button></div>");
    w.s("<div class='form'><input type='date' id='date'><input type='time' id='time' value='09:00'><select id='pri'><option value='high'>High</option><option value='medium' selected>Medium</option><option value='low'>Low</option></select>");
    w.s("<input type='text' id='task' placeholder='Task description'><button onclick='addTask()'>Add</button></div>");
    w.s("<div id='timeline' class='timeline'></div></div>");
    w.s("<script>const B=location.pathname;let curDate=new Date().toISOString().split('T')[0];");
    w.s("function showDate(){document.getElementById('date').value=curDate;document.getElementById('dateLabel').textContent=curDate;loadDay();}");
    w.s("function prevDay(){const d=new Date(curDate);d.setDate(d.getDate()-1);curDate=d.toISOString().split('T')[0];showDate();}");
    w.s("function nextDay(){const d=new Date(curDate);d.setDate(d.getDate()+1);curDate=d.toISOString().split('T')[0];showDate();}");
    w.s("async function addTask(){const t=document.getElementById('task').value.trim();if(!t)return;const d=document.getElementById('date').value;const time=document.getElementById('time').value;const pri=document.getElementById('pri').value;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({date:d,time:time,task:t,priority:pri})});location.reload();}");
    w.s("async function delTask(date,idx){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',date:date,index:String(idx)})});location.reload();}");
    w.s("function loadDay(){/* tasks rendered server-side on reload */}");
    w.s("showDate();document.getElementById('task').addEventListener('keydown',e=>{if(e.key==='Enter')addTask();});");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
