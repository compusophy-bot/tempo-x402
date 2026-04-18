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
        let idx_str = find_json_str(body, "index").unwrap_or("0");

        if action == "add" && name.len() > 0 {
            let existing = kv_read("habits").unwrap_or("");
            let mut bp = 0usize;
            bp = buf_write(bp, existing);
            bp = buf_write(bp, name);
            bp = buf_write(bp, "|0|0\n");
            kv_write("habits", buf_as_str(bp));
        } else if action == "toggle" {
            let target = parse_usize(idx_str);
            let existing = kv_read("habits").unwrap_or("");
            let eb = existing.as_bytes();
            static mut UPD: [u8; 16384] = [0u8; 16384];
            let mut np = 0usize;
            let mut epos = 0;
            let mut count = 0usize;
            while epos < eb.len() {
                let mut eend = epos;
                while eend < eb.len() && eb[eend] != b'\n' { eend += 1; }
                if eend > epos {
                    let line = &eb[epos..eend];
                    if count == target {
                        let mut seps: [usize; 2] = [0; 2];
                        let mut sc = 0;
                        let mut si = 0;
                        while si < line.len() && sc < 2 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
                        if sc >= 2 {
                            let hname = &line[..seps[0]];
                            let streak = parse_usize(unsafe { core::str::from_utf8_unchecked(&line[seps[0]+1..seps[1]]) });
                            let done = parse_usize(unsafe { core::str::from_utf8_unchecked(&line[seps[1]+1..]) });
                            unsafe { UPD[np..np+hname.len()].copy_from_slice(hname); np += hname.len(); UPD[np] = b'|'; np += 1; }
                            if done == 0 {
                                let new_streak = streak + 1;
                                let mut tmp = [0u8; 20];
                                let mut ti = 0;
                                let mut ns = new_streak;
                                if ns == 0 { tmp[0] = b'0'; ti = 1; } else {
                                    while ns > 0 { tmp[ti] = b'0' + (ns % 10) as u8; ns /= 10; ti += 1; }
                                }
                                while ti > 0 { ti -= 1; unsafe { UPD[np] = tmp[ti]; np += 1; } }
                                unsafe { UPD[np] = b'|'; np += 1; UPD[np] = b'1'; np += 1; UPD[np] = b'\n'; np += 1; }
                            } else {
                                let new_streak = if streak > 0 { streak - 1 } else { 0 };
                                let mut tmp = [0u8; 20];
                                let mut ti = 0;
                                let mut ns = new_streak;
                                if ns == 0 { tmp[0] = b'0'; ti = 1; } else {
                                    while ns > 0 { tmp[ti] = b'0' + (ns % 10) as u8; ns /= 10; ti += 1; }
                                }
                                while ti > 0 { ti -= 1; unsafe { UPD[np] = tmp[ti]; np += 1; } }
                                unsafe { UPD[np] = b'|'; np += 1; UPD[np] = b'0'; np += 1; UPD[np] = b'\n'; np += 1; }
                            }
                        }
                    } else {
                        unsafe { UPD[np..np+line.len()].copy_from_slice(line); np += line.len(); UPD[np] = b'\n'; np += 1; }
                    }
                    count += 1;
                }
                epos = eend + 1;
            }
            kv_write("habits", unsafe { core::str::from_utf8_unchecked(&UPD[..np]) });
        } else if action == "delete" {
            let target = parse_usize(idx_str);
            let existing = kv_read("habits").unwrap_or("");
            let eb = existing.as_bytes();
            static mut DEL: [u8; 16384] = [0u8; 16384];
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
            kv_write("habits", unsafe { core::str::from_utf8_unchecked(&DEL[..np]) });
        } else if action == "reset" {
            let existing = kv_read("habits").unwrap_or("");
            let eb = existing.as_bytes();
            static mut RST: [u8; 16384] = [0u8; 16384];
            let mut np = 0usize;
            let mut epos = 0;
            while epos < eb.len() {
                let mut eend = epos;
                while eend < eb.len() && eb[eend] != b'\n' { eend += 1; }
                if eend > epos {
                    let line = &eb[epos..eend];
                    let mut seps: [usize; 2] = [0; 2];
                    let mut sc = 0;
                    let mut si = 0;
                    while si < line.len() && sc < 2 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
                    if sc >= 2 {
                        let prefix = &line[..seps[1]+1];
                        unsafe { RST[np..np+prefix.len()].copy_from_slice(prefix); np += prefix.len(); RST[np] = b'0'; np += 1; RST[np] = b'\n'; np += 1; }
                    }
                }
                epos = eend + 1;
            }
            kv_write("habits", unsafe { core::str::from_utf8_unchecked(&RST[..np]) });
        }
    }

    let habits = kv_read("habits").unwrap_or("");

    let mut p = 0usize;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Habit Tracker</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#fefce8;color:#422006;font-family:'Segoe UI',sans-serif;min-height:100vh;padding:20px;display:flex;flex-direction:column;align-items:center}
