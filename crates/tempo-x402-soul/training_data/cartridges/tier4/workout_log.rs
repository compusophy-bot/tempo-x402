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
        let exercise = find_json_str(body, "exercise").unwrap_or("");
        let sets = find_json_str(body, "sets").unwrap_or("");
        let reps = find_json_str(body, "reps").unwrap_or("");
        let weight = find_json_str(body, "weight").unwrap_or("");

        if exercise.len() > 0 {
            let existing = kv_read("workouts").unwrap_or("");
            let mut p = 0usize;
            p = buf_write(p, existing);
            p = buf_write(p, exercise);
            p = buf_write(p, "|");
            p = buf_write(p, sets);
            p = buf_write(p, "|");
            p = buf_write(p, reps);
            p = buf_write(p, "|");
            p = buf_write(p, weight);
            p = buf_write(p, "\n");
            kv_write("workouts", buf_as_str(p));
        }
    }

    let workouts = kv_read("workouts").unwrap_or("");

    let mut p = 0usize;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Workout Log</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#0d1117;color:#c9d1d9;font-family:'Segoe UI',sans-serif;min-height:100vh;padding:20px;display:flex;flex-direction:column;align-items:center}
h1{color:#58a6ff;margin:20px 0;font-size:2em}
.container{width:100%;max-width:650px}
.form{background:#161b22;padding:20px;border-radius:12px;margin-bottom:20px;border:1px solid #30363d}
.form h2{color:#58a6ff;margin-bottom:15px;font-size:1.2em}
.row{display:flex;gap:10px;margin-bottom:10px;flex-wrap:wrap}
.row input{flex:1;min-width:100px;padding:10px;border:1px solid #30363d;border-radius:8px;background:#0d1117;color:#c9d1d9;font-size:1em}
.row input:focus{outline:none;border-color:#58a6ff}
.form button{width:100%;padding:12px;background:#238636;color:#fff;border:none;border-radius:8px;font-size:1em;cursor:pointer;font-weight:bold}
.form button:hover{background:#2ea043}
.history h2{color:#58a6ff;margin-bottom:15px;font-size:1.2em}
.entry{background:#161b22;border:1px solid #30363d;border-radius:10px;padding:15px;margin-bottom:8px;display:flex;justify-content:space-between;align-items:center}
.entry .name{font-weight:bold;color:#f0f6fc;font-size:1.05em}
.entry .details{display:flex;gap:15px}
.badge{background:#30363d;padding:4px 10px;border-radius:6px;font-size:0.85em;color:#8b949e}
.badge span{color:#58a6ff;font-weight:bold}
.empty{text-align:center;color:#484f58;padding:40px;font-size:1.1em}
.stats{background:#161b22;border:1px solid #30363d;border-radius:12px;padding:15px;margin-bottom:20px;display:flex;justify-content:space-around}
.stat{text-align:center}
.stat .num{font-size:1.8em;color:#58a6ff;font-weight:bold}
.stat .label{font-size:0.85em;color:#8b949e}
</style></head><body>
<h1>&#127947; Workout Log</h1>
<div class="container">
"##);

    // Count total workouts
    let wb = workouts.as_bytes();
    let mut total = 0usize;
    let mut wpos = 0usize;
    while wpos < wb.len() {
        if wb[wpos] == b'\n' { total += 1; }
        wpos += 1;
    }

    p = buf_write(p, r##"<div class="stats"><div class="stat"><div class="num">"##);
    p = write_usize(p, total);
    p = buf_write(p, r##"</div><div class="label">Total Exercises</div></div></div>"##);

    p = buf_write(p, r##"<div class="form"><h2>Log Exercise</h2>
<div class="row">
<input type="text" id="exercise" placeholder="Exercise name">
<input type="number" id="sets" placeholder="Sets" min="1">
</div>
<div class="row">
<input type="number" id="reps" placeholder="Reps" min="1">
<input type="text" id="weight" placeholder="Weight (lbs/kg)">
</div>
<button onclick="addWorkout()">Log Workout</button>
</div>
<div class="history"><h2>History</h2>"##);

    // Render entries in reverse order
    if total == 0 {
        p = buf_write(p, r##"<div class="empty">No workouts logged yet. Start your fitness journey!</div>"##);
    } else {
        // Parse entries
        let mut starts: [usize; 128] = [0; 128];
        let mut ends: [usize; 128] = [0; 128];
        let mut cnt = 0usize;
        let mut lp = 0usize;
        while lp < wb.len() && cnt < 128 {
            starts[cnt] = lp;
            let mut le = lp;
            while le < wb.len() && wb[le] != b'\n' { le += 1; }
            ends[cnt] = le;
            if le > lp { cnt += 1; }
            lp = le + 1;
        }
        let mut ri = cnt;
        while ri > 0 {
            ri -= 1;
            let line = unsafe { core::str::from_utf8_unchecked(&wb[starts[ri]..ends[ri]]) };
            let lb = line.as_bytes();
            let mut seps: [usize; 3] = [0; 3];
            let mut sc = 0;
            let mut si = 0;
            while si < lb.len() && sc < 3 {
                if lb[si] == b'|' { seps[sc] = si; sc += 1; }
                si += 1;
            }
            if sc >= 3 {
                let name = unsafe { core::str::from_utf8_unchecked(&lb[..seps[0]]) };
                let sets = unsafe { core::str::from_utf8_unchecked(&lb[seps[0]+1..seps[1]]) };
                let reps = unsafe { core::str::from_utf8_unchecked(&lb[seps[1]+1..seps[2]]) };
                let wt = unsafe { core::str::from_utf8_unchecked(&lb[seps[2]+1..]) };
                p = buf_write(p, r##"<div class="entry"><div class="name">"##);
                p = buf_write(p, name);
                p = buf_write(p, r##"</div><div class="details"><div class="badge"><span>"##);
                p = buf_write(p, sets);
                p = buf_write(p, r##"</span> sets</div><div class="badge"><span>"##);
                p = buf_write(p, reps);
                p = buf_write(p, r##"</span> reps</div><div class="badge"><span>"##);
                p = buf_write(p, wt);
                p = buf_write(p, r##"</span></div></div></div>"##);
            }
        }
    }

    p = buf_write(p, r##"</div></div>
<script>
function addWorkout(){
  var e=document.getElementById('exercise').value;
  var s=document.getElementById('sets').value;
  var r=document.getElementById('reps').value;
  var w=document.getElementById('weight').value||'BW';
  if(!e||!s||!r)return alert('Please fill exercise, sets and reps');
  fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({exercise:e,sets:s,reps:r,weight:w})}).then(()=>location.reload());
}
</script></body></html>"##);

    respond(200, buf_as_str(p), "text/html");
}
