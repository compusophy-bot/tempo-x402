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

fn respond(status: i32, body: &str, content_type: &str) {
    unsafe {
        response(status, body.as_ptr(), body.len() as i32, content_type.as_ptr(), content_type.len() as i32);
    }
}

fn host_log(level: i32, msg: &str) {
    unsafe { log(level, msg.as_ptr(), msg.len() as i32); }
}

fn find_json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let key_bytes = key.as_bytes();
    let json_bytes = json.as_bytes();
    let mut i = 0;
    while i + key_bytes.len() + 3 < json_bytes.len() {
        if json_bytes[i] == b'"' {
            let start = i + 1;
            if start + key_bytes.len() < json_bytes.len()
                && &json_bytes[start..start + key_bytes.len()] == key_bytes
                && json_bytes[start + key_bytes.len()] == b'"'
            {
                let mut j = start + key_bytes.len() + 1;
                while j < json_bytes.len() && (json_bytes[j] == b':' || json_bytes[j] == b' ') { j += 1; }
                if j < json_bytes.len() && json_bytes[j] == b'"' {
                    let val_start = j + 1;
                    let mut val_end = val_start;
                    while val_end < json_bytes.len() && json_bytes[val_end] != b'"' { val_end += 1; }
                    return core::str::from_utf8(&json_bytes[val_start..val_end]).ok();
                }
            }
        }
        i += 1;
    }
    None
}

fn kv_read(key: &str) -> Option<&'static str> {
    unsafe {
        let result = kv_get(key.as_ptr(), key.len() as i32);
        if result < 0 { return None; }
        let ptr = (result >> 32) as *const u8;
        let len = (result & 0xFFFFFFFF) as usize;
        let bytes = core::slice::from_raw_parts(ptr, len);
        core::str::from_utf8(bytes).ok()
    }
}

fn kv_write(key: &str, value: &str) {
    unsafe {
        kv_set(key.as_ptr(), key.len() as i32, value.as_ptr(), value.len() as i32);
    }
}

static mut BUF: [u8; 65536] = [0u8; 65536];

struct BufWriter {
    pos: usize,
}

impl BufWriter {
    fn new() -> Self { Self { pos: 0 } }

    fn push_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        unsafe {
            let end = (self.pos + bytes.len()).min(BUF.len());
            let copy_len = end - self.pos;
            BUF[self.pos..end].copy_from_slice(&bytes[..copy_len]);
            self.pos = end;
        }
    }

    fn push_num(&mut self, mut n: u32) {
        if n == 0 { self.push_str("0"); return; }
        let mut digits = [0u8; 10];
        let mut i = 0;
        while n > 0 { digits[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
        while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = digits[i]; self.pos += 1; } } }
    }

    fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) }
    }
}

