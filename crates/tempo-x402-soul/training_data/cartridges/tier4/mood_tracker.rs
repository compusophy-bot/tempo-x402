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
        let mood = find_json_str(body, "mood").unwrap_or("");
        let note = find_json_str(body, "note").unwrap_or("");
        let day = find_json_str(body, "day").unwrap_or("0");

        if mood.len() > 0 {
            let existing = kv_read("moods").unwrap_or("");
            let mut p = 0usize;
            p = buf_write(p, existing);
            p = buf_write(p, day);
            p = buf_write(p, "|");
            p = buf_write(p, mood);
            p = buf_write(p, "|");
            p = buf_write(p, note);
            p = buf_write(p, "\n");
            kv_write("moods", buf_as_str(p));
        }
    }

    let moods = kv_read("moods").unwrap_or("");

    let mut p = 0usize;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Mood Tracker</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:linear-gradient(135deg,#667eea 0%,#764ba2 100%);color:#fff;font-family:'Segoe UI',sans-serif;min-height:100vh;padding:20px;display:flex;flex-direction:column;align-items:center}
h1{margin:20px 0;font-size:2.2em;text-shadow:0 2px 4px rgba(0,0,0,0.3)}
.container{width:100%;max-width:550px}
.mood-picker{background:rgba(255,255,255,0.15);backdrop-filter:blur(10px);border-radius:20px;padding:25px;margin-bottom:20px;text-align:center}
.mood-picker h2{margin-bottom:15px;font-size:1.1em;opacity:0.9}
.emojis{display:flex;justify-content:center;gap:12px;margin-bottom:15px;flex-wrap:wrap}
.emoji-btn{font-size:2.5em;background:rgba(255,255,255,0.1);border:3px solid transparent;border-radius:16px;padding:10px 14px;cursor:pointer;transition:all 0.2s}
.emoji-btn:hover{transform:scale(1.15);background:rgba(255,255,255,0.25)}
.emoji-btn.selected{border-color:#fff;background:rgba(255,255,255,0.3);transform:scale(1.1)}
.note-input{width:100%;padding:12px;border:2px solid rgba(255,255,255,0.3);border-radius:10px;background:rgba(255,255,255,0.1);color:#fff;font-size:1em;margin-bottom:12px}
.note-input::placeholder{color:rgba(255,255,255,0.5)}
.note-input:focus{outline:none;border-color:#fff}
.submit-btn{padding:12px 40px;background:#fff;color:#764ba2;border:none;border-radius:10px;font-size:1.05em;cursor:pointer;font-weight:bold}
.submit-btn:hover{transform:scale(1.05)}
.history{background:rgba(255,255,255,0.12);backdrop-filter:blur(10px);border-radius:20px;padding:20px}
.history h2{margin-bottom:15px;font-size:1.2em}
.mood-entry{background:rgba(255,255,255,0.1);border-radius:12px;padding:14px 18px;margin-bottom:8px;display:flex;align-items:center;gap:15px}
.mood-entry .mood-emoji{font-size:2em}
.mood-entry .mood-info{flex:1}
.mood-entry .mood-day{font-weight:bold;opacity:0.8;font-size:0.85em}
.mood-entry .mood-note{margin-top:4px;opacity:0.9}
.empty{text-align:center;padding:30px;opacity:0.7;font-size:1.1em}
.week-bar{display:flex;justify-content:space-around;margin-bottom:20px;background:rgba(255,255,255,0.1);border-radius:12px;padding:10px}
.week-day{text-align:center;font-size:0.85em;opacity:0.8}
.week-day .day-emoji{font-size:1.8em;display:block;margin-top:4px}
</style></head><body>
<h1>Mood Tracker</h1>
<div class="container">
<div class="mood-picker">
<h2>How are you feeling today?</h2>
<div class="emojis">
<button class="emoji-btn" data-mood="amazing" onclick="selectMood(this)">&#128525;</button>
<button class="emoji-btn" data-mood="good" onclick="selectMood(this)">&#128522;</button>
<button class="emoji-btn" data-mood="okay" onclick="selectMood(this)">&#128528;</button>
<button class="emoji-btn" data-mood="sad" onclick="selectMood(this)">&#128546;</button>
<button class="emoji-btn" data-mood="angry" onclick="selectMood(this)">&#128545;</button>
</div>
<input class="note-input" type="text" id="note" placeholder="Add a note (optional)...">
<button class="submit-btn" onclick="logMood()">Log Mood</button>
</div>
<div class="history"><h2>Mood History</h2>
"##);

    let mb = moods.as_bytes();
    let mut count = 0usize;
    let mut mpos = 0usize;

    // Collect entries for reverse display
    let mut starts: [usize; 64] = [0; 64];
    let mut ends: [usize; 64] = [0; 64];
    let mut cnt = 0usize;
    while mpos < mb.len() && cnt < 64 {
        starts[cnt] = mpos;
        let mut mend = mpos;
        while mend < mb.len() && mb[mend] != b'\n' { mend += 1; }
        ends[cnt] = mend;
        if mend > mpos { cnt += 1; }
        mpos = mend + 1;
    }

    if cnt == 0 {
        p = buf_write(p, r##"<div class="empty">No moods logged yet. How are you feeling?</div>"##);
    } else {
        let mut ri = cnt;
        while ri > 0 {
            ri -= 1;
            let line = &mb[starts[ri]..ends[ri]];
            let mut seps: [usize; 2] = [0; 2];
            let mut sc = 0;
            let mut si = 0;
            while si < line.len() && sc < 2 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
            if sc >= 2 {
                let day = unsafe { core::str::from_utf8_unchecked(&line[..seps[0]]) };
                let mood = unsafe { core::str::from_utf8_unchecked(&line[seps[0]+1..seps[1]]) };
                let note = unsafe { core::str::from_utf8_unchecked(&line[seps[1]+1..]) };
                let emoji = match mood {
                    "amazing" => "&#128525;",
                    "good" => "&#128522;",
                    "okay" => "&#128528;",
                    "sad" => "&#128546;",
                    "angry" => "&#128545;",
                    _ => "&#128528;",
                };
                p = buf_write(p, r##"<div class="mood-entry"><div class="mood-emoji">"##);
                p = buf_write(p, emoji);
                p = buf_write(p, r##"</div><div class="mood-info"><div class="mood-day">Entry "##);
                p = buf_write(p, day);
                p = buf_write(p, r##"</div>"##);
                if note.len() > 0 {
                    p = buf_write(p, r##"<div class="mood-note">"##);
                    p = buf_write(p, note);
                    p = buf_write(p, "</div>");
                }
                p = buf_write(p, "</div></div>");
            }
        }
    }

    p = buf_write(p, r##"</div></div>
<script>
var selectedMood='';
var entryCount="##);
    p = write_usize(p, cnt);
    p = buf_write(p, r##";
function selectMood(el){document.querySelectorAll('.emoji-btn').forEach(function(b){b.classList.remove('selected')});el.classList.add('selected');selectedMood=el.getAttribute('data-mood')}
function logMood(){if(!selectedMood)return alert('Please select a mood');var n=document.getElementById('note').value;fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({mood:selectedMood,note:n,day:String(entryCount+1)})}).then(function(){location.reload()})}
</script></body></html>"##);

    respond(200, buf_as_str(p), "text/html");
}
