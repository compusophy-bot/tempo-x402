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

#[no_mangle]
pub extern "C" fn handle_request(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("");
        let task = find_json_str(body, "task").unwrap_or("");
        let minutes = find_json_str(body, "minutes").unwrap_or("0");

        if action == "log" && task.len() > 0 {
            let existing = kv_read("time_entries").unwrap_or("");
            let mut p = 0usize;
            p = buf_write(p, existing);
            p = buf_write(p, task);
            p = buf_write(p, "|");
            p = buf_write(p, minutes);
            p = buf_write(p, "\n");
            kv_write("time_entries", buf_as_str(p));
        } else if action == "clear" {
            kv_write("time_entries", "");
        }
    }

    let entries = kv_read("time_entries").unwrap_or("");

    let mut p = 0usize;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Time Tracker</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#1e1e2e;color:#cdd6f4;font-family:'Segoe UI',sans-serif;min-height:100vh;padding:20px;display:flex;flex-direction:column;align-items:center}
h1{color:#89b4fa;margin:20px 0;font-size:2em}
.container{width:100%;max-width:600px}
.timer-card{background:#313244;border-radius:16px;padding:30px;margin-bottom:20px;text-align:center;border:1px solid #45475a}
.timer-display{font-size:3.5em;font-weight:bold;color:#f5c2e7;font-family:monospace;margin:20px 0}
.task-input{width:100%;padding:14px;border:2px solid #45475a;border-radius:10px;background:#1e1e2e;color:#cdd6f4;font-size:1.1em;margin-bottom:15px;text-align:center}
.task-input:focus{outline:none;border-color:#89b4fa}
.btn-row{display:flex;gap:10px;justify-content:center}
.btn{padding:12px 30px;border:none;border-radius:10px;font-size:1em;cursor:pointer;font-weight:bold;transition:transform 0.1s}
.btn:hover{transform:scale(1.05)}
.btn-start{background:#a6e3a1;color:#1e1e2e}
.btn-stop{background:#f38ba8;color:#1e1e2e}
.btn-log{background:#89b4fa;color:#1e1e2e}
.log-form{background:#313244;border-radius:16px;padding:20px;margin-bottom:20px;border:1px solid #45475a}
.log-form h2{color:#89b4fa;margin-bottom:15px}
.log-row{display:flex;gap:10px}
.log-row input{flex:1;padding:10px;border:1px solid #45475a;border-radius:8px;background:#1e1e2e;color:#cdd6f4;font-size:1em}
.log-row input:focus{outline:none;border-color:#89b4fa}
.entries h2{color:#89b4fa;margin-bottom:15px}
.entry{background:#313244;border:1px solid #45475a;border-radius:10px;padding:12px 18px;margin-bottom:8px;display:flex;justify-content:space-between;align-items:center}
.entry .task-name{font-weight:bold;color:#f5e0dc}
.entry .mins{color:#a6e3a1;font-weight:bold;font-size:1.1em}
.total-bar{background:#313244;border:1px solid #45475a;border-radius:12px;padding:18px;margin-bottom:20px;display:flex;justify-content:space-between;align-items:center}
.total-bar .label{color:#bac2de;font-size:1.1em}
.total-bar .value{color:#f5c2e7;font-size:1.8em;font-weight:bold}
.empty{text-align:center;color:#6c7086;padding:30px}
</style></head><body>
<h1>&#9202; Time Tracker</h1>
<div class="container">
<div class="timer-card">
<input class="task-input" type="text" id="timerTask" placeholder="What are you working on?">
<div class="timer-display" id="display">00:00:00</div>
<div class="btn-row">
<button class="btn btn-start" id="startBtn" onclick="startTimer()">Start</button>
<button class="btn btn-stop" id="stopBtn" onclick="stopTimer()" style="display:none">Stop</button>
<button class="btn btn-log" id="logBtn" onclick="logTime()" style="display:none">Log Time</button>
</div>
</div>
<div class="log-form"><h2>Quick Log</h2>
<div class="log-row">
<input type="text" id="manualTask" placeholder="Task name">
<input type="number" id="manualMins" placeholder="Minutes" min="1">
<button class="btn btn-log" onclick="manualLog()">Add</button>
</div></div>
"##);

    // Calculate total and render entries
    let eb = entries.as_bytes();
    let mut total_mins = 0usize;
    let mut epos = 0usize;
    let mut count = 0usize;

    // First pass: count total
    while epos < eb.len() {
        let mut eend = epos;
        while eend < eb.len() && eb[eend] != b'\n' { eend += 1; }
        if eend > epos {
            let line = &eb[epos..eend];
            let mut sep = 0;
            let mut si = 0;
            while si < line.len() { if line[si] == b'|' { sep = si; break; } si += 1; }
            if sep > 0 {
                let mins_str = unsafe { core::str::from_utf8_unchecked(&line[sep+1..]) };
                total_mins += parse_usize(mins_str);
                count += 1;
            }
        }
        epos = eend + 1;
    }

    let hours = total_mins / 60;
    let mins = total_mins % 60;

    p = buf_write(p, r##"<div class="total-bar"><div class="label">Total Tracked</div><div class="value">"##);
    p = write_usize(p, hours);
    p = buf_write(p, "h ");
    p = write_usize(p, mins);
    p = buf_write(p, r##"m</div></div><div class="entries"><h2>Log Entries</h2>"##);

    if count == 0 {
        p = buf_write(p, r##"<div class="empty">No time entries yet. Start tracking!</div>"##);
    } else {
        epos = 0;
        while epos < eb.len() {
            let mut eend = epos;
            while eend < eb.len() && eb[eend] != b'\n' { eend += 1; }
            if eend > epos {
                let line = &eb[epos..eend];
                let mut sep = 0;
                let mut si = 0;
                while si < line.len() { if line[si] == b'|' { sep = si; break; } si += 1; }
                if sep > 0 {
                    let task = unsafe { core::str::from_utf8_unchecked(&line[..sep]) };
                    let mins_s = unsafe { core::str::from_utf8_unchecked(&line[sep+1..]) };
                    p = buf_write(p, r##"<div class="entry"><span class="task-name">"##);
                    p = buf_write(p, task);
                    p = buf_write(p, r##"</span><span class="mins">"##);
                    p = buf_write(p, mins_s);
                    p = buf_write(p, r##" min</span></div>"##);
                }
            }
            epos = eend + 1;
        }
    }

    p = buf_write(p, r##"</div></div>
<script>
var running=false,elapsed=0,interval=null;
function pad(n){return n<10?'0'+n:''+n}
function updateDisplay(){var h=Math.floor(elapsed/3600),m=Math.floor((elapsed%3600)/60),s=elapsed%60;document.getElementById('display').textContent=pad(h)+':'+pad(m)+':'+pad(s)}
function startTimer(){if(running)return;running=true;interval=setInterval(function(){elapsed++;updateDisplay()},1000);document.getElementById('startBtn').style.display='none';document.getElementById('stopBtn').style.display='inline-block'}
function stopTimer(){running=false;clearInterval(interval);document.getElementById('stopBtn').style.display='none';document.getElementById('startBtn').style.display='inline-block';document.getElementById('logBtn').style.display='inline-block'}
function logTime(){var t=document.getElementById('timerTask').value||'Untitled';var m=Math.ceil(elapsed/60);if(m<1)m=1;fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'log',task:t,minutes:String(m)})}).then(function(){location.reload()})}
function manualLog(){var t=document.getElementById('manualTask').value;var m=document.getElementById('manualMins').value;if(!t||!m)return alert('Fill in task and minutes');fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'log',task:t,minutes:m})}).then(function(){location.reload()})}
</script></body></html>"##);

    respond(200, buf_as_str(p), "text/html");
}

fn parse_usize(s: &str) -> usize {
    let mut n = 0usize;
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() { if b[i] >= b'0' && b[i] <= b'9' { n = n * 10 + (b[i] - b'0') as usize; } i += 1; }
    n
}