#[no_mangle]
pub extern "C" fn handle(method_ptr: *const u8, method_len: i32, _path_ptr: *const u8, _path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if method.as_bytes()[0] == b'P' {
        if let Some(target) = find_json_str(body, "target") {
            kv_write("countdown_target", target);
            if let Some(label) = find_json_str(body, "label") {
                kv_write("countdown_label", label);
            }
            respond(200, "{\"ok\":true}", "application/json");
        } else {
            respond(400, "{\"error\":\"missing target\"}", "application/json");
        }
        return;
    }

    let target = kv_read("countdown_target").unwrap_or("");
    let label = kv_read("countdown_label").unwrap_or("Countdown");

    let mut w = BufWriter::new();
    w.push_str("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Countdown Timer</title><style>");
    w.push_str("*{margin:0;padding:0;box-sizing:border-box}");
    w.push_str("body{background:#0a0a1a;color:#e0e0e0;font-family:'Segoe UI',sans-serif;min-height:100vh;display:flex;justify-content:center;align-items:center;padding:20px}");
    w.push_str(".container{text-align:center;max-width:700px;width:100%}");
    w.push_str(".event-label{font-size:24px;color:#c084fc;margin-bottom:16px;font-weight:300;letter-spacing:2px;text-transform:uppercase}");
    w.push_str(".timer-display{display:flex;justify-content:center;gap:20px;margin-bottom:40px}");
    w.push_str(".time-unit{background:linear-gradient(180deg,#1a1a3e,#0d0d2b);border:1px solid #333;border-radius:16px;padding:24px 16px;min-width:120px}");
    w.push_str(".time-value{font-size:64px;font-weight:bold;color:#e0e0ff;font-family:'SF Mono',monospace;line-height:1}");
    w.push_str(".time-label{font-size:12px;color:#666;text-transform:uppercase;letter-spacing:3px;margin-top:8px}");
    w.push_str(".separator{font-size:48px;color:#444;display:flex;align-items:center;font-weight:bold}");
    w.push_str(".setup{background:#1a1a3e;border-radius:16px;padding:32px;margin-top:32px}");
    w.push_str(".setup h3{color:#c084fc;margin-bottom:20px;font-size:18px}");
    w.push_str(".form-row{display:flex;gap:12px;align-items:end;justify-content:center;flex-wrap:wrap}");
    w.push_str(".field{text-align:left}");
    w.push_str(".field label{display:block;font-size:12px;color:#888;margin-bottom:4px;text-transform:uppercase;letter-spacing:1px}");
    w.push_str(".field input{padding:12px 16px;background:#0a0a1a;border:2px solid #333;color:#e0e0e0;border-radius:8px;font-size:16px;outline:none;width:220px}");
    w.push_str(".field input:focus{border-color:#c084fc}");
    w.push_str(".set-btn{padding:12px 28px;background:#c084fc;color:#0a0a1a;border:none;border-radius:8px;font-size:16px;font-weight:bold;cursor:pointer;height:48px}");
    w.push_str(".set-btn:hover{background:#a855f7}");
    w.push_str(".expired{color:#f87171;font-size:24px;margin-top:20px;animation:pulse 1s infinite}");
    w.push_str("@keyframes pulse{0%,100%{opacity:1}50%{opacity:0.5}}");
    w.push_str(".particles{position:fixed;top:0;left:0;width:100%;height:100%;pointer-events:none;overflow:hidden;z-index:-1}");
    w.push_str(".particle{position:absolute;width:4px;height:4px;background:#c084fc;border-radius:50%;opacity:0.3;animation:float 6s infinite}");
    w.push_str("@keyframes float{0%{transform:translateY(100vh) rotate(0deg);opacity:0}10%{opacity:0.3}90%{opacity:0.3}100%{transform:translateY(-10vh) rotate(720deg);opacity:0}}");
    w.push_str("</style></head><body>");

    // Floating particles
    w.push_str("<div class='particles'>");
    let mut p = 0;
    while p < 20 {
        w.push_str("<div class='particle' style='left:");
        w.push_num(p * 5);
        w.push_str("%;animation-delay:");
        w.push_num(p % 6);
        w.push_str("s;animation-duration:");
        w.push_num(4 + p % 5);
        w.push_str("s'></div>");
        p += 1;
    }
    w.push_str("</div>");

    w.push_str("<div class='container'>");
    w.push_str("<div class='event-label' id='eventLabel'>");
    w.push_str(label);
    w.push_str("</div>");
    w.push_str("<div class='timer-display'>");
    w.push_str("<div class='time-unit'><div class='time-value' id='days'>00</div><div class='time-label'>Days</div></div>");
    w.push_str("<div class='separator'>:</div>");
    w.push_str("<div class='time-unit'><div class='time-value' id='hours'>00</div><div class='time-label'>Hours</div></div>");
    w.push_str("<div class='separator'>:</div>");
    w.push_str("<div class='time-unit'><div class='time-value' id='minutes'>00</div><div class='time-label'>Minutes</div></div>");
    w.push_str("<div class='separator'>:</div>");
    w.push_str("<div class='time-unit'><div class='time-value' id='seconds'>00</div><div class='time-label'>Seconds</div></div>");
    w.push_str("</div>");
    w.push_str("<div id='expiredMsg'></div>");

    w.push_str("<div class='setup'><h3>Set Countdown Target</h3>");
    w.push_str("<div class='form-row'>");
    w.push_str("<div class='field'><label>Event Name</label><input type='text' id='labelInput' placeholder='My Event'></div>");
    w.push_str("<div class='field'><label>Target Date & Time</label><input type='datetime-local' id='targetInput'></div>");
    w.push_str("<button class='set-btn' onclick='setTarget()'>Set</button>");
    w.push_str("</div></div></div>");

    w.push_str("<script>");
    w.push_str("const BASE=location.pathname;");
    w.push_str("let targetDate=");
    if target.len() > 0 {
        w.push_str("new Date('");
        w.push_str(target);
        w.push_str("')");
    } else {
        w.push_str("null");
    }
    w.push_str(";");
    w.push_str("function pad(n){return n<10?'0'+n:String(n);}");
    w.push_str("function updateTimer(){if(!targetDate){document.getElementById('expiredMsg').innerHTML='<p style=\"color:#888\">No target set yet</p>';return;}");
    w.push_str("const now=new Date();const diff=targetDate-now;");
    w.push_str("if(diff<=0){document.getElementById('days').textContent='00';document.getElementById('hours').textContent='00';document.getElementById('minutes').textContent='00';document.getElementById('seconds').textContent='00';document.getElementById('expiredMsg').innerHTML='<div class=\"expired\">Event has arrived!</div>';return;}");
    w.push_str("const d=Math.floor(diff/86400000);const h=Math.floor((diff%86400000)/3600000);const m=Math.floor((diff%3600000)/60000);const s=Math.floor((diff%60000)/1000);");
    w.push_str("document.getElementById('days').textContent=pad(d);document.getElementById('hours').textContent=pad(h);document.getElementById('minutes').textContent=pad(m);document.getElementById('seconds').textContent=pad(s);}");
    w.push_str("async function setTarget(){const label=document.getElementById('labelInput').value||'Countdown';const target=document.getElementById('targetInput').value;if(!target){alert('Please select a date');return;}");
    w.push_str("await fetch(BASE,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({target,label})});location.reload();}");
    w.push_str("setInterval(updateTimer,1000);updateTimer();");
    w.push_str("</script></body></html>");

    respond(200, w.as_str(), "text/html");
}