h1{color:#ca8a04;margin:20px 0;font-size:2.2em}
.container{width:100%;max-width:550px}
.add-form{background:#fff;border-radius:16px;padding:20px;margin-bottom:20px;box-shadow:0 4px 12px rgba(0,0,0,0.08);border:1px solid #fef08a}
.add-form h2{color:#ca8a04;margin-bottom:12px}
.form-row{display:flex;gap:10px}
.form-row input{flex:1;padding:12px;border:2px solid #fef08a;border-radius:10px;background:#fffbeb;color:#422006;font-size:1em}
.form-row input:focus{outline:none;border-color:#ca8a04}
.add-btn{padding:12px 24px;background:#ca8a04;color:#fff;border:none;border-radius:10px;cursor:pointer;font-weight:bold;font-size:1em}
.add-btn:hover{background:#a16207}
.progress{background:#fff;border-radius:16px;padding:20px;margin-bottom:20px;box-shadow:0 4px 12px rgba(0,0,0,0.08);text-align:center;border:1px solid #fef08a}
.progress-bar{height:12px;background:#fef08a;border-radius:6px;overflow:hidden;margin:10px 0}
.progress-fill{height:100%;background:linear-gradient(90deg,#eab308,#ca8a04);border-radius:6px;transition:width 0.5s}
.progress-text{color:#854d0e;font-size:0.95em}
.habit{background:#fff;border-radius:14px;padding:18px;margin-bottom:10px;display:flex;align-items:center;gap:15px;box-shadow:0 2px 8px rgba(0,0,0,0.06);border:1px solid #fef08a;transition:all 0.2s}
.habit.done{background:#f0fdf4;border-color:#86efac}
.check{width:40px;height:40px;border-radius:50%;border:3px solid #d4d4d8;display:flex;align-items:center;justify-content:center;cursor:pointer;font-size:1.4em;transition:all 0.2s}
.habit.done .check{border-color:#22c55e;background:#22c55e;color:#fff}
.habit .info{flex:1}
.habit .name{font-weight:bold;font-size:1.1em}
.habit .streak{display:flex;align-items:center;gap:4px;color:#ca8a04;font-size:0.9em;margin-top:2px}
.streak-fire{font-size:1.1em}
.habit .del{background:transparent;border:none;color:#d4d4d8;cursor:pointer;font-size:1.2em;padding:5px}
.habit .del:hover{color:#ef4444}
.reset-btn{display:block;width:100%;padding:12px;background:#fef08a;color:#854d0e;border:none;border-radius:10px;cursor:pointer;font-weight:bold;font-size:0.95em;margin-top:10px}
.reset-btn:hover{background:#fde047}
.empty{text-align:center;color:#a16207;padding:40px;font-size:1.1em}
</style></head><body>
<h1>&#127775; Habit Tracker</h1>
<div class="container">
<div class="add-form"><h2>New Habit</h2><div class="form-row">
<input type="text" id="habitName" placeholder="e.g. Exercise 30 minutes">
<button class="add-btn" onclick="addHabit()">Add</button></div></div>
"##);

    let hb = habits.as_bytes();
    let mut hpos = 0;
    let mut total_habits = 0usize;
    let mut done_today = 0usize;

    while hpos < hb.len() {
        let mut hend = hpos;
        while hend < hb.len() && hb[hend] != b'\n' { hend += 1; }
        if hend > hpos {
            let line = &hb[hpos..hend];
            let mut seps: [usize; 2] = [0; 2];
            let mut sc = 0;
            let mut si = 0;
            while si < line.len() && sc < 2 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
            if sc >= 2 {
                total_habits += 1;
                let done = parse_usize(unsafe { core::str::from_utf8_unchecked(&line[seps[1]+1..]) });
                if done == 1 { done_today += 1; }
            }
        }
        hpos = hend + 1;
    }

    if total_habits > 0 {
        let pct = (done_today * 100) / total_habits;
        p = buf_write(p, r##"<div class="progress"><div class="progress-text">"##);
        p = write_usize(p, done_today);
        p = buf_write(p, " of ");
        p = write_usize(p, total_habits);
        p = buf_write(p, r##" habits completed today</div><div class="progress-bar"><div class="progress-fill" style="width:"##);
        p = write_usize(p, pct);
        p = buf_write(p, r##"%"></div></div></div>"##);
    }

    hpos = 0;
    let mut idx = 0usize;
    if total_habits == 0 {
        p = buf_write(p, r##"<div class="empty">No habits yet. Start building good habits today!</div>"##);
    }
    while hpos < hb.len() {
        let mut hend = hpos;
        while hend < hb.len() && hb[hend] != b'\n' { hend += 1; }
        if hend > hpos {
            let line = &hb[hpos..hend];
            let mut seps: [usize; 2] = [0; 2];
            let mut sc = 0;
            let mut si = 0;
            while si < line.len() && sc < 2 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
            if sc >= 2 {
                let hname = unsafe { core::str::from_utf8_unchecked(&line[..seps[0]]) };
                let streak = parse_usize(unsafe { core::str::from_utf8_unchecked(&line[seps[0]+1..seps[1]]) });
                let done = parse_usize(unsafe { core::str::from_utf8_unchecked(&line[seps[1]+1..]) });
                let is_done = done == 1;

                p = buf_write(p, r##"<div class="habit "##);
                if is_done { p = buf_write(p, "done"); }
                p = buf_write(p, r##""><div class="check" onclick="toggleHabit("##);
                p = write_usize(p, idx);
                p = buf_write(p, r##")">"##);
                if is_done { p = buf_write(p, "&#10003;"); }
                p = buf_write(p, r##"</div><div class="info"><div class="name">"##);
                p = buf_write(p, hname);
                p = buf_write(p, r##"</div><div class="streak"><span class="streak-fire">&#128293;</span> "##);
                p = write_usize(p, streak);
                p = buf_write(p, r##" day streak</div></div><button class="del" onclick="delHabit("##);
                p = write_usize(p, idx);
                p = buf_write(p, r##")">&#128465;</button></div>"##);
                idx += 1;
            }
        }
        hpos = hend + 1;
    }

    if total_habits > 0 {
        p = buf_write(p, r##"<button class="reset-btn" onclick="resetDay()">&#128260; Reset Day (mark all incomplete)</button>"##);
    }

    p = buf_write(p, r##"</div>
<script>
function addHabit(){var n=document.getElementById('habitName').value;if(!n)return alert('Enter a habit name');fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add',name:n})}).then(function(){location.reload()})}
function toggleHabit(i){fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'toggle',index:String(i)})}).then(function(){location.reload()})}
function delHabit(i){if(!confirm('Delete this habit?'))return;fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',index:String(i)})}).then(function(){location.reload()})}
function resetDay(){if(!confirm('Reset all habits for today?'))return;fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'reset'})}).then(function(){location.reload()})}
</script></body></html>"##);

    respond(200, buf_as_str(p), "text/html");
}
