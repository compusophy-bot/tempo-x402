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

static mut NBUF: [u8; 32] = [0u8; 32];
fn num_to_str(mut n: u32) -> &'static str {
    if n == 0 {
        unsafe { NBUF[0] = b'0'; }
        return unsafe { core::str::from_utf8_unchecked(&NBUF[..1]) };
    }
    let mut i = 31;
    while n > 0 {
        unsafe { NBUF[i] = b'0' + (n % 10) as u8; }
        n /= 10;
        i -= 1;
    }
    unsafe { core::str::from_utf8_unchecked(&NBUF[i + 1..32]) }
}

fn parse_u32(s: &str) -> u32 {
    let mut n: u32 = 0;
    for &b in s.as_bytes() {
        if b >= b'0' && b <= b'9' {
            n = n.wrapping_mul(10).wrapping_add((b - b'0') as u32);
        }
    }
    n
}

fn render_timer() {
    let total_sessions = parse_u32(kv_read("pm_total_sessions").unwrap_or("0"));
    let total_focus_min = parse_u32(kv_read("pm_total_focus").unwrap_or("0"));
    let today_sessions = parse_u32(kv_read("pm_today_sessions").unwrap_or("0"));
    let streak = parse_u32(kv_read("pm_streak").unwrap_or("0"));

    let mut p = 0;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Pomodoro Timer</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:'Segoe UI',sans-serif;background:#0f1117;color:#e0e0e0;min-height:100vh;display:flex;flex-direction:column;align-items:center}
.header{background:#1a1d23;width:100%;padding:20px;text-align:center;border-bottom:2px solid #e53e3e}
.header h1{color:#e53e3e;font-size:1.8em}
.container{max-width:700px;width:100%;padding:30px 20px}
.timer-circle{width:300px;height:300px;border-radius:50%;border:8px solid #2d3748;margin:30px auto;display:flex;align-items:center;justify-content:center;position:relative;transition:border-color 0.5s}
.timer-circle.work{border-color:#e53e3e}
.timer-circle.break{border-color:#48bb78}
.timer-circle.long-break{border-color:#63b3ed}
.timer-display{text-align:center}
.timer-display .time{font-size:4em;font-weight:700;font-variant-numeric:tabular-nums}
.timer-display .phase{font-size:1.1em;color:#a0aec0;margin-top:5px}
.progress-ring{position:absolute;top:-4px;left:-4px;width:308px;height:308px}
.progress-ring circle{fill:none;stroke-width:8;stroke-linecap:round;transform:rotate(-90deg);transform-origin:50% 50%;transition:stroke-dashoffset 1s linear}
.controls{display:flex;gap:15px;justify-content:center;margin:20px 0}
.btn{padding:12px 30px;border:none;border-radius:8px;font-size:1em;font-weight:600;cursor:pointer;transition:transform 0.1s}
.btn:active{transform:scale(0.95)}
.btn-start{background:#e53e3e;color:#fff}
.btn-start:hover{background:#c53030}
.btn-pause{background:#f6ad55;color:#1a1d23}
.btn-reset{background:#4a5568;color:#e0e0e0}
.btn-reset:hover{background:#718096}
.btn-skip{background:#2d3748;color:#e0e0e0}
.settings{display:flex;gap:15px;justify-content:center;margin:15px 0;flex-wrap:wrap}
.setting{background:#1a1d23;border-radius:8px;padding:10px 15px;text-align:center}
.setting label{display:block;color:#a0aec0;font-size:0.8em;margin-bottom:5px}
.setting input{background:#2d3748;color:#e0e0e0;border:1px solid #4a5568;border-radius:4px;padding:5px;width:60px;text-align:center;font-size:1em}
.stats{display:grid;grid-template-columns:repeat(4,1fr);gap:12px;margin-top:30px}
.stat{background:#1a1d23;border-radius:10px;padding:18px;text-align:center}
.stat .val{font-size:1.8em;font-weight:700;color:#e53e3e}
.stat .label{color:#a0aec0;font-size:0.8em;margin-top:4px}
.session-log{background:#1a1d23;border-radius:10px;padding:20px;margin-top:25px}
.session-log h3{color:#e53e3e;margin-bottom:12px}
.log-entry{display:flex;justify-content:space-between;padding:8px 12px;border-bottom:1px solid #2d3748;font-size:0.9em}
.log-entry:last-child{border-bottom:none}
.log-entry .type{font-weight:600}
.log-entry .type.work{color:#e53e3e}
.log-entry .type.break{color:#48bb78}
.task-input{width:100%;background:#2d3748;color:#e0e0e0;border:1px solid #4a5568;border-radius:8px;padding:10px 15px;margin:15px 0;font-family:inherit;font-size:1em;text-align:center}
.task-input:focus{border-color:#e53e3e;outline:none}
</style></head><body>
<div class="header"><h1>Pomodoro Timer</h1></div>
<div class="container">
<input type="text" class="task-input" id="task-name" placeholder="What are you working on?">
<div class="timer-circle work" id="timer-circle">
<svg class="progress-ring"><circle id="progress-circle" cx="154" cy="154" r="146" stroke="#e53e3e" stroke-dasharray="917" stroke-dashoffset="0"/></svg>
<div class="timer-display">
<div class="time" id="timer">25:00</div>
<div class="phase" id="phase">Focus Time</div>
</div></div>
<div class="controls">
<button class="btn btn-start" id="start-btn" onclick="toggleTimer()">Start</button>
<button class="btn btn-skip" onclick="skipPhase()">Skip</button>
<button class="btn btn-reset" onclick="resetTimer()">Reset</button>
</div>
<div class="settings">
<div class="setting"><label>Focus (min)</label><input type="number" id="work-min" value="25" min="1" max="60" onchange="updateSettings()"></div>
<div class="setting"><label>Short Break</label><input type="number" id="break-min" value="5" min="1" max="30" onchange="updateSettings()"></div>
<div class="setting"><label>Long Break</label><input type="number" id="long-min" value="15" min="5" max="60" onchange="updateSettings()"></div>
<div class="setting"><label>Until Long</label><input type="number" id="cycle-count" value="4" min="2" max="8" onchange="updateSettings()"></div>
</div>
<div class="stats">
<div class="stat"><div class="val" id="s-today">"##);
    p = buf_write(p, num_to_str(today_sessions));
    p = buf_write(p, r##"</div><div class="label">Today</div></div>
<div class="stat"><div class="val" id="s-total">"##);
    p = buf_write(p, num_to_str(total_sessions));
    p = buf_write(p, r##"</div><div class="label">All Time</div></div>
<div class="stat"><div class="val" id="s-focus" style="color:#48bb78">"##);
    p = buf_write(p, num_to_str(total_focus_min));
    p = buf_write(p, r##"</div><div class="label">Focus Min</div></div>
<div class="stat"><div class="val" id="s-streak" style="color:#f6ad55">"##);
    p = buf_write(p, num_to_str(streak));
    p = buf_write(p, r##"</div><div class="label">Streak</div></div>
</div>
<div class="session-log"><h3>Session Log</h3><div id="log">"##);

    // Render session log
    let log_count = parse_u32(kv_read("pm_log_count").unwrap_or("0"));
    let log_start = if log_count > 10 { log_count - 10 } else { 0 };
    let mut li = log_count;
    while li > log_start {
        li -= 1;
        let mut lkbuf = [0u8; 24];
        let prefix = b"pm_log_";
        lkbuf[..prefix.len()].copy_from_slice(prefix);
        let ns = num_to_str(li);
        let nb = ns.as_bytes();
        lkbuf[prefix.len()..prefix.len()+nb.len()].copy_from_slice(nb);
        let lkey = unsafe { core::str::from_utf8_unchecked(&lkbuf[..prefix.len()+nb.len()]) };
        if let Some(data) = kv_read(lkey) {
            // data = "type|duration|task"
            let db = data.as_bytes();
            let mut pipe1 = 0;
            let mut pipe2 = 0;
            let mut fi = 0;
            let mut pipes = 0;
            while fi < db.len() {
                if db[fi] == b'|' {
                    pipes += 1;
                    if pipes == 1 { pipe1 = fi; }
                    else if pipes == 2 { pipe2 = fi; break; }
                }
                fi += 1;
            }
            if pipe2 == 0 { pipe2 = db.len(); }
            let stype = unsafe { core::str::from_utf8_unchecked(&db[..pipe1]) };
            let dur = if pipe2 > pipe1 + 1 { unsafe { core::str::from_utf8_unchecked(&db[pipe1+1..pipe2]) } } else { "0" };
            let task = if pipe2 < db.len() { unsafe { core::str::from_utf8_unchecked(&db[pipe2+1..]) } } else { "" };
            p = buf_write(p, r##"<div class="log-entry"><span class="type "##);
            p = buf_write(p, stype);
            p = buf_write(p, r##"">"##);
            p = buf_write(p, stype);
            p = buf_write(p, r##"</span><span>"##);
            if !task.is_empty() {
                p = buf_write(p, task);
                p = buf_write(p, " &middot; ");
            }
            p = buf_write(p, dur);
            p = buf_write(p, r##" min</span></div>"##);
        }
    }
    if log_count == 0 {
        p = buf_write(p, r##"<div class="log-entry" style="color:#a0aec0">No sessions yet. Start your first pomodoro!</div>"##);
    }

    p = buf_write(p, r##"</div></div></div>
<script>
var workMin=25,breakMin=5,longMin=15,cyclesUntilLong=4;
var timeLeft=workMin*60,running=false,interval=null;
var phase='work',completedCycles=0;
var circumference=917;
function updateSettings(){
  workMin=parseInt(document.getElementById('work-min').value)||25;
  breakMin=parseInt(document.getElementById('break-min').value)||5;
  longMin=parseInt(document.getElementById('long-min').value)||15;
  cyclesUntilLong=parseInt(document.getElementById('cycle-count').value)||4;
  if(!running)resetTimer();
}
function formatTime(s){var m=Math.floor(s/60);var sec=s%60;return(m<10?'0':'')+m+':'+(sec<10?'0':'')+sec;}
function updateDisplay(){
  document.getElementById('timer').textContent=formatTime(timeLeft);
  var total=phase==='work'?workMin*60:phase==='break'?breakMin*60:longMin*60;
  var pct=(total-timeLeft)/total;
  document.getElementById('progress-circle').style.strokeDashoffset=circumference*(1-pct);
}
function toggleTimer(){
  if(running){running=false;clearInterval(interval);document.getElementById('start-btn').textContent='Resume';document.getElementById('start-btn').className='btn btn-start';return;}
  running=true;document.getElementById('start-btn').textContent='Pause';document.getElementById('start-btn').className='btn btn-pause';
  interval=setInterval(function(){
    timeLeft--;updateDisplay();
    if(timeLeft<=0){clearInterval(interval);running=false;completePhase();}
  },1000);
}
function completePhase(){
  var dur=phase==='work'?workMin:phase==='break'?breakMin:longMin;
  var task=document.getElementById('task-name').value;
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'complete',phase:phase,duration:''+dur,task:task})})
  .then(function(){
    if(phase==='work'){
      completedCycles++;
      if(completedCycles>=cyclesUntilLong){phase='long-break';completedCycles=0;}
      else{phase='break';}
    }else{phase='work';}
    setPhase();
  });
}
function skipPhase(){if(running){clearInterval(interval);running=false;}completePhase();}
function resetTimer(){
  if(running){clearInterval(interval);running=false;}
  phase='work';completedCycles=0;setPhase();
  document.getElementById('start-btn').textContent='Start';document.getElementById('start-btn').className='btn btn-start';
}
function setPhase(){
  var circle=document.getElementById('timer-circle');
  var pCircle=document.getElementById('progress-circle');
  circle.className='timer-circle '+phase;
  if(phase==='work'){timeLeft=workMin*60;pCircle.setAttribute('stroke','#e53e3e');document.getElementById('phase').textContent='Focus Time';}
  else if(phase==='break'){timeLeft=breakMin*60;pCircle.setAttribute('stroke','#48bb78');document.getElementById('phase').textContent='Short Break';}
  else{timeLeft=longMin*60;pCircle.setAttribute('stroke','#63b3ed');document.getElementById('phase').textContent='Long Break';}
  document.getElementById('start-btn').textContent='Start';document.getElementById('start-btn').className='btn btn-start';
  updateDisplay();
}
updateDisplay();
</script></body></html>"##);
    respond(200, buf_as_str(p), "text/html");
}

#[no_mangle]
pub extern "C" fn x402_handle(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    host_log(1, "Pomodoro timer request");

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("");
        if action == "complete" {
            let phase = find_json_str(body, "phase").unwrap_or("work");
            let duration = find_json_str(body, "duration").unwrap_or("25");
            let task = find_json_str(body, "task").unwrap_or("");

            // Log session
            let log_count = parse_u32(kv_read("pm_log_count").unwrap_or("0"));
            let mut lkbuf = [0u8; 24];
            let prefix = b"pm_log_";
            lkbuf[..prefix.len()].copy_from_slice(prefix);
            let cs = num_to_str(log_count);
            let cb = cs.as_bytes();
            lkbuf[prefix.len()..prefix.len()+cb.len()].copy_from_slice(cb);
            let lkey = unsafe { core::str::from_utf8_unchecked(&lkbuf[..prefix.len()+cb.len()]) };
            // "type|duration|task"
            let mut val = [0u8; 256];
            let pb = phase.as_bytes();
            val[..pb.len()].copy_from_slice(pb);
            val[pb.len()] = b'|';
            let db = duration.as_bytes();
            val[pb.len()+1..pb.len()+1+db.len()].copy_from_slice(db);
            val[pb.len()+1+db.len()] = b'|';
            let tb = task.as_bytes();
            let tlen = tb.len().min(val.len() - pb.len() - 2 - db.len());
            val[pb.len()+2+db.len()..pb.len()+2+db.len()+tlen].copy_from_slice(&tb[..tlen]);
            let vlen = pb.len()+2+db.len()+tlen;
            let vstr = unsafe { core::str::from_utf8_unchecked(&val[..vlen]) };
            kv_write(lkey, vstr);
            kv_write("pm_log_count", num_to_str(log_count + 1));

            // Update stats
            if phase == "work" {
                let total = parse_u32(kv_read("pm_total_sessions").unwrap_or("0")) + 1;
                kv_write("pm_total_sessions", num_to_str(total));
                let focus = parse_u32(kv_read("pm_total_focus").unwrap_or("0")) + parse_u32(duration);
                kv_write("pm_total_focus", num_to_str(focus));
                let today = parse_u32(kv_read("pm_today_sessions").unwrap_or("0")) + 1;
                kv_write("pm_today_sessions", num_to_str(today));
                let streak = parse_u32(kv_read("pm_streak").unwrap_or("0")) + 1;
                kv_write("pm_streak", num_to_str(streak));
            }
            respond(200, r##"{"ok":true}"##, "application/json");
        } else {
            respond(400, r##"{"error":"unknown action"}"##, "application/json");
        }
        return;
    }

    render_timer();
}
