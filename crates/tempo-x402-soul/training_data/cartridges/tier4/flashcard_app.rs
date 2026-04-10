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
fn tmp_write_usize(pos: usize, mut n: usize) -> usize {
    if n == 0 { return tmp_write(pos, "0"); }
    static mut TD: [u8; 20] = [0u8; 20];
    let mut i = 0;
    while n > 0 { unsafe { TD[i] = b'0' + (n % 10) as u8; } n /= 10; i += 1; }
    let mut p = pos;
    while i > 0 { i -= 1; let d = unsafe { TD[i] }; let s = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(&d, 1)) }; p = tmp_write(p, s); }
    p
}
fn tmp_as_str(len: usize) -> &'static str {
    unsafe { core::str::from_utf8_unchecked(&TMP[..len]) }
}

// KV "cards": "front|back|correct|total\n" per card
// KV "mastery": overall mastery score as a number string

#[no_mangle]
pub extern "C" fn handle_request(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("");
        if action == "add" {
            let front = find_json_str(body, "front").unwrap_or("");
            let back = find_json_str(body, "back").unwrap_or("");
            if front.len() > 0 && back.len() > 0 {
                let existing = kv_read("cards").unwrap_or("");
                let mut tp = 0usize;
                tp = tmp_write(tp, existing);
                tp = tmp_write(tp, front);
                tp = tmp_write(tp, "|");
                tp = tmp_write(tp, back);
                tp = tmp_write(tp, "|0|0\n");
                kv_write("cards", tmp_as_str(tp));
            }
            respond(200, "{\"ok\":true}", "application/json");
            return;
        }
        if action == "answer" {
            let idx = parse_usize(find_json_str(body, "index").unwrap_or("0"));
            let correct = find_json_str(body, "correct").unwrap_or("0");
            let existing = kv_read("cards").unwrap_or("");
            let eb = existing.as_bytes();
            let mut tp = 0usize;
            let mut pos = 0usize;
            let mut line_num = 0usize;
            let mut total_correct = 0usize;
            let mut total_attempts = 0usize;
            while pos < eb.len() {
                let start = pos;
                while pos < eb.len() && eb[pos] != b'\n' { pos += 1; }
                let line = &eb[start..pos];
                if pos < eb.len() { pos += 1; }
                if line.len() < 3 { line_num += 1; continue; }
                // Parse: front|back|correct_count|total_count
                let mut seps: [usize; 3] = [0; 3];
                let mut sc = 0;
                let mut si = 0;
                while si < line.len() && sc < 3 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
                if sc >= 3 {
                    let front_s = unsafe { core::str::from_utf8_unchecked(&line[..seps[0]]) };
                    let back_s = unsafe { core::str::from_utf8_unchecked(&line[seps[0]+1..seps[1]]) };
                    let c_s = unsafe { core::str::from_utf8_unchecked(&line[seps[1]+1..seps[2]]) };
                    let t_s = unsafe { core::str::from_utf8_unchecked(&line[seps[2]+1..]) };
                    let mut c = parse_usize(c_s);
                    let mut t = parse_usize(t_s);
                    if line_num == idx {
                        t += 1;
                        if correct == "1" { c += 1; }
                    }
                    total_correct += c;
                    total_attempts += t;
                    tp = tmp_write(tp, front_s);
                    tp = tmp_write(tp, "|");
                    tp = tmp_write(tp, back_s);
                    tp = tmp_write(tp, "|");
                    tp = tmp_write_usize(tp, c);
                    tp = tmp_write(tp, "|");
                    tp = tmp_write_usize(tp, t);
                    tp = tmp_write(tp, "\n");
                }
                line_num += 1;
            }
            kv_write("cards", tmp_as_str(tp));
            // Update mastery score (percentage)
            if total_attempts > 0 {
                let mastery = (total_correct * 100) / total_attempts;
                let mut mp = 0usize;
                mp = tmp_write_usize(mp, mastery);
                kv_write("mastery", tmp_as_str(mp));
            }
            respond(200, "{\"ok\":true}", "application/json");
            return;
        }
        if action == "delete" {
            let idx = parse_usize(find_json_str(body, "index").unwrap_or("0"));
            let existing = kv_read("cards").unwrap_or("");
            let eb = existing.as_bytes();
            let mut tp = 0usize;
            let mut pos = 0usize;
            let mut line_num = 0usize;
            while pos < eb.len() {
                let start = pos;
                while pos < eb.len() && eb[pos] != b'\n' { pos += 1; }
                let line = &eb[start..pos];
                if pos < eb.len() { pos += 1; }
                if line.len() < 3 { line_num += 1; continue; }
                if line_num != idx {
                    let ls = unsafe { core::str::from_utf8_unchecked(line) };
                    tp = tmp_write(tp, ls);
                    tp = tmp_write(tp, "\n");
                }
                line_num += 1;
            }
            kv_write("cards", tmp_as_str(tp));
            respond(200, "{\"ok\":true}", "application/json");
            return;
        }
        respond(400, "{\"error\":\"unknown action\"}", "application/json");
        return;
    }

    // GET — render
    let cards = kv_read("cards").unwrap_or("");
    let mastery = kv_read("mastery").unwrap_or("0");
    let mut p = 0usize;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Flashcard App</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#1a1a2e;color:#eee;font-family:'Segoe UI',sans-serif;min-height:100vh;display:flex;flex-direction:column;align-items:center;padding:20px}
