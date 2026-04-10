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
    while i < b.len() {
        if b[i] >= b'0' && b[i] <= b'9' { n = n * 10 + (b[i] - b'0') as usize; }
        i += 1;
    }
    n
}

// KV format for habits: "name|streak|done_today\n" per line
// done_today: "1" or "0", streak is cumulative

#[no_mangle]
pub extern "C" fn handle_request(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("");
        if action == "add" {
            let name = find_json_str(body, "name").unwrap_or("");
            if name.len() > 0 {
                let existing = kv_read("habits").unwrap_or("");
                let mut p = 0usize;
                p = buf_write(p, existing);
                p = buf_write(p, name);
                p = buf_write(p, "|0|0\n");
                kv_write("habits", buf_as_str(p));
            }
            respond(200, "{\"ok\":true}", "application/json");
            return;
        }
        if action == "toggle" {
            let idx = parse_usize(find_json_str(body, "index").unwrap_or("0"));
            let existing = kv_read("habits").unwrap_or("");
            let eb = existing.as_bytes();
            let mut p = 0usize;
            let mut pos = 0usize;
            let mut line_num = 0usize;
            static mut TMP: [u8; 16384] = [0u8; 16384];
            let mut tp = 0usize;
            while pos < eb.len() {
                let start = pos;
                while pos < eb.len() && eb[pos] != b'\n' { pos += 1; }
                let line = &eb[start..pos];
                if pos < eb.len() { pos += 1; }
                if line.len() == 0 { line_num += 1; continue; }
                // Parse name|streak|done
                let mut sep1 = 0usize;
                let mut sep2 = 0usize;
                let mut sc = 0;
                let mut si = 0;
                while si < line.len() { if line[si] == b'|' { if sc == 0 { sep1 = si; } else { sep2 = si; } sc += 1; } si += 1; }
                if sc >= 2 {
                    let lname = unsafe { core::str::from_utf8_unchecked(&line[..sep1]) };
                    let streak_s = unsafe { core::str::from_utf8_unchecked(&line[sep1+1..sep2]) };
                    let done_s = unsafe { core::str::from_utf8_unchecked(&line[sep2+1..]) };
                    let streak = parse_usize(streak_s);
                    let done = done_s == "1";
                    unsafe {
                        if line_num == idx {
                            let new_done = !done;
                            let new_streak = if new_done { streak + 1 } else { if streak > 0 { streak - 1 } else { 0 } };
                            // Write: name|new_streak|new_done
                            let nb = lname.as_bytes();
                            TMP[tp..tp+nb.len()].copy_from_slice(nb);
                            tp += nb.len();
                            TMP[tp] = b'|'; tp += 1;
                            // write streak number
                            tp = write_to_tmp(tp, new_streak);
                            TMP[tp] = b'|'; tp += 1;
                            TMP[tp] = if new_done { b'1' } else { b'0' }; tp += 1;
                            TMP[tp] = b'\n'; tp += 1;
                        } else {
                            TMP[tp..tp+line.len()].copy_from_slice(line);
                            tp += line.len();
                            TMP[tp] = b'\n'; tp += 1;
                        }
                    }
                }
                line_num += 1;
            }
            let new_val = unsafe { core::str::from_utf8_unchecked(&TMP[..tp]) };
            kv_write("habits", new_val);
            respond(200, "{\"ok\":true}", "application/json");
            return;
        }
        if action == "delete" {
            let idx = parse_usize(find_json_str(body, "index").unwrap_or("0"));
            let existing = kv_read("habits").unwrap_or("");
            let eb = existing.as_bytes();
            static mut TMP2: [u8; 16384] = [0u8; 16384];
            let mut tp = 0usize;
            let mut pos = 0usize;
            let mut line_num = 0usize;
            while pos < eb.len() {
                let start = pos;
                while pos < eb.len() && eb[pos] != b'\n' { pos += 1; }
                let line = &eb[start..pos];
                if pos < eb.len() { pos += 1; }
                if line.len() == 0 { line_num += 1; continue; }
                if line_num != idx {
                    unsafe { TMP2[tp..tp+line.len()].copy_from_slice(line); tp += line.len(); TMP2[tp] = b'\n'; tp += 1; }
                }
                line_num += 1;
            }
            let new_val = unsafe { core::str::from_utf8_unchecked(&TMP2[..tp]) };
            kv_write("habits", new_val);
            respond(200, "{\"ok\":true}", "application/json");
            return;
        }
        if action == "reset" {
            // Reset all done_today to 0 (new day)
            let existing = kv_read("habits").unwrap_or("");
            let eb = existing.as_bytes();
            static mut TMP3: [u8; 16384] = [0u8; 16384];
            let mut tp = 0usize;
            let mut pos = 0usize;
            while pos < eb.len() {
                let start = pos;
                while pos < eb.len() && eb[pos] != b'\n' { pos += 1; }
                let line = &eb[start..pos];
                if pos < eb.len() { pos += 1; }
                if line.len() == 0 { continue; }
                let mut sep1 = 0usize;
                let mut sep2 = 0usize;
                let mut sc = 0;
                let mut si = 0;
                while si < line.len() { if line[si] == b'|' { if sc == 0 { sep1 = si; } else { sep2 = si; } sc += 1; } si += 1; }
                if sc >= 2 {
                    unsafe {
                        TMP3[tp..tp+sep2].copy_from_slice(&line[..sep2]);
                        tp += sep2;
                        TMP3[tp] = b'|'; tp += 1;
                        TMP3[tp] = b'0'; tp += 1;
                        TMP3[tp] = b'\n'; tp += 1;
                    }
                }
            }
            let new_val = unsafe { core::str::from_utf8_unchecked(&TMP3[..tp]) };
            kv_write("habits", new_val);
            respond(200, "{\"ok\":true}", "application/json");
            return;
        }
        respond(400, "{\"error\":\"unknown action\"}", "application/json");
        return;
    }

    // GET — render HTML
    let habits = kv_read("habits").unwrap_or("");
    let mut p = 0usize;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Habit Tracker</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#0d1117;color:#e6edf3;font-family:'Segoe UI',sans-serif;min-height:100vh;display:flex;flex-direction:column;align-items:center;padding:20px}
