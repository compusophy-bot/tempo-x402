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
    unsafe { response(status, body.as_ptr(), body.len() as i32, content_type.as_ptr(), content_type.len() as i32); }
}

fn host_log(level: i32, msg: &str) {
    unsafe { log(level, msg.as_ptr(), msg.len() as i32); }
}

fn find_json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let kb = key.as_bytes();
    let jb = json.as_bytes();
    let mut i = 0;
    while i + kb.len() + 3 < jb.len() {
        if jb[i] == b'"' {
            let s = i + 1;
            if s + kb.len() < jb.len() && &jb[s..s + kb.len()] == kb && jb[s + kb.len()] == b'"' {
                let mut j = s + kb.len() + 1;
                while j < jb.len() && (jb[j] == b':' || jb[j] == b' ') { j += 1; }
                if j < jb.len() && jb[j] == b'"' {
                    let vs = j + 1;
                    let mut ve = vs;
                    while ve < jb.len() && jb[ve] != b'"' { ve += 1; }
                    return core::str::from_utf8(&jb[vs..ve]).ok();
                }
            }
        }
        i += 1;
    }
    None
}

fn kv_read(key: &str) -> Option<&'static str> {
    unsafe {
        let r = kv_get(key.as_ptr(), key.len() as i32);
        if r < 0 { return None; }
        let ptr = (r >> 32) as *const u8;
        let len = (r & 0xFFFFFFFF) as usize;
        core::str::from_utf8(core::slice::from_raw_parts(ptr, len)).ok()
    }
}

fn kv_write(key: &str, value: &str) {
    unsafe { kv_set(key.as_ptr(), key.len() as i32, value.as_ptr(), value.len() as i32); }
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}

static mut BUF: [u8; 65536] = [0u8; 65536];
fn buf_write(pos: usize, s: &str) -> usize {
    let b = s.as_bytes();
    let end = (pos + b.len()).min(unsafe { BUF.len() });
    unsafe { BUF[pos..end].copy_from_slice(&b[..end - pos]); }
    end
}
fn buf_as_str(len: usize) -> &'static str {
    unsafe { core::str::from_utf8_unchecked(&BUF[..len]) }
}

fn write_usize(pos: usize, mut n: usize) -> usize {
    if n == 0 { return buf_write(pos, "0"); }
    static mut DIGITS: [u8; 20] = [0u8; 20];
    let mut i = 0;
    while n > 0 { unsafe { DIGITS[i] = b'0' + (n % 10) as u8; } n /= 10; i += 1; }
    let mut p = pos;
    while i > 0 { i -= 1; let d = unsafe { DIGITS[i] }; let s = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(&d, 1)) }; p = buf_write(p, s); }
    p
}

fn parse_usize(s: &str) -> usize {
    let mut n = 0usize;
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() { if b[i] >= b'0' && b[i] <= b'9' { n = n * 10 + (b[i] - b'0') as usize; } i += 1; }
    n
}

#[no_mangle]
pub extern "C" fn handle_request(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("add");
        let name = find_json_str(body, "name").unwrap_or("");
        let date = find_json_str(body, "date").unwrap_or("");
        let idx_str = find_json_str(body, "index").unwrap_or("0");

        if action == "add" && name.len() > 0 && date.len() > 0 {
            let existing = kv_read("countdowns").unwrap_or("");
            let mut bp = 0usize;
            bp = buf_write(bp, existing);
            bp = buf_write(bp, name);
            bp = buf_write(bp, "|");
            bp = buf_write(bp, date);
            bp = buf_write(bp, "\n");
            kv_write("countdowns", buf_as_str(bp));
        } else if action == "delete" {
            let target = parse_usize(idx_str);
            let existing = kv_read("countdowns").unwrap_or("");
            let eb = existing.as_bytes();
            static mut DEL: [u8; 8192] = [0u8; 8192];
            let mut np = 0usize;
            let mut epos = 0;
            let mut count = 0usize;
            while epos < eb.len() {
                let mut eend = epos;
                while eend < eb.len() && eb[eend] != b'\n' { eend += 1; }
                if eend > epos {
                    if count != target {
                        let line = &eb[epos..eend];
                        unsafe { DEL[np..np+line.len()].copy_from_slice(line); np += line.len(); DEL[np] = b'\n'; np += 1; }
                    }
                    count += 1;
                }
                epos = eend + 1;
            }
            kv_write("countdowns", unsafe { core::str::from_utf8_unchecked(&DEL[..np]) });
        }
    }

    let countdowns = kv_read("countdowns").unwrap_or("");

    let mut p = 0usize;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Countdown Timer</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#0b0c10;color:#c5c6c7;font-family:'Segoe UI',sans-serif;min-height:100vh;padding:20px;display:flex;flex-direction:column;align-items:center}