h1{color:#e94560;margin:20px 0;font-size:2em}
.container{width:100%;max-width:600px}
.add-form{background:#16213e;padding:20px;border-radius:12px;margin-bottom:20px;display:flex;flex-direction:column;gap:10px}
.add-form input{padding:12px;border:1px solid #0f3460;border-radius:8px;background:#1a1a2e;color:#eee;font-size:1em}
.add-form input:focus{outline:none;border-color:#e94560}
.add-form button{padding:12px;background:#e94560;color:#fff;border:none;border-radius:8px;font-size:1em;cursor:pointer;font-weight:bold}
.add-form button:hover{background:#c81e45}
.mastery-bar{background:#16213e;border-radius:12px;padding:15px;margin-bottom:20px;text-align:center}
.mastery-bar h2{font-size:1em;color:#8b949e;margin-bottom:8px}
.bar-outer{background:#0f3460;border-radius:8px;height:24px;overflow:hidden}
.bar-inner{background:linear-gradient(90deg,#e94560,#f0883e);height:100%;border-radius:8px;transition:width 0.3s}
.mastery-pct{font-size:1.3em;font-weight:bold;color:#e94560;margin-top:8px}
.card-container{perspective:800px;margin-bottom:20px;display:none}
.card{width:100%;min-height:200px;position:relative;transform-style:preserve-3d;transition:transform 0.6s;cursor:pointer}
.card.flipped{transform:rotateY(180deg)}
.card-face{position:absolute;width:100%;min-height:200px;backface-visibility:hidden;border-radius:16px;display:flex;align-items:center;justify-content:center;padding:30px;font-size:1.3em;text-align:center}
.card-front{background:linear-gradient(135deg,#16213e,#0f3460);border:2px solid #e94560}
.card-back{background:linear-gradient(135deg,#0f3460,#16213e);border:2px solid #53d8fb;transform:rotateY(180deg);color:#53d8fb}
.card-actions{display:none;justify-content:center;gap:15px;margin-bottom:20px}
.card-actions button{padding:12px 30px;border:none;border-radius:8px;font-size:1em;cursor:pointer;font-weight:bold}
.btn-wrong{background:#da3633;color:#fff}
.btn-wrong:hover{background:#b52a27}
.btn-right{background:#238636;color:#fff}
.btn-right:hover{background:#2ea043}
.btn-next{background:#0f3460;color:#53d8fb;border:1px solid #53d8fb;padding:10px 20px;border-radius:8px;cursor:pointer;font-size:0.95em}
.btn-next:hover{background:#16213e}
.card-list{margin-top:10px}
.card-item{background:#16213e;border-radius:10px;padding:14px;margin-bottom:8px;display:flex;justify-content:space-between;align-items:center}
.card-item .front-text{color:#e94560;font-weight:bold}
.card-item .stats{color:#8b949e;font-size:0.85em}
.card-item .del{background:#21262d;color:#f85149;border:1px solid #30363d;border-radius:6px;padding:4px 10px;cursor:pointer;font-size:0.85em}
.card-item .del:hover{background:#da3633;color:#fff}
.empty{text-align:center;color:#8b949e;padding:40px}
.study-btn{display:block;width:100%;padding:14px;background:#e94560;color:#fff;border:none;border-radius:10px;font-size:1.1em;cursor:pointer;font-weight:bold;margin-bottom:20px}
.study-btn:hover{background:#c81e45}
</style></head><body>
<h1>&#127183; Flashcard App</h1>
<div class="container">
<div class="add-form">
<input type="text" id="front" placeholder="Front (question)...">
<input type="text" id="back" placeholder="Back (answer)...">
<button onclick="addCard()">Add Card</button>
</div>
"##);

    // Count cards and compute mastery
    let cb = cards.as_bytes();
    let mut card_count = 0usize;
    let mut cpos = 0usize;
    while cpos < cb.len() {
        let start = cpos;
        while cpos < cb.len() && cb[cpos] != b'\n' { cpos += 1; }
        if cpos > start + 2 { card_count += 1; }
        if cpos < cb.len() { cpos += 1; }
    }

    let mastery_n = parse_usize(mastery);

    if card_count > 0 {
        p = buf_write(p, r##"<div class="mastery-bar"><h2>Mastery Score</h2><div class="bar-outer"><div class="bar-inner" style="width:"##);
        p = write_usize(p, mastery_n);
        p = buf_write(p, r##"%"></div></div><div class="mastery-pct">"##);
        p = write_usize(p, mastery_n);
        p = buf_write(p, r##"%</div></div>
<button class="study-btn" onclick="startStudy()">Study Cards ("##);
        p = write_usize(p, card_count);
        p = buf_write(p, r##")</button>
<div class="card-container" id="cardBox"><div class="card" id="card" onclick="flipCard()"><div class="card-face card-front" id="cardFront"></div><div class="card-face card-back" id="cardBack"></div></div></div>
<div class="card-actions" id="actions"><button class="btn-wrong" onclick="answer(0)">&#10007; Wrong</button><button class="btn-right" onclick="answer(1)">&#10003; Correct</button></div>
"##);
    }

    // Card list
    p = buf_write(p, r##"<div class="card-list"><h2 style="margin-bottom:12px;color:#8b949e;font-size:1em">All Cards</h2>"##);
    cpos = 0;
    let mut cidx = 0usize;
    while cpos < cb.len() {
        let start = cpos;
        while cpos < cb.len() && cb[cpos] != b'\n' { cpos += 1; }
        let line = &cb[start..cpos];
        if cpos < cb.len() { cpos += 1; }
        if line.len() < 3 { continue; }
        let mut seps: [usize; 3] = [0; 3];
        let mut sc = 0;
        let mut si = 0;
        while si < line.len() && sc < 3 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
        if sc >= 3 {
            let front_s = unsafe { core::str::from_utf8_unchecked(&line[..seps[0]]) };
            let c_s = unsafe { core::str::from_utf8_unchecked(&line[seps[1]+1..seps[2]]) };
            let t_s = unsafe { core::str::from_utf8_unchecked(&line[seps[2]+1..]) };
            p = buf_write(p, r##"<div class="card-item"><div><span class="front-text">"##);
            p = buf_write(p, front_s);
            p = buf_write(p, r##"</span><span class="stats"> — "##);
            p = buf_write(p, c_s);
            p = buf_write(p, "/");
            p = buf_write(p, t_s);
            p = buf_write(p, r##" correct</span></div><button class="del" onclick="delCard("##);
            p = write_usize(p, cidx);
            p = buf_write(p, r##")">Del</button></div>"##);
            cidx += 1;
        }
    }
    if card_count == 0 {
        p = buf_write(p, r##"<div class="empty">No cards yet. Add some flashcards to start studying!</div>"##);
    }
    p = buf_write(p, "</div></div>");

    // Build JS card data array
    p = buf_write(p, r##"<script>
var B=location.pathname;
var cards=["##);
    cpos = 0;
    let mut first = true;
    let mut ji = 0usize;
    while cpos < cb.len() {
        let start = cpos;
        while cpos < cb.len() && cb[cpos] != b'\n' { cpos += 1; }
        let line = &cb[start..cpos];
        if cpos < cb.len() { cpos += 1; }
        if line.len() < 3 { continue; }
        let mut seps: [usize; 3] = [0; 3];
        let mut sc = 0;
        let mut si = 0;
        while si < line.len() && sc < 3 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
        if sc >= 3 {
            let front_s = unsafe { core::str::from_utf8_unchecked(&line[..seps[0]]) };
            let back_s = unsafe { core::str::from_utf8_unchecked(&line[seps[0]+1..seps[1]]) };
            if !first { p = buf_write(p, ","); }
            p = buf_write(p, "{f:\"");
            p = buf_write(p, front_s);
            p = buf_write(p, "\",b:\"");
            p = buf_write(p, back_s);
            p = buf_write(p, "\",i:");
            p = write_usize(p, ji);
            p = buf_write(p, "}");
            first = false;
            ji += 1;
        }
    }
    p = buf_write(p, r##"];
var ci=0,studying=false;
function startStudy(){if(!cards.length)return;studying=true;ci=0;showCard();document.getElementById('cardBox').style.display='block';document.getElementById('actions').style.display='flex'}
function showCard(){var c=cards[ci];document.getElementById('cardFront').textContent=c.f;document.getElementById('cardBack').textContent=c.b;document.getElementById('card').classList.remove('flipped')}
function flipCard(){document.getElementById('card').classList.toggle('flipped')}
function answer(ok){fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'answer',index:String(cards[ci].i),correct:String(ok)})}).then(()=>{ci++;if(ci>=cards.length){location.reload()}else{showCard()}})}
function addCard(){var f=document.getElementById('front').value.trim();var b=document.getElementById('back').value.trim();if(!f||!b)return alert('Fill in both sides');fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add',front:f,back:b})}).then(()=>location.reload())}
function delCard(i){fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',index:String(i)})}).then(()=>location.reload())}
</script></body></html>"##);

    respond(200, buf_as_str(p), "text/html");
}