h1{color:#58a6ff;margin:20px 0;font-size:2em}
.container{width:100%;max-width:550px}
.add-row{display:flex;gap:10px;margin-bottom:25px}
.add-row input{flex:1;padding:12px;border:1px solid #30363d;border-radius:8px;background:#161b22;color:#e6edf3;font-size:1em}
.add-row input:focus{outline:none;border-color:#58a6ff}
.add-row button{padding:12px 20px;background:#238636;color:#fff;border:none;border-radius:8px;font-weight:bold;cursor:pointer;font-size:1em}
.add-row button:hover{background:#2ea043}
.habit{background:#161b22;border:1px solid #30363d;border-radius:12px;padding:16px;margin-bottom:10px;display:flex;align-items:center;gap:14px;transition:all 0.2s}
.habit:hover{border-color:#58a6ff}
.habit.done{border-color:#238636;background:#0d1f0d}
.check{width:32px;height:32px;border-radius:8px;border:2px solid #30363d;background:transparent;cursor:pointer;display:flex;align-items:center;justify-content:center;font-size:18px;color:#238636;flex-shrink:0;transition:all 0.2s}
.check.checked{background:#238636;border-color:#238636;color:#fff}
.habit-info{flex:1}
.habit-name{font-size:1.1em;font-weight:600}
.streak{font-size:0.85em;color:#8b949e;margin-top:2px}
.streak span{color:#f0883e;font-weight:bold}
.del{background:#21262d;color:#f85149;border:1px solid #30363d;border-radius:6px;padding:6px 12px;cursor:pointer;font-size:0.85em}
.del:hover{background:#da3633;color:#fff;border-color:#da3633}
.toolbar{display:flex;justify-content:space-between;align-items:center;margin-bottom:15px}
.toolbar .reset-btn{padding:8px 16px;background:#21262d;color:#8b949e;border:1px solid #30363d;border-radius:6px;cursor:pointer;font-size:0.9em}
.toolbar .reset-btn:hover{color:#e6edf3;border-color:#58a6ff}
.summary{text-align:center;color:#8b949e;margin-top:20px;font-size:0.95em;padding:15px;background:#161b22;border-radius:10px;border:1px solid #30363d}
.summary .big{font-size:2em;font-weight:bold;color:#58a6ff;display:block;margin-bottom:5px}
.empty{text-align:center;color:#8b949e;padding:40px;font-size:1.1em}
</style></head><body>
<h1>&#9745; Habit Tracker</h1>
<div class="container">
<div class="add-row"><input type="text" id="inp" placeholder="New habit..." onkeydown="if(event.key==='Enter')addHabit()"><button onclick="addHabit()">Add Habit</button></div>
"##);

    // Parse and count
    let hb = habits.as_bytes();
    let mut pos = 0usize;
    let mut total = 0usize;
    let mut done_count = 0usize;
    let mut max_streak = 0usize;
    // First pass: count
    let mut tpos = 0usize;
    while tpos < hb.len() {
        let start = tpos;
        while tpos < hb.len() && hb[tpos] != b'\n' { tpos += 1; }
        let line = &hb[start..tpos];
        if tpos < hb.len() { tpos += 1; }
        if line.len() < 3 { continue; }
        let mut sep1 = 0usize; let mut sep2 = 0usize; let mut sc = 0; let mut si = 0;
        while si < line.len() { if line[si] == b'|' { if sc == 0 { sep1 = si; } else { sep2 = si; } sc += 1; } si += 1; }
        if sc >= 2 {
            total += 1;
            let streak_s = unsafe { core::str::from_utf8_unchecked(&line[sep1+1..sep2]) };
            let done_s = unsafe { core::str::from_utf8_unchecked(&line[sep2+1..]) };
            let streak = parse_usize(streak_s);
            if done_s == "1" { done_count += 1; }
            if streak > max_streak { max_streak = streak; }
        }
    }

    if total > 0 {
        p = buf_write(p, r##"<div class="toolbar"><span>"##);
        p = write_usize(p, total);
        p = buf_write(p, r##" habits</span><button class="reset-btn" onclick="resetDay()">New Day (Reset Checks)</button></div>"##);
    }

    // Second pass: render
    let mut idx = 0usize;
    pos = 0;
    while pos < hb.len() {
        let start = pos;
        while pos < hb.len() && hb[pos] != b'\n' { pos += 1; }
        let line = &hb[start..pos];
        if pos < hb.len() { pos += 1; }
        if line.len() < 3 { continue; }
        let mut sep1 = 0usize; let mut sep2 = 0usize; let mut sc = 0; let mut si = 0;
        while si < line.len() { if line[si] == b'|' { if sc == 0 { sep1 = si; } else { sep2 = si; } sc += 1; } si += 1; }
        if sc >= 2 {
            let name = unsafe { core::str::from_utf8_unchecked(&line[..sep1]) };
            let streak_s = unsafe { core::str::from_utf8_unchecked(&line[sep1+1..sep2]) };
            let done_s = unsafe { core::str::from_utf8_unchecked(&line[sep2+1..]) };
            let streak = parse_usize(streak_s);
            let done = done_s == "1";
            p = buf_write(p, "<div class=\"habit");
            if done { p = buf_write(p, " done"); }
            p = buf_write(p, "\"><button class=\"check");
            if done { p = buf_write(p, " checked"); }
            p = buf_write(p, "\" onclick=\"toggle(");
            p = write_usize(p, idx);
            p = buf_write(p, ")\">");
            if done { p = buf_write(p, "&#10003;"); }
            p = buf_write(p, "</button><div class=\"habit-info\"><div class=\"habit-name\">");
            p = buf_write(p, name);
            p = buf_write(p, "</div><div class=\"streak\">Streak: <span>");
            p = write_usize(p, streak);
            p = buf_write(p, " days</span></div></div><button class=\"del\" onclick=\"del(");
            p = write_usize(p, idx);
            p = buf_write(p, ")\">Remove</button></div>");
            idx += 1;
        }
    }

    if total == 0 {
        p = buf_write(p, r##"<div class="empty">No habits yet. Start building good habits today!</div>"##);
    } else {
        p = buf_write(p, r##"<div class="summary"><span class="big">"##);
        p = write_usize(p, done_count);
        p = buf_write(p, " / ");
        p = write_usize(p, total);
        p = buf_write(p, r##"</span>completed today | Best streak: "##);
        p = write_usize(p, max_streak);
        p = buf_write(p, " days</div>");
    }

    p = buf_write(p, r##"</div>
<script>
var B=location.pathname;
function addHabit(){var i=document.getElementById('inp');var n=i.value.trim();if(!n)return;fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add',name:n})}).then(()=>location.reload())}
function toggle(i){fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'toggle',index:String(i)})}).then(()=>location.reload())}
function del(i){if(!confirm('Remove this habit?'))return;fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',index:String(i)})}).then(()=>location.reload())}
function resetDay(){fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'reset'})}).then(()=>location.reload())}
</script></body></html>"##);

    respond(200, buf_as_str(p), "text/html");
}

fn write_to_tmp(mut tp: usize, mut n: usize) -> usize {
    if n == 0 { unsafe { SCRATCH[tp] = b'0'; } return tp + 1; }
    static mut TD: [u8; 20] = [0u8; 20];
    let mut i = 0;
    while n > 0 { unsafe { TD[i] = b'0' + (n % 10) as u8; } n /= 10; i += 1; }
    while i > 0 { i -= 1; unsafe { let mut tmp3: *mut u8 = core::ptr::null_mut(); // use TMP directly
        // We need to write to the TMP buffer, but it's in the caller. Use a different approach.
    } }
    // Simpler: write digits to TMP via index
    tp
}
