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

static mut TMP: [u8; 16384] = [0u8; 16384];
fn tmp_write(pos: usize, s: &str) -> usize {
    let b = s.as_bytes();
    let end = (pos + b.len()).min(unsafe { TMP.len() });
    unsafe { TMP[pos..end].copy_from_slice(&b[..end - pos]); }
    end
}
fn tmp_as_str(len: usize) -> &'static str {
    unsafe { core::str::from_utf8_unchecked(&TMP[..len]) }
}

// KV "history": "password|label\n" per line (last 20 kept)
// KV "seed": LCG seed for pseudo-randomness

static mut RNG_STATE: u64 = 123456789;

fn next_rng() -> u64 {
    unsafe {
        RNG_STATE = RNG_STATE.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        RNG_STATE
    }
}

fn seed_rng_from_kv() {
    let seed_s = kv_read("seed").unwrap_or("0");
    let seed = parse_usize(seed_s) as u64;
    unsafe { RNG_STATE = if seed == 0 { 987654321 } else { seed }; }
}

fn save_rng_seed() {
    let mut tp = 0usize;
    let s = unsafe { RNG_STATE };
    // Store lower 32 bits as string
    let n = (s & 0xFFFFFFFF) as usize;
    static mut SD: [u8; 20] = [0u8; 20];
    let mut nn = n;
    if nn == 0 { tp = tmp_write(tp, "1"); }
    else {
        let mut i = 0;
        while nn > 0 { unsafe { SD[i] = b'0' + (nn % 10) as u8; } nn /= 10; i += 1; }
        while i > 0 { i -= 1; let d = unsafe { SD[i] }; let ss = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(&d, 1)) }; tp = tmp_write(tp, ss); }
    }
    kv_write("seed", tmp_as_str(tp));
}

// POST: {"action":"generate","length":"16","upper":"1","lower":"1","digits":"1","symbols":"1","label":"MyBank"}
// POST: {"action":"clear"} — clear history

#[no_mangle]
pub extern "C" fn handle_request(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("");
        if action == "generate" {
            seed_rng_from_kv();
            // Advance RNG with body length as additional entropy
            let mut extra = body.len() as u64;
            while extra > 0 { next_rng(); extra -= 1; }

            let length = parse_usize(find_json_str(body, "length").unwrap_or("16"));
            let len = if length < 4 { 4 } else if length > 64 { 64 } else { length };
            let use_upper = find_json_str(body, "upper").unwrap_or("1") == "1";
            let use_lower = find_json_str(body, "lower").unwrap_or("1") == "1";
            let use_digits = find_json_str(body, "digits").unwrap_or("1") == "1";
            let use_symbols = find_json_str(body, "symbols").unwrap_or("1") == "1";
            let label = find_json_str(body, "label").unwrap_or("Untitled");

            // Build charset
            static mut CHARSET: [u8; 94] = [0u8; 94];
            let mut clen = 0usize;
            if use_lower {
                let low = b"abcdefghijklmnopqrstuvwxyz";
                let mut li = 0;
                while li < 26 { unsafe { CHARSET[clen] = low[li]; } clen += 1; li += 1; }
            }
            if use_upper {
                let up = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
                let mut li = 0;
                while li < 26 { unsafe { CHARSET[clen] = up[li]; } clen += 1; li += 1; }
            }
            if use_digits {
                let dig = b"0123456789";
                let mut li = 0;
                while li < 10 { unsafe { CHARSET[clen] = dig[li]; } clen += 1; li += 1; }
            }
            if use_symbols {
                let sym = b"!@#$%^&*-_=+?";
                let mut li = 0;
                while li < 14 { unsafe { CHARSET[clen] = sym[li]; } clen += 1; li += 1; }
            }
            if clen == 0 {
                // Fallback to lowercase
                let low = b"abcdefghijklmnopqrstuvwxyz";
                let mut li = 0;
                while li < 26 { unsafe { CHARSET[clen] = low[li]; } clen += 1; li += 1; }
            }

            // Generate password
            static mut PASS: [u8; 64] = [0u8; 64];
            let mut pi = 0;
            while pi < len {
                let r = next_rng();
                let idx = ((r >> 16) as usize) % clen;
                unsafe { PASS[pi] = CHARSET[idx]; }
                pi += 1;
            }
            let password = unsafe { core::str::from_utf8_unchecked(&PASS[..len]) };

            save_rng_seed();

            // Append to history (keep last 20)
            let existing = kv_read("history").unwrap_or("");
            let mut tp = 0usize;
            // Count existing lines
            let eb = existing.as_bytes();
            let mut line_count = 0usize;
            let mut epos = 0usize;
            while epos < eb.len() {
                while epos < eb.len() && eb[epos] != b'\n' { epos += 1; }
                if epos > 0 { line_count += 1; }
                if epos < eb.len() { epos += 1; }
            }
            // If >= 20, skip oldest lines
            let skip = if line_count >= 20 { line_count - 19 } else { 0 };
            epos = 0;
            let mut skipped = 0usize;
            while epos < eb.len() {
                let start = epos;
                while epos < eb.len() && eb[epos] != b'\n' { epos += 1; }
                if skipped >= skip {
                    let line = unsafe { core::str::from_utf8_unchecked(&eb[start..epos]) };
                    tp = tmp_write(tp, line);
                    tp = tmp_write(tp, "\n");
                }
                skipped += 1;
                if epos < eb.len() { epos += 1; }
            }
            tp = tmp_write(tp, password);
            tp = tmp_write(tp, "|");
            tp = tmp_write(tp, label);
            tp = tmp_write(tp, "\n");
            kv_write("history", tmp_as_str(tp));

            respond(200, "{\"ok\":true}", "application/json");
            return;
        }
        if action == "clear" {
            kv_write("history", "");
            respond(200, "{\"ok\":true}", "application/json");
            return;
        }
        respond(400, "{\"error\":\"unknown action\"}", "application/json");
        return;
    }

    // GET — render
    let history = kv_read("history").unwrap_or("");
    let mut p = 0usize;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Password Generator</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#0f0f23;color:#ccc;font-family:'Segoe UI',sans-serif;min-height:100vh;display:flex;flex-direction:column;align-items:center;padding:20px}
