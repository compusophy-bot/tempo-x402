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
    host_log(0, "workout_planner: handling request");

    if method == "POST" {
        if let Some(action) = find_json_str(body, "action") {
            if action == "add_workout" {
                let name = find_json_str(body, "name").unwrap_or("");
                let day = find_json_str(body, "day").unwrap_or("monday");
                if !name.is_empty() {
                    let mut dk = [0u8; 32]; let mut dp = 0;
                    for &b in b"wp_" { dk[dp] = b; dp += 1; }
                    for &b in day.as_bytes() { if dp < 32 { dk[dp] = b; dp += 1; } }
                    let key = unsafe { core::str::from_utf8_unchecked(&dk[..dp]) };
                    let existing = kv_read(key).unwrap_or("");
                    let mut w = W::new();
                    if !existing.is_empty() { w.s(existing); w.s("\n"); }
                    // exercise_name|sets|reps|weight|done
                    let sets = find_json_str(body, "sets").unwrap_or("3");
                    let reps = find_json_str(body, "reps").unwrap_or("10");
                    let weight = find_json_str(body, "weight").unwrap_or("0");
                    w.s(name); w.s("|"); w.s(sets); w.s("|"); w.s(reps); w.s("|"); w.s(weight); w.s("|0");
                    kv_write(key, w.out());
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"name required"}"#, "application/json"); }
            } else if action == "toggle_done" {
                let day = find_json_str(body, "day").unwrap_or("");
                let idx = find_json_str(body, "index").map(|s| parse_u32(s)).unwrap_or(0);
                let mut dk = [0u8; 32]; let mut dp = 0;
                for &b in b"wp_" { dk[dp] = b; dp += 1; }
                for &b in day.as_bytes() { if dp < 32 { dk[dp] = b; dp += 1; } }
                let key = unsafe { core::str::from_utf8_unchecked(&dk[..dp]) };
                let existing = kv_read(key).unwrap_or("");
                let eb = existing.as_bytes();
                let mut w = W::new();
                let mut p = 0; let mut line_num: u32 = 0;
                while p < eb.len() {
                    let ls = p;
                    while p < eb.len() && eb[p] != b'\n' { p += 1; }
                    let line = unsafe { core::str::from_utf8_unchecked(&eb[ls..p]) };
                    if p < eb.len() { p += 1; }
                    if w.pos > 0 { w.s("\n"); }
                    if line_num == idx {
                        let lb = line.as_bytes();
                        let mut last_pipe = 0; let mut li = 0;
                        while li < lb.len() { if lb[li] == b'|' { last_pipe = li; } li += 1; }
                        w.s(&line[..last_pipe + 1]);
                        if last_pipe + 1 < lb.len() && lb[last_pipe + 1] == b'1' { w.s("0"); } else { w.s("1"); }
                    } else { w.s(line); }
                    line_num += 1;
                }
                kv_write(key, w.out());
                // Track total workouts completed
                let total = kv_read("wp_total").map(|s| parse_u32(s)).unwrap_or(0);
                let mut tw = W::new(); tw.n(total + 1); kv_write("wp_total", tw.out());
                respond(200, r#"{"ok":true}"#, "application/json");
            } else if action == "log_session" {
                let date = find_json_str(body, "date").unwrap_or("");
                let duration = find_json_str(body, "duration").unwrap_or("0");
                let notes = find_json_str(body, "notes").unwrap_or("");
                let existing = kv_read("wp_log").unwrap_or("");
                let mut w = W::new();
                if !existing.is_empty() { w.s(existing); w.s("\n"); }
                w.s(date); w.s("|"); w.s(duration); w.s("|"); w.s(notes);
                kv_write("wp_log", w.out());
                respond(200, r#"{"ok":true}"#, "application/json");
            } else if action == "delete" {
                let day = find_json_str(body, "day").unwrap_or("");
                let idx = find_json_str(body, "index").map(|s| parse_u32(s)).unwrap_or(0);
                let mut dk = [0u8; 32]; let mut dp = 0;
                for &b in b"wp_" { dk[dp] = b; dp += 1; }
                for &b in day.as_bytes() { if dp < 32 { dk[dp] = b; dp += 1; } }
                let key = unsafe { core::str::from_utf8_unchecked(&dk[..dp]) };
                let existing = kv_read(key).unwrap_or("");
                let eb = existing.as_bytes();
                let mut w = W::new();
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
            } else { respond(400, r#"{"error":"unknown"}"#, "application/json"); }
        } else { respond(400, r#"{"error":"missing action"}"#, "application/json"); }
        return;
    }

    // GET
    let total = kv_read("wp_total").map(|s| parse_u32(s)).unwrap_or(0);
    let days = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];
    let day_labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Workout Planner</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#0d1117;color:#c9d1d9;font-family:'Segoe UI',sans-serif;padding:20px}");
    w.s("h1{text-align:center;color:#f0883e;margin-bottom:16px}");
    w.s(".stat{text-align:center;color:#8b949e;margin-bottom:20px;font-size:14px}.stat span{color:#f0883e;font-weight:bold}");
    w.s(".tabs{display:flex;gap:4px;margin-bottom:16px;overflow-x:auto;padding-bottom:4px}");
    w.s(".tab{padding:8px 14px;background:#161b22;border:1px solid #30363d;border-radius:6px;cursor:pointer;color:#8b949e;font-size:13px;white-space:nowrap;flex-shrink:0}");
    w.s(".tab.active{background:#0d2744;border-color:#58a6ff;color:#58a6ff}");
    w.s(".day-content{display:none}.day-content.active{display:block}");
    w.s(".form{display:flex;gap:6px;margin-bottom:16px;flex-wrap:wrap}input,select{padding:8px;background:#0d1117;border:1px solid #30363d;color:#c9d1d9;border-radius:4px;font-size:13px}");
    w.s("input{flex:1;min-width:80px}button{padding:8px 14px;background:#238636;color:#fff;border:none;border-radius:4px;cursor:pointer;font-size:13px}");
    w.s(".exercise{display:flex;align-items:center;gap:10px;padding:10px;background:#161b22;border-radius:6px;margin-bottom:4px}");
    w.s(".exercise.done{opacity:0.5}.exercise .name{flex:1;font-size:14px}.exercise .info{font-size:12px;color:#8b949e}");
    w.s(".chk{width:18px;height:18px;cursor:pointer;accent-color:#238636}");
    w.s(".del{background:#21262d;border:none;color:#f85149;cursor:pointer;padding:4px 8px;border-radius:4px;font-size:12px}");
    w.s("</style></head><body><h1>Workout Planner</h1>");
    w.s("<div class='stat'>Total exercises completed: <span>"); w.n(total); w.s("</span></div>");

    // Day tabs
    w.s("<div class='tabs'>");
    let mut di = 0;
    while di < 7 {
        w.s("<div class='tab"); if di == 0 { w.s(" active"); }
        w.s("' onclick='showDay("); w.n(di as u32); w.s(")'>"); w.s(day_labels[di]); w.s("</div>");
        di += 1;
    }
    w.s("</div>");

    // Day contents
    di = 0;
    while di < 7 {
        let day = days[di];
        w.s("<div class='day-content"); if di == 0 { w.s(" active"); }
        w.s("' id='day"); w.n(di as u32); w.s("'>");
        w.s("<div class='form'><input id='ex_"); w.s(day); w.s("' placeholder='Exercise name'>");
        w.s("<input type='number' id='sets_"); w.s(day); w.s("' placeholder='Sets' value='3' style='width:60px'>");
        w.s("<input type='number' id='reps_"); w.s(day); w.s("' placeholder='Reps' value='10' style='width:60px'>");
        w.s("<input type='number' id='wt_"); w.s(day); w.s("' placeholder='Weight' value='0' style='width:70px'>");
        w.s("<button onclick=\"addEx('"); w.s(day); w.s("')\">Add</button></div>");

        // Load exercises for this day
        let mut dk = [0u8; 32]; let mut dp = 0;
        for &b in b"wp_" { dk[dp] = b; dp += 1; }
        for &b in day.as_bytes() { if dp < 32 { dk[dp] = b; dp += 1; } }
        let key = unsafe { core::str::from_utf8_unchecked(&dk[..dp]) };
        let exercises = kv_read(key).unwrap_or("");
        if !exercises.is_empty() {
            let eb = exercises.as_bytes();
            let mut p = 0; let mut idx: u32 = 0;
            while p < eb.len() {
                let ls = p;
                while p < eb.len() && eb[p] != b'\n' { p += 1; }
                let line = unsafe { core::str::from_utf8_unchecked(&eb[ls..p]) };
                if p < eb.len() { p += 1; }
                let lb = line.as_bytes();
                let mut pipes = [0usize; 4]; let mut pi = 0; let mut li = 0;
                while li < lb.len() && pi < 4 { if lb[li] == b'|' { pipes[pi] = li; pi += 1; } li += 1; }
                if pi >= 4 {
                    let name = &line[..pipes[0]];
                    let sets = &line[pipes[0]+1..pipes[1]];
                    let reps = &line[pipes[1]+1..pipes[2]];
                    let weight = &line[pipes[2]+1..pipes[3]];
                    let done = pipes[3] + 1 < lb.len() && lb[pipes[3] + 1] == b'1';
                    w.s("<div class='exercise"); if done { w.s(" done"); }
                    w.s("'><input type='checkbox' class='chk'");
                    if done { w.s(" checked"); }
                    w.s(" onchange=\"toggle('"); w.s(day); w.s("',"); w.n(idx); w.s(")\">");
                    w.s("<span class='name'>"); w.s(name); w.s("</span>");
                    w.s("<span class='info'>"); w.s(sets); w.s("x"); w.s(reps);
                    if weight != "0" { w.s(" @ "); w.s(weight); w.s("kg"); }
                    w.s("</span><button class='del' onclick=\"delEx('"); w.s(day); w.s("',"); w.n(idx); w.s(")\">x</button></div>");
                }
                idx += 1;
            }
        }
        w.s("</div>");
        di += 1;
    }

    w.s("<script>const B=location.pathname;");
    w.s("function showDay(i){document.querySelectorAll('.day-content').forEach((d,j)=>d.classList.toggle('active',j===i));document.querySelectorAll('.tab').forEach((t,j)=>t.classList.toggle('active',j===i));}");
    w.s("async function addEx(day){const n=document.getElementById('ex_'+day).value.trim();if(!n)return;const s=document.getElementById('sets_'+day).value;const r=document.getElementById('reps_'+day).value;const wt=document.getElementById('wt_'+day).value;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add_workout',day:day,name:n,sets:s,reps:r,weight:wt})});location.reload();}");
    w.s("async function toggle(day,i){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'toggle_done',day:day,index:String(i)})});location.reload();}");
    w.s("async function delEx(day,i){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',day:day,index:String(i)})});location.reload();}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