h1{color:#66fcf1;margin:20px 0;font-size:2.2em;text-shadow:0 0 20px rgba(102,252,241,0.3)}
.container{width:100%;max-width:600px}
.add-form{background:#1f2833;border-radius:16px;padding:25px;margin-bottom:25px;border:1px solid #45a29e}
.add-form h2{color:#66fcf1;margin-bottom:15px}
.form-row{display:flex;gap:10px;flex-wrap:wrap}
.form-row input{flex:1;min-width:120px;padding:12px;border:1px solid #45a29e;border-radius:10px;background:#0b0c10;color:#c5c6c7;font-size:1em}
.form-row input:focus{outline:none;border-color:#66fcf1;box-shadow:0 0 10px rgba(102,252,241,0.2)}
.add-btn{padding:12px 24px;background:#45a29e;color:#0b0c10;border:none;border-radius:10px;cursor:pointer;font-weight:bold;font-size:1em}
.add-btn:hover{background:#66fcf1}
.countdown-card{background:#1f2833;border:1px solid #45a29e;border-radius:16px;padding:25px;margin-bottom:15px;position:relative;overflow:hidden}
.countdown-card::before{content:'';position:absolute;top:0;left:0;right:0;height:3px;background:linear-gradient(90deg,#45a29e,#66fcf1)}
.cd-name{font-size:1.3em;font-weight:bold;color:#66fcf1;margin-bottom:5px}
.cd-date{color:#808b96;font-size:0.9em;margin-bottom:15px}
.cd-display{display:flex;gap:15px;justify-content:center;margin-bottom:15px}
.cd-unit{text-align:center;background:#0b0c10;border-radius:12px;padding:15px 20px;min-width:75px;border:1px solid #45a29e}
.cd-unit .num{font-size:2em;font-weight:bold;color:#66fcf1;font-family:monospace}
.cd-unit .label{font-size:0.75em;color:#808b96;margin-top:4px;text-transform:uppercase}
.cd-passed{text-align:center;color:#45a29e;font-size:1.1em;font-weight:bold;padding:15px}
.del-btn{position:absolute;top:15px;right:15px;background:#1f2833;border:1px solid #45a29e;color:#c5c6c7;border-radius:8px;padding:5px 10px;cursor:pointer;font-size:0.85em}
.del-btn:hover{background:#45a29e;color:#0b0c10}
.empty{text-align:center;color:#808b96;padding:50px;font-size:1.1em}
.glow{animation:glow 2s ease-in-out infinite alternate}
@keyframes glow{from{text-shadow:0 0 5px #66fcf1,0 0 10px #66fcf1}to{text-shadow:0 0 10px #66fcf1,0 0 20px #66fcf1,0 0 30px #45a29e}}
</style></head><body>
<h1 class="glow">&#9200; Countdown Timer</h1>
<div class="container">
<div class="add-form"><h2>New Countdown</h2><div class="form-row">
<input type="text" id="name" placeholder="Event name">
<input type="date" id="date">
<button class="add-btn" onclick="addCountdown()">Create</button></div></div>
<div id="countdowns">"##);

    let cb = countdowns.as_bytes();
    let mut cpos = 0;
    let mut idx = 0usize;
    let mut has_any = false;

    while cpos < cb.len() {
        let mut cend = cpos;
        while cend < cb.len() && cb[cend] != b'\n' { cend += 1; }
        if cend > cpos {
            let line = &cb[cpos..cend];
            let mut sep = 0;
            let mut si = 0;
            while si < line.len() { if line[si] == b'|' { sep = si; break; } si += 1; }
            if sep > 0 {
                has_any = true;
                let name = unsafe { core::str::from_utf8_unchecked(&line[..sep]) };
                let date = unsafe { core::str::from_utf8_unchecked(&line[sep+1..]) };
                p = buf_write(p, r##"<div class="countdown-card"><button class="del-btn" onclick="delCountdown("##);
                p = write_usize(p, idx);
                p = buf_write(p, r##")">&#10005;</button><div class="cd-name">"##);
                p = buf_write(p, name);
                p = buf_write(p, r##"</div><div class="cd-date">"##);
                p = buf_write(p, date);
                p = buf_write(p, r##"</div><div class="cd-display" data-target=""##);
                p = buf_write(p, date);
                p = buf_write(p, r##"">
<div class="cd-unit"><div class="num days">--</div><div class="label">Days</div></div>
<div class="cd-unit"><div class="num hours">--</div><div class="label">Hours</div></div>
<div class="cd-unit"><div class="num mins">--</div><div class="label">Min</div></div>
<div class="cd-unit"><div class="num secs">--</div><div class="label">Sec</div></div>
</div></div>"##);
                idx += 1;
            }
        }
        cpos = cend + 1;
    }

    if !has_any {
        p = buf_write(p, r##"<div class="empty">No countdowns yet. Create one for an upcoming event!</div>"##);
    }

    p = buf_write(p, r##"</div></div>
<script>
function addCountdown(){var n=document.getElementById('name').value;var d=document.getElementById('date').value;if(!n||!d)return alert('Fill in event name and date');fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add',name:n,date:d})}).then(function(){location.reload()})}
function delCountdown(i){fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',index:String(i)})}).then(function(){location.reload()})}
function updateCountdowns(){document.querySelectorAll('.cd-display').forEach(function(el){var target=new Date(el.getAttribute('data-target')+'T00:00:00');var now=new Date();var diff=target-now;if(diff<=0){el.innerHTML='<div class="cd-passed">&#127881; Event has passed!</div>';return}var d=Math.floor(diff/86400000);var h=Math.floor((diff%86400000)/3600000);var m=Math.floor((diff%3600000)/60000);var s=Math.floor((diff%60000)/1000);el.querySelector('.days').textContent=d;el.querySelector('.hours').textContent=h;el.querySelector('.mins').textContent=m;el.querySelector('.secs').textContent=s})}
updateCountdowns();setInterval(updateCountdowns,1000);
</script></body></html>"##);

    respond(200, buf_as_str(p), "text/html");
}
