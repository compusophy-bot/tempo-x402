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

    host_log(0, "tip_calculator: handling request");

    // POST — save a bill to history
    if method == "POST" {
        if let Some(bill) = find_json_str(body, "bill") {
            let tip_pct = find_json_str(body, "tip_pct").unwrap_or("15");
            let split = find_json_str(body, "split").unwrap_or("1");
            let existing = kv_read("tip_history").unwrap_or("");
            let mut w = W::new();
            w.s(bill); w.s("|"); w.s(tip_pct); w.s("|"); w.s(split);
            if !existing.is_empty() { w.s("\n"); w.s(existing); }
            // Keep last 15
            let bytes = w.out().as_bytes();
            let mut lines = 0; let mut trunc = bytes.len(); let mut p = 0;
            while p < bytes.len() { if bytes[p] == b'\n' { lines += 1; if lines >= 15 { trunc = p; break; } } p += 1; }
            let trimmed = unsafe { core::str::from_utf8_unchecked(&bytes[..trunc]) };
            kv_write("tip_history", trimmed);
            respond(200, r#"{"ok":true}"#, "application/json");
        } else { respond(400, r#"{"error":"missing bill"}"#, "application/json"); }
        return;
    }

    // GET — calculator UI (all math client-side)
    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Tip Calculator</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#0f172a;color:#e2e8f0;font-family:'Segoe UI',sans-serif;padding:30px 20px;display:flex;justify-content:center}");
    w.s(".c{max-width:450px;width:100%}h1{text-align:center;color:#38bdf8;margin-bottom:24px}");
    w.s(".card{background:#1e293b;padding:24px;border-radius:16px;margin-bottom:16px}");
    w.s("label{display:block;font-size:13px;color:#94a3b8;margin-bottom:4px;text-transform:uppercase;letter-spacing:1px}");
    w.s("input[type=number]{width:100%;padding:14px;background:#0f172a;border:2px solid #334155;color:#e2e8f0;border-radius:8px;font-size:24px;text-align:center;margin-bottom:16px}");
    w.s("input:focus{border-color:#38bdf8;outline:none}");
    w.s(".tip-btns{display:flex;gap:8px;margin-bottom:16px;flex-wrap:wrap}");
    w.s(".tip-btn{flex:1;padding:12px;background:#334155;border:2px solid transparent;color:#e2e8f0;border-radius:8px;cursor:pointer;font-size:16px;font-weight:600;text-align:center;min-width:60px}");
    w.s(".tip-btn.active{background:#0c4a6e;border-color:#38bdf8;color:#38bdf8}");
    w.s(".split-row{display:flex;align-items:center;gap:12px;margin-bottom:16px}");
    w.s(".split-btn{width:44px;height:44px;border-radius:50%;background:#334155;border:none;color:#e2e8f0;font-size:20px;cursor:pointer}");
    w.s(".split-val{font-size:24px;font-weight:bold;min-width:40px;text-align:center}");
    w.s(".result{background:#0f172a;padding:20px;border-radius:12px;margin-top:16px}");
    w.s(".result-row{display:flex;justify-content:space-between;padding:8px 0;font-size:16px}.result-row.total{font-size:22px;font-weight:bold;color:#38bdf8;border-top:2px solid #334155;margin-top:8px;padding-top:12px}");
    w.s("button.save{width:100%;padding:14px;background:#38bdf8;color:#0f172a;border:none;border-radius:8px;cursor:pointer;font-size:16px;font-weight:600;margin-top:12px}");
    w.s(".history{background:#1e293b;padding:16px;border-radius:12px}.history h3{color:#94a3b8;font-size:12px;text-transform:uppercase;margin-bottom:8px}");
    w.s(".hist-row{display:flex;justify-content:space-between;padding:6px 0;font-size:13px;border-bottom:1px solid #1e293b;color:#64748b}");
    w.s("</style></head><body><div class='c'><h1>Tip Calculator</h1>");
    w.s("<div class='card'><label>Bill Amount ($)</label><input type='number' id='bill' value='50' step='0.01' min='0' oninput='calc()'>");
    w.s("<label>Tip Percentage</label><div class='tip-btns'><div class='tip-btn' onclick='setTip(10)'>10%</div><div class='tip-btn active' onclick='setTip(15)'>15%</div><div class='tip-btn' onclick='setTip(18)'>18%</div><div class='tip-btn' onclick='setTip(20)'>20%</div><div class='tip-btn' onclick='setTip(25)'>25%</div></div>");
    w.s("<label>Split Between</label><div class='split-row'><button class='split-btn' onclick='changeSplit(-1)'>-</button><span class='split-val' id='splitVal'>1</span><button class='split-btn' onclick='changeSplit(1)'>+</button></div>");
    w.s("<div class='result'><div class='result-row'><span>Tip</span><span id='tipAmt'>$7.50</span></div><div class='result-row'><span>Total</span><span id='totalAmt'>$57.50</span></div><div class='result-row total'><span>Per Person</span><span id='perPerson'>$57.50</span></div></div>");
    w.s("<button class='save' onclick='saveBill()'>Save to History</button></div>");

    // History
    let history = kv_read("tip_history").unwrap_or("");
    if !history.is_empty() {
        w.s("<div class='history'><h3>Recent Bills</h3>");
        let hb = history.as_bytes();
        let mut p = 0;
        while p < hb.len() {
            let ls = p;
            while p < hb.len() && hb[p] != b'\n' { p += 1; }
            let line = unsafe { core::str::from_utf8_unchecked(&hb[ls..p]) };
            if p < hb.len() { p += 1; }
            // Parse bill|tip%|split
            let lb = line.as_bytes();
            let mut p1 = 0; while p1 < lb.len() && lb[p1] != b'|' { p1 += 1; }
            let mut p2 = p1 + 1; while p2 < lb.len() && lb[p2] != b'|' { p2 += 1; }
            let bill_s = &line[..p1];
            let tip_s = if p1 + 1 < p2 { &line[p1 + 1..p2] } else { "15" };
            let split_s = if p2 + 1 < lb.len() { &line[p2 + 1..] } else { "1" };
            w.s("<div class='hist-row'><span>$");
            w.s(bill_s);
            w.s("</span><span>");
            w.s(tip_s);
            w.s("% tip</span><span>");
            w.s(split_s);
            w.s(" people</span></div>");
        }
        w.s("</div>");
    }

    w.s("</div><script>let tipPct=15;let split=1;");
    w.s("function setTip(p){tipPct=p;document.querySelectorAll('.tip-btn').forEach((b,i)=>b.classList.toggle('active',[10,15,18,20,25][i]===p));calc();}");
    w.s("function changeSplit(d){split=Math.max(1,split+d);document.getElementById('splitVal').textContent=split;calc();}");
    w.s("function calc(){const bill=parseFloat(document.getElementById('bill').value)||0;const tip=bill*tipPct/100;const total=bill+tip;const pp=total/split;document.getElementById('tipAmt').textContent='$'+tip.toFixed(2);document.getElementById('totalAmt').textContent='$'+total.toFixed(2);document.getElementById('perPerson').textContent='$'+pp.toFixed(2);}");
    w.s("async function saveBill(){const b=document.getElementById('bill').value;await fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({bill:b,tip_pct:String(tipPct),split:String(split)})});location.reload();}");
    w.s("calc();");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
