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
        if action == "add" {
            let word = find_json_str(body, "word").unwrap_or("");
            let meaning = find_json_str(body, "meaning").unwrap_or("");
            if word.len() > 0 && meaning.len() > 0 {
                let existing = kv_read("vocab_cards").unwrap_or("");
                let mut p = 0usize;
                p = buf_write(p, existing);
                p = buf_write(p, word);
                p = buf_write(p, "|");
                p = buf_write(p, meaning);
                p = buf_write(p, "\n");
                kv_write("vocab_cards", buf_as_str(p));
            }
        } else if action == "correct" || action == "wrong" {
            let score_str = kv_read("vocab_score").unwrap_or("0|0");
            let sb = score_str.as_bytes();
            let mut sep = 0;
            let mut si = 0;
            while si < sb.len() { if sb[si] == b'|' { sep = si; break; } si += 1; }
            let correct = parse_usize(unsafe { core::str::from_utf8_unchecked(&sb[..sep]) });
            let wrong = parse_usize(unsafe { core::str::from_utf8_unchecked(&sb[sep+1..]) });
            let mut p = 0usize;
            if action == "correct" {
                p = write_usize(p, correct + 1);
                p = buf_write(p, "|");
                p = write_usize(p, wrong);
            } else {
                p = write_usize(p, correct);
                p = buf_write(p, "|");
                p = write_usize(p, wrong + 1);
            }
            kv_write("vocab_score", buf_as_str(p));
        }
    }

    let cards = kv_read("vocab_cards").unwrap_or("");
    let score_str = kv_read("vocab_score").unwrap_or("0|0");

    let sb = score_str.as_bytes();
    let mut sep = 0;
    let mut si = 0;
    while si < sb.len() { if sb[si] == b'|' { sep = si; break; } si += 1; }
    let correct = if sep > 0 { parse_usize(unsafe { core::str::from_utf8_unchecked(&sb[..sep]) }) } else { 0 };
    let wrong = if sep > 0 { parse_usize(unsafe { core::str::from_utf8_unchecked(&sb[sep+1..]) }) } else { 0 };

    let mut p = 0usize;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Vocabulary Cards</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#0f0e17;color:#fffffe;font-family:'Segoe UI',sans-serif;min-height:100vh;padding:20px;display:flex;flex-direction:column;align-items:center}
