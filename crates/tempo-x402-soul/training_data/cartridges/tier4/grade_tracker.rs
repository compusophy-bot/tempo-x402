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

fn write_key(buf: &mut [u8], prefix: &[u8], num: u32) -> usize {
    let mut pos = 0;
    for &b in prefix { buf[pos] = b; pos += 1; }
    if num == 0 { buf[pos] = b'0'; return pos + 1; }
    let mut d = [0u8; 10]; let mut di = 0; let mut n = num;
    while n > 0 { d[di] = b'0' + (n % 10) as u8; n /= 10; di += 1; }
    while di > 0 { di -= 1; buf[pos] = d[di]; pos += 1; }
    pos
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize)) };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let body = find_json_str(request, "body").unwrap_or("");
    host_log(0, "grade_tracker: handling request");

    if method == "POST" {
        if let Some(action) = find_json_str(body, "action") {
            if action == "add_subject" {
                let name = find_json_str(body, "name").unwrap_or("");
                if !name.is_empty() {
                    let subjects = kv_read("gr_subjects").unwrap_or("");
                    let mut w = W::new();
                    if !subjects.is_empty() { w.s(subjects); w.s(","); }
                    w.s(name);
                    kv_write("gr_subjects", w.out());
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"name required"}"#, "application/json"); }
            } else if action == "add_grade" {
                let subject = find_json_str(body, "subject").unwrap_or("");
                let grade_s = find_json_str(body, "grade").unwrap_or("0");
                let label = find_json_str(body, "label").unwrap_or("Test");
                let weight_s = find_json_str(body, "weight").unwrap_or("100");
                if !subject.is_empty() {
                    let mut key = [0u8; 40]; let mut kp = 0;
                    for &b in b"gr_" { key[kp] = b; kp += 1; }
                    for &b in subject.as_bytes() { if kp < 40 { key[kp] = b; kp += 1; } }
                    let k = unsafe { core::str::from_utf8_unchecked(&key[..kp]) };
                    let existing = kv_read(k).unwrap_or("");
                    let mut w = W::new();
                    if !existing.is_empty() { w.s(existing); w.s("\n"); }
                    w.s(label); w.s("|"); w.s(grade_s); w.s("|"); w.s(weight_s);
                    kv_write(k, w.out());
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"subject required"}"#, "application/json"); }
            } else { respond(400, r#"{"error":"unknown action"}"#, "application/json"); }
        } else { respond(400, r#"{"error":"missing action"}"#, "application/json"); }
        return;
    }

    // GET
    let subjects = kv_read("gr_subjects").unwrap_or("");
    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Grade Tracker</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#0d1117;color:#c9d1d9;font-family:'Segoe UI',sans-serif;padding:30px 20px;display:flex;justify-content:center}");
    w.s(".c{max-width:700px;width:100%}h1{text-align:center;color:#f0883e;margin-bottom:20px}");
    w.s(".add-subj{display:flex;gap:8px;margin-bottom:20px}input,select{padding:10px;background:#161b22;border:1px solid #30363d;color:#c9d1d9;border-radius:6px;font-size:14px}");
    w.s("input{flex:1}button{padding:10px 18px;background:#238636;color:#fff;border:none;border-radius:6px;cursor:pointer;font-size:14px}");
    w.s(".subject{background:#161b22;border-radius:10px;padding:16px;margin-bottom:12px}");
    w.s(".subj-header{display:flex;justify-content:space-between;align-items:center;margin-bottom:12px}");
    w.s(".subj-name{font-size:18px;font-weight:bold;color:#f0883e}.avg{font-size:24px;font-weight:bold}");
    w.s(".avg.a{color:#3fb950}.avg.b{color:#58a6ff}.avg.c{color:#d29922}.avg.d{color:#f0883e}.avg.f{color:#f85149}");
    w.s(".grade-form{display:flex;gap:6px;margin-bottom:8px;flex-wrap:wrap}.grade-form input{width:100px}");
    w.s(".grades{font-size:13px;color:#8b949e}.grade-item{display:inline-block;padding:2px 8px;margin:2px;background:#21262d;border-radius:4px}");
    w.s("</style></head><body><div class='c'><h1>Grade Tracker</h1>");
    w.s("<div class='add-subj'><input id='subj' placeholder='Add subject (e.g. Math)'><button onclick='addSubj()'>Add Subject</button></div>");
    w.s("<div id='subjects'>");

    if !subjects.is_empty() {
        let sb = subjects.as_bytes();
        let mut p = 0;
        while p <= sb.len() {
            let ss = p;
            while p < sb.len() && sb[p] != b',' { p += 1; }
            let name = unsafe { core::str::from_utf8_unchecked(&sb[ss..p]) };
            p += 1;
            if name.is_empty() { continue; }

            // Read grades for this subject
            let mut key = [0u8; 40]; let mut kp = 0;
            for &b in b"gr_" { key[kp] = b; kp += 1; }
            for &b in name.as_bytes() { if kp < 40 { key[kp] = b; kp += 1; } }
            let k = unsafe { core::str::from_utf8_unchecked(&key[..kp]) };
            let grades_data = kv_read(k).unwrap_or("");

            // Compute weighted average
            let mut total_weighted: u32 = 0;
            let mut total_weight: u32 = 0;
            let gb = grades_data.as_bytes();
            let mut gp = 0;
            while gp < gb.len() {
                let ls = gp;
                while gp < gb.len() && gb[gp] != b'\n' { gp += 1; }
                let line = unsafe { core::str::from_utf8_unchecked(&gb[ls..gp]) };
                if gp < gb.len() { gp += 1; }
                // Parse label|grade|weight
                let lb = line.as_bytes();
                let mut p1 = 0; while p1 < lb.len() && lb[p1] != b'|' { p1 += 1; }
                let mut p2 = p1 + 1; while p2 < lb.len() && lb[p2] != b'|' { p2 += 1; }
                let grade = if p1 + 1 < p2 { parse_u32(&line[p1 + 1..p2]) } else { 0 };
                let weight = if p2 + 1 < lb.len() { parse_u32(&line[p2 + 1..]) } else { 100 };
                total_weighted += grade * weight;
                total_weight += weight;
            }
            let avg = if total_weight > 0 { total_weighted / total_weight } else { 0 };
            let letter = if avg >= 90 { "A" } else if avg >= 80 { "B" } else if avg >= 70 { "C" } else if avg >= 60 { "D" } else { "F" };
            let color = if avg >= 90 { "a" } else if avg >= 80 { "b" } else if avg >= 70 { "c" } else if avg >= 60 { "d" } else { "f" };

            w.s("<div class='subject'><div class='subj-header'><span class='subj-name'>");
            w.s(name);
            w.s("</span><span class='avg "); w.s(color); w.s("'>");
            w.n(avg); w.s("% ("); w.s(letter); w.s(")</span></div>");
            w.s("<div class='grade-form'><input id='lbl_"); w.s(name); w.s("' placeholder='Label' value='Test'>");
            w.s("<input type='number' id='grd_"); w.s(name); w.s("' placeholder='Grade' min='0' max='100'>");
            w.s("<input type='number' id='wgt_"); w.s(name); w.s("' placeholder='Weight' value='100' min='1'>");
            w.s("<button onclick=\"addGrade('"); w.s(name); w.s("')\">Add</button></div>");

            // Show grades
            if !grades_data.is_empty() {
                w.s("<div class='grades'>");
                gp = 0;
                while gp < gb.len() {
                    let ls = gp;
                    while gp < gb.len() && gb[gp] != b'\n' { gp += 1; }
                    let line = unsafe { core::str::from_utf8_unchecked(&gb[ls..gp]) };
                    if gp < gb.len() { gp += 1; }
                    let lb = line.as_bytes();
                    let mut p1 = 0; while p1 < lb.len() && lb[p1] != b'|' { p1 += 1; }
                    let mut p2 = p1 + 1; while p2 < lb.len() && lb[p2] != b'|' { p2 += 1; }
                    let label = &line[..p1];
                    let grade_s = if p1 + 1 < p2 { &line[p1 + 1..p2] } else { "0" };
                    w.s("<span class='grade-item'>"); w.s(label); w.s(": "); w.s(grade_s); w.s("%</span>");
                }
                w.s("</div>");
            }
            w.s("</div>");
        }
    }

    w.s("</div></div><script>const B=location.pathname;");
    w.s("async function addSubj(){const n=document.getElementById('subj').value.trim();if(!n)return;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add_subject',name:n})});location.reload();}");
    w.s("async function addGrade(subj){const l=document.getElementById('lbl_'+subj).value;const g=document.getElementById('grd_'+subj).value;const w=document.getElementById('wgt_'+subj).value;if(!g)return;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add_grade',subject:subj,label:l,grade:g,weight:w})});location.reload();}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