h1{color:#00cc00;margin:20px 0;font-size:2em;font-family:monospace}
.container{width:100%;max-width:550px}
.gen-box{background:#1a1a3e;border:1px solid #333;border-radius:14px;padding:25px;margin-bottom:20px}
.gen-box h2{color:#00cc00;font-size:1.1em;margin-bottom:15px}
.row{display:flex;align-items:center;gap:12px;margin-bottom:12px}
.row label{color:#aaa;font-size:0.95em;min-width:80px}
.row input[type=range]{flex:1;accent-color:#00cc00}
.row input[type=text]{flex:1;padding:10px;background:#0f0f23;border:1px solid #333;border-radius:6px;color:#ccc;font-size:1em}
.row input[type=text]:focus{outline:none;border-color:#00cc00}
.len-val{color:#00cc00;font-weight:bold;min-width:30px;text-align:center}
.toggle-row{display:flex;gap:10px;margin-bottom:15px;flex-wrap:wrap}
.toggle{padding:8px 16px;border:1px solid #333;border-radius:8px;background:#0f0f23;color:#aaa;cursor:pointer;font-size:0.9em;transition:all 0.2s}
.toggle.active{border-color:#00cc00;color:#00cc00;background:#0a2a0a}
.gen-btn{width:100%;padding:14px;background:#00cc00;color:#0f0f23;border:none;border-radius:10px;font-size:1.1em;font-weight:bold;cursor:pointer}
.gen-btn:hover{background:#00ff00}
.history{background:#1a1a3e;border:1px solid #333;border-radius:14px;padding:20px}
.history h2{color:#aaa;font-size:1em;margin-bottom:12px;display:flex;justify-content:space-between;align-items:center}
.clear-btn{padding:4px 12px;background:#333;color:#888;border:none;border-radius:6px;cursor:pointer;font-size:0.85em}
.clear-btn:hover{color:#f85149}
.pw-item{background:#0f0f23;border:1px solid #222;border-radius:8px;padding:12px;margin-bottom:8px;display:flex;justify-content:space-between;align-items:center}
.pw-item .pw{font-family:monospace;color:#00cc00;font-size:1.05em;word-break:break-all;flex:1;margin-right:10px}
.pw-item .pw-label{color:#666;font-size:0.8em;margin-top:2px}
.copy-btn{padding:6px 14px;background:#333;color:#00cc00;border:1px solid #00cc00;border-radius:6px;cursor:pointer;font-size:0.85em;flex-shrink:0}
.copy-btn:hover{background:#0a2a0a}
.empty{text-align:center;color:#666;padding:30px}
</style></head><body>
<h1>&#128272; Password Generator</h1>
<div class="container">
<div class="gen-box">
<h2>Generate Password</h2>
<div class="row"><label>Length</label><input type="range" id="len" min="4" max="64" value="16" oninput="document.getElementById('lenVal').textContent=this.value"><span class="len-val" id="lenVal">16</span></div>
<div class="row"><label>Label</label><input type="text" id="label" placeholder="e.g. My Bank"></div>
<div class="toggle-row">
<button class="toggle active" id="tLower" onclick="this.classList.toggle('active')">abc</button>
<button class="toggle active" id="tUpper" onclick="this.classList.toggle('active')">ABC</button>
<button class="toggle active" id="tDigits" onclick="this.classList.toggle('active')">123</button>
<button class="toggle active" id="tSymbols" onclick="this.classList.toggle('active')">!@#</button>
</div>
<button class="gen-btn" onclick="generate()">Generate</button>
</div>
<div class="history"><h2>History <button class="clear-btn" onclick="clearHist()">Clear</button></h2>
"##);

    // Render history in reverse
    let hb = history.as_bytes();
    let mut starts: [usize; 64] = [0; 64];
    let mut ends: [usize; 64] = [0; 64];
    let mut cnt = 0usize;
    let mut hpos = 0usize;
    while hpos < hb.len() && cnt < 64 {
        starts[cnt] = hpos;
        let mut hend = hpos;
        while hend < hb.len() && hb[hend] != b'\n' { hend += 1; }
        ends[cnt] = hend;
        if hend > hpos { cnt += 1; }
        hpos = hend + 1;
    }
    if cnt == 0 {
        p = buf_write(p, r##"<div class="empty">No passwords generated yet.</div>"##);
    } else {
        let mut ri = cnt;
        while ri > 0 {
            ri -= 1;
            let line = &hb[starts[ri]..ends[ri]];
            let mut sep = 0;
            let mut si = 0;
            while si < line.len() { if line[si] == b'|' { sep = si; break; } si += 1; }
            if sep > 0 {
                let pw = unsafe { core::str::from_utf8_unchecked(&line[..sep]) };
                let label = unsafe { core::str::from_utf8_unchecked(&line[sep+1..]) };
                p = buf_write(p, r##"<div class="pw-item"><div><div class="pw" id="pw"##);
                p = write_usize(p, ri);
                p = buf_write(p, "\">");
                p = buf_write(p, pw);
                p = buf_write(p, r##"</div><div class="pw-label">"##);
                p = buf_write(p, label);
                p = buf_write(p, r##"</div></div><button class="copy-btn" onclick="copyPw('pw"##);
                p = write_usize(p, ri);
                p = buf_write(p, r##"')">Copy</button></div>"##);
            }
        }
    }

    p = buf_write(p, r##"</div></div>
<script>
var B=location.pathname;
function generate(){
  var len=document.getElementById('len').value;
  var label=document.getElementById('label').value||'Untitled';
  var lower=document.getElementById('tLower').classList.contains('active')?'1':'0';
  var upper=document.getElementById('tUpper').classList.contains('active')?'1':'0';
  var digits=document.getElementById('tDigits').classList.contains('active')?'1':'0';
  var symbols=document.getElementById('tSymbols').classList.contains('active')?'1':'0';
  fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'generate',length:len,upper:upper,lower:lower,digits:digits,symbols:symbols,label:label})}).then(()=>location.reload());
}
function copyPw(id){var el=document.getElementById(id);if(navigator.clipboard){navigator.clipboard.writeText(el.textContent);el.style.color='#fff';setTimeout(()=>el.style.color='',500)}}
function clearHist(){fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'clear'})}).then(()=>location.reload())}
</script></body></html>"##);

    respond(200, buf_as_str(p), "text/html");
}