h1{color:#ff8906;margin:20px 0;font-size:2em}
.container{width:100%;max-width:550px}
.score-bar{display:flex;gap:20px;justify-content:center;margin-bottom:20px}
.score-item{background:#1a1a2e;padding:12px 24px;border-radius:12px;text-align:center}
.score-item .num{font-size:1.8em;font-weight:bold}
.score-item.correct .num{color:#2cb67d}
.score-item.wrong .num{color:#e53170}
.score-item .label{font-size:0.8em;color:#a7a9be;margin-top:4px}
.card-area{perspective:1000px;margin-bottom:20px;min-height:250px}
.card{width:100%;height:250px;position:relative;cursor:pointer;transform-style:preserve-3d;transition:transform 0.6s}
.card.flipped{transform:rotateY(180deg)}
.card-face{position:absolute;width:100%;height:100%;border-radius:20px;display:flex;flex-direction:column;align-items:center;justify-content:center;backface-visibility:hidden;padding:30px}
.card-front{background:linear-gradient(135deg,#1a1a2e,#2d2b55);border:2px solid #ff8906}
.card-back{background:linear-gradient(135deg,#2d2b55,#1a1a2e);border:2px solid #2cb67d;transform:rotateY(180deg)}
.card-word{font-size:2.2em;font-weight:bold;color:#ff8906}
.card-meaning{font-size:1.6em;color:#2cb67d}
.card-hint{font-size:0.9em;color:#a7a9be;margin-top:15px}
.btn-row{display:flex;gap:10px;margin-bottom:20px}
.btn{flex:1;padding:14px;border:none;border-radius:10px;font-size:1em;cursor:pointer;font-weight:bold}
.btn-correct{background:#2cb67d;color:#0f0e17}
.btn-wrong{background:#e53170;color:#fff}
.btn-next{background:#ff8906;color:#0f0e17}
.add-form{background:#1a1a2e;border-radius:16px;padding:20px;margin-bottom:20px}
.add-form h2{color:#ff8906;margin-bottom:12px}
.add-form input{width:100%;padding:12px;border:1px solid #2d2b55;border-radius:8px;background:#0f0e17;color:#fffffe;font-size:1em;margin-bottom:10px}
.add-form input:focus{outline:none;border-color:#ff8906}
.add-form button{width:100%;padding:12px;background:#ff8906;color:#0f0e17;border:none;border-radius:8px;font-size:1em;cursor:pointer;font-weight:bold}
.card-list{background:#1a1a2e;border-radius:16px;padding:20px}
.card-list h2{color:#ff8906;margin-bottom:12px}
.vocab-item{padding:10px;border-bottom:1px solid #2d2b55;display:flex;justify-content:space-between}
.vocab-item:last-child{border-bottom:none}
.vocab-item .w{color:#ff8906;font-weight:bold}
.vocab-item .m{color:#a7a9be}
.empty{text-align:center;color:#a7a9be;padding:30px}
</style></head><body>
<h1>&#128218; Vocabulary Cards</h1>
<div class="container">
<div class="score-bar">
<div class="score-item correct"><div class="num">"##);
    p = write_usize(p, correct);
    p = buf_write(p, r##"</div><div class="label">Correct</div></div>
<div class="score-item wrong"><div class="num">"##);
    p = write_usize(p, wrong);
    p = buf_write(p, r##"</div><div class="label">Wrong</div></div>
</div>
<div class="card-area"><div class="card" id="flashcard" onclick="flipCard()">
<div class="card-face card-front"><div class="card-word" id="cardWord">Add cards below</div><div class="card-hint">Click to flip</div></div>
<div class="card-face card-back"><div class="card-meaning" id="cardMeaning">-</div><div class="card-hint">Did you know it?</div></div>
</div></div>
<div class="btn-row">
<button class="btn btn-wrong" onclick="markWrong()">&#10060; Wrong</button>
<button class="btn btn-next" onclick="nextCard()">&#10145; Next</button>
<button class="btn btn-correct" onclick="markCorrect()">&#9989; Correct</button>
</div>
<div class="add-form"><h2>Add New Card</h2>
<input type="text" id="word" placeholder="Word or phrase">
<input type="text" id="meaning" placeholder="Definition or translation">
<button onclick="addCard()">Add Card</button></div>
<div class="card-list"><h2>All Cards</h2>"##);

    let cb = cards.as_bytes();
    let mut cpos = 0usize;
    let mut card_count = 0usize;

    // Emit cards as JS data and list items
    p = buf_write(p, "<div id='cardItems'>");
    while cpos < cb.len() {
        let mut cend = cpos;
        while cend < cb.len() && cb[cend] != b'\n' { cend += 1; }
        if cend > cpos {
            let line = &cb[cpos..cend];
            let mut lsep = 0;
            let mut li = 0;
            while li < line.len() { if line[li] == b'|' { lsep = li; break; } li += 1; }
            if lsep > 0 {
                let word = unsafe { core::str::from_utf8_unchecked(&line[..lsep]) };
                let meaning = unsafe { core::str::from_utf8_unchecked(&line[lsep+1..]) };
                p = buf_write(p, r##"<div class="vocab-item"><span class="w">"##);
                p = buf_write(p, word);
                p = buf_write(p, r##"</span><span class="m">"##);
                p = buf_write(p, meaning);
                p = buf_write(p, "</span></div>");
                card_count += 1;
            }
        }
        cpos = cend + 1;
    }
    if card_count == 0 {
        p = buf_write(p, r##"<div class="empty">No cards yet. Add your first vocabulary card!</div>"##);
    }
    p = buf_write(p, "</div></div></div>");

    // Build JS card data
    p = buf_write(p, "\n<script>\nvar cards=[");
    cpos = 0;
    let mut first = true;
    while cpos < cb.len() {
        let mut cend = cpos;
        while cend < cb.len() && cb[cend] != b'\n' { cend += 1; }
        if cend > cpos {
            let line = &cb[cpos..cend];
            let mut lsep = 0;
            let mut li = 0;
            while li < line.len() { if line[li] == b'|' { lsep = li; break; } li += 1; }
            if lsep > 0 {
                let word = unsafe { core::str::from_utf8_unchecked(&line[..lsep]) };
                let meaning = unsafe { core::str::from_utf8_unchecked(&line[lsep+1..]) };
                if !first { p = buf_write(p, ","); }
                p = buf_write(p, "[\"");
                p = buf_write(p, word);
                p = buf_write(p, "\",\"");
                p = buf_write(p, meaning);
                p = buf_write(p, "\"]");
                first = false;
            }
        }
        cpos = cend + 1;
    }

    p = buf_write(p, r##"];
var idx=0;
function showCard(){if(cards.length===0)return;idx=idx%cards.length;document.getElementById('cardWord').textContent=cards[idx][0];document.getElementById('cardMeaning').textContent=cards[idx][1];document.getElementById('flashcard').classList.remove('flipped')}
function flipCard(){document.getElementById('flashcard').classList.toggle('flipped')}
function nextCard(){idx++;showCard()}
function markCorrect(){fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'correct'})}).then(function(){nextCard();location.reload()})}
function markWrong(){fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'wrong'})}).then(function(){nextCard();location.reload()})}
function addCard(){var w=document.getElementById('word').value;var m=document.getElementById('meaning').value;if(!w||!m)return alert('Fill in both fields');fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add',word:w,meaning:m})}).then(function(){location.reload()})}
showCard();
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
