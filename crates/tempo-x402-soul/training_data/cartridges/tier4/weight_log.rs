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

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize)) };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let body = find_json_str(request, "body").unwrap_or("");
    host_log(0, "weight_log: handling request");

    if method == "POST" {
        let date = find_json_str(body, "date").unwrap_or("");
        let weight = find_json_str(body, "weight").unwrap_or("");
        if !date.is_empty() && !weight.is_empty() {
            let existing = kv_read("wt_data").unwrap_or("");
            let mut w = W::new();
            if !existing.is_empty() { w.s(existing); w.s("\n"); }
            w.s(date); w.s("|"); w.s(weight);
            kv_write("wt_data", w.out());
            if let Some(goal) = find_json_str(body, "goal") {
                kv_write("wt_goal", goal);
            }
            respond(200, r#"{"ok":true}"#, "application/json");
        } else { respond(400, r#"{"error":"missing fields"}"#, "application/json"); }
        return;
    }

    let data = kv_read("wt_data").unwrap_or("");
    let goal = kv_read("wt_goal").unwrap_or("");
    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Weight Log</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#0d1117;color:#c9d1d9;font-family:'Segoe UI',sans-serif;padding:30px 20px;display:flex;justify-content:center}");
    w.s(".c{max-width:600px;width:100%}h1{text-align:center;color:#3fb950;margin-bottom:20px}");
    w.s(".form{background:#161b22;padding:16px;border-radius:10px;margin-bottom:20px;display:flex;gap:8px;flex-wrap:wrap;align-items:end}");
    w.s(".field{display:flex;flex-direction:column;gap:4px}.field label{font-size:11px;color:#8b949e;text-transform:uppercase}");
    w.s("input{padding:10px;background:#0d1117;border:1px solid #30363d;color:#c9d1d9;border-radius:6px;font-size:14px;width:120px}");
    w.s("button{padding:10px 18px;background:#238636;color:#fff;border:none;border-radius:6px;cursor:pointer;font-size:14px}");
    w.s(".chart{background:#161b22;padding:16px;border-radius:10px;margin-bottom:16px;min-height:200px}");
    w.s(".chart h3{color:#8b949e;font-size:12px;margin-bottom:10px}");
    w.s(".bar-row{display:flex;align-items:center;gap:8px;margin-bottom:4px}");
    w.s(".bar-date{width:80px;font-size:11px;color:#8b949e;text-align:right}.bar{height:20px;border-radius:3px;background:#238636;transition:width 0.3s}.bar-val{font-size:12px;color:#3fb950;min-width:50px}");
    w.s(".entries{max-height:300px;overflow-y:auto}");
    w.s(".entry{display:flex;justify-content:space-between;padding:8px 12px;background:#161b22;border-radius:6px;margin-bottom:4px;font-size:14px}");
    w.s(".entry .date{color:#8b949e}.entry .val{color:#3fb950;font-weight:bold}");
    w.s("</style></head><body><div class='c'><h1>Weight Log</h1>");
    w.s("<div class='form'><div class='field'><label>Date</label><input type='date' id='date'></div><div class='field'><label>Weight</label><input type='number' id='weight' step='0.1' placeholder='kg'></div><div class='field'><label>Goal</label><input type='number' id='goal' step='0.1' placeholder='kg' value='"); w.s(goal); w.s("'></div>");
    w.s("<button onclick='logWeight()'>Log</button></div>");

    // Simple bar chart
    if !data.is_empty() {
        w.s("<div class='chart'><h3>Recent Entries</h3>");
        let db = data.as_bytes();
        // Count entries
        let mut count: u32 = 0; let mut p = 0;
        while p < db.len() { if db[p] == b'\n' { count += 1; } p += 1; }
        count += 1;
        // Show last 14
        let skip = if count > 14 { count - 14 } else { 0 };
        p = 0; let mut idx: u32 = 0;
        while p < db.len() {
            let ls = p;
            while p < db.len() && db[p] != b'\n' { p += 1; }
            let line = unsafe { core::str::from_utf8_unchecked(&db[ls..p]) };
            if p < db.len() { p += 1; }
            if idx >= skip {
                let lb = line.as_bytes();
                let mut sep = 0; while sep < lb.len() && lb[sep] != b'|' { sep += 1; }
                let date = &line[..sep];
                let val = if sep + 1 < lb.len() { &line[sep + 1..] } else { "0" };
                // Approximate bar width (assume weight 50-150 range, map to 0-100%)
                w.s("<div class='bar-row'><span class='bar-date'>"); w.s(date);
                w.s("</span><div class='bar' style='width:"); w.s(val); w.s("px'></div>");
                w.s("<span class='bar-val'>"); w.s(val); w.s(" kg</span></div>");
            }
            idx += 1;
        }
        w.s("</div>");
    }

    w.s("</div><script>const B=location.pathname;document.getElementById('date').valueAsDate=new Date();");
    w.s("async function logWeight(){const d=document.getElementById('date').value;const wt=document.getElementById('weight').value;const g=document.getElementById('goal').value;if(!wt)return;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({date:d,weight:wt,goal:g})});location.reload();}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
