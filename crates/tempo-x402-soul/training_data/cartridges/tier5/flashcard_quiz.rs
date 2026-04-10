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

fn respond(status: i32, body: &str, ct: &str) { unsafe { response(status, body.as_ptr(), body.len() as i32, ct.as_ptr(), ct.len() as i32); } }
fn host_log(level: i32, msg: &str) { unsafe { log(level, msg.as_ptr(), msg.len() as i32); } }
fn kv_read(key: &str) -> Option<&'static str> { unsafe { let r = kv_get(key.as_ptr(), key.len() as i32); if r < 0 { return None; } let p = (r >> 32) as *const u8; let l = (r & 0xFFFFFFFF) as usize; core::str::from_utf8(core::slice::from_raw_parts(p, l)).ok() } }
fn kv_write(key: &str, val: &str) { unsafe { kv_set(key.as_ptr(), key.len() as i32, val.as_ptr(), val.len() as i32); } }
fn find_json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let kb = key.as_bytes(); let jb = json.as_bytes(); let mut i = 0;
    while i + kb.len() + 3 < jb.len() { if jb[i] == b'"' { let s = i + 1; if s + kb.len() < jb.len() && &jb[s..s + kb.len()] == kb && jb[s + kb.len()] == b'"' { let mut j = s + kb.len() + 1; while j < jb.len() && (jb[j] == b':' || jb[j] == b' ') { j += 1; } if j < jb.len() && jb[j] == b'"' { let vs = j + 1; let mut ve = vs; while ve < jb.len() && jb[ve] != b'"' { ve += 1; } return core::str::from_utf8(&jb[vs..ve]).ok(); } } } i += 1; } None
}
static mut BUF: [u8; 65536] = [0u8; 65536];
struct W { pos: usize }
impl W {
    fn new() -> Self { Self { pos: 0 } }
    fn s(&mut self, s: &str) { let b = s.as_bytes(); unsafe { let e = (self.pos + b.len()).min(BUF.len()); BUF[self.pos..e].copy_from_slice(&b[..e - self.pos]); self.pos = e; } }
    fn n(&mut self, mut n: u32) { if n == 0 { self.s("0"); return; } let mut d = [0u8; 10]; let mut i = 0; while n > 0 { d[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; } while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = d[i]; self.pos += 1; } } } }
    fn out(&self) -> &str { unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) } }
}
static mut SCRATCH: [u8; 131072] = [0u8; 131072];
#[no_mangle] pub extern "C" fn x402_alloc(size: i32) -> *mut u8 { unsafe { SCRATCH.as_mut_ptr() } }
fn parse_u32(s: &str) -> u32 { let mut n: u32 = 0; for &b in s.as_bytes() { if b >= b'0' && b <= b'9' { n = n * 10 + (b - b'0') as u32; } } n }
fn write_key(buf: &mut [u8], prefix: &[u8], num: u32) -> usize {
    let mut pos = 0; for &b in prefix { buf[pos] = b; pos += 1; }
    if num == 0 { buf[pos] = b'0'; return pos + 1; }
    let mut d = [0u8; 10]; let mut di = 0; let mut n = num;
    while n > 0 { d[di] = b'0' + (n % 10) as u8; n /= 10; di += 1; }
    while di > 0 { di -= 1; buf[pos] = d[di]; pos += 1; } pos
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize)) };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let path = find_json_str(request, "path").unwrap_or("/");
    let body = find_json_str(request, "body").unwrap_or("");
    host_log(0, "flashcard_quiz: handling request");

    if method == "POST" {
        if let Some(action) = find_json_str(body, "action") {
            if action == "add_deck" {
                let name = find_json_str(body, "name").unwrap_or("");
                if !name.is_empty() {
                    let decks = kv_read("fq_decks").unwrap_or("");
                    let mut w = W::new();
                    if !decks.is_empty() { w.s(decks); w.s(","); }
                    w.s(name);
                    kv_write("fq_decks", w.out());
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"name required"}"#, "application/json"); }
            } else if action == "add_card" {
                let deck = find_json_str(body, "deck").unwrap_or("");
                let front = find_json_str(body, "front").unwrap_or("");
                let back = find_json_str(body, "back").unwrap_or("");
                if !deck.is_empty() && !front.is_empty() {
                    let mut dk = [0u8; 32]; let mut dp = 0;
                    for &b in b"fq_d_" { dk[dp] = b; dp += 1; }
                    for &b in deck.as_bytes() { if dp < 32 { dk[dp] = b; dp += 1; } }
                    let key = unsafe { core::str::from_utf8_unchecked(&dk[..dp]) };
                    let existing = kv_read(key).unwrap_or("");
                    let mut w = W::new();
                    if !existing.is_empty() { w.s(existing); w.s("\n"); }
                    // Format: front|back|correct|total
                    w.s(front); w.s("|"); w.s(back); w.s("|0|0");
                    kv_write(key, w.out());
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"missing fields"}"#, "application/json"); }
            } else if action == "answer" {
                let deck = find_json_str(body, "deck").unwrap_or("");
                let card_idx = find_json_str(body, "card").map(|s| parse_u32(s)).unwrap_or(0);
                let correct = find_json_str(body, "correct").unwrap_or("0");
                let mut dk = [0u8; 32]; let mut dp = 0;
                for &b in b"fq_d_" { dk[dp] = b; dp += 1; }
                for &b in deck.as_bytes() { if dp < 32 { dk[dp] = b; dp += 1; } }
                let key = unsafe { core::str::from_utf8_unchecked(&dk[..dp]) };
                let existing = kv_read(key).unwrap_or("");
                let eb = existing.as_bytes();
                let mut w = W::new();
                let mut p = 0; let mut line_num: u32 = 0;
                while p < eb.len() {
                    let ls = p;
                    while p < eb.len() && eb[p] != b'\n' { p += 1; }
                    let line = unsafe { core::str::from_utf8_unchecked(&eb[ls..p]) };
                    if p < eb.len() { p += 1; }
                    if w.pos > 0 { w.s("\n"); }
                    if line_num == card_idx {
                        // Update stats
                        let lb = line.as_bytes();
                        let mut pipes = [0usize; 3]; let mut pi = 0; let mut li = 0;
                        while li < lb.len() && pi < 3 { if lb[li] == b'|' { pipes[pi] = li; pi += 1; } li += 1; }
                        if pi >= 3 {
                            let c = parse_u32(&line[pipes[1]+1..pipes[2]]);
                            let t = parse_u32(&line[pipes[2]+1..]);
                            w.s(&line[..pipes[1]+1]);
                            if correct == "1" { w.n(c + 1); } else { w.n(c); }
                            w.s("|"); w.n(t + 1);
                        } else { w.s(line); }
                    } else { w.s(line); }
                    line_num += 1;
                }
                kv_write(key, w.out());
                // Update total session stats
                let total = kv_read("fq_total").map(|s| parse_u32(s)).unwrap_or(0);
                let right = kv_read("fq_right").map(|s| parse_u32(s)).unwrap_or(0);
                let mut tw = W::new(); tw.n(total + 1); kv_write("fq_total", tw.out());
                if correct == "1" { let mut rw = W::new(); rw.n(right + 1); kv_write("fq_right", rw.out()); }
                respond(200, r#"{"ok":true}"#, "application/json");
            } else { respond(400, r#"{"error":"unknown"}"#, "application/json"); }
        } else { respond(400, r#"{"error":"missing action"}"#, "application/json"); }
        return;
    }

    let decks = kv_read("fq_decks").unwrap_or("");
    let total = kv_read("fq_total").map(|s| parse_u32(s)).unwrap_or(0);
    let right = kv_read("fq_right").map(|s| parse_u32(s)).unwrap_or(0);
    let pct = if total > 0 { (right * 100) / total } else { 0 };

    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Flashcard Quiz</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#0f0f1a;color:#e0e0e0;font-family:'Segoe UI',sans-serif;padding:30px 20px;display:flex;justify-content:center}");
    w.s(".c{max-width:600px;width:100%}h1{text-align:center;color:#a78bfa;margin-bottom:20px}");
    w.s(".stats{display:flex;gap:12px;justify-content:center;margin-bottom:20px}");
    w.s(".stat{background:#1a1a2e;padding:12px 20px;border-radius:8px;text-align:center}.stat .v{font-size:22px;font-weight:bold;color:#a78bfa}.stat .l{font-size:11px;color:#888}");
    w.s(".form{background:#1a1a2e;padding:16px;border-radius:10px;margin-bottom:16px;display:flex;gap:8px}");
    w.s("input,textarea{padding:10px;background:#0f0f1a;border:1px solid #333;color:#e0e0e0;border-radius:6px;font-size:14px;flex:1}");
    w.s("button{padding:10px 18px;background:#7c3aed;color:#fff;border:none;border-radius:6px;cursor:pointer;font-size:14px}");
    w.s(".deck{background:#1a1a2e;padding:16px;border-radius:10px;margin-bottom:10px}");
    w.s(".deck-header{display:flex;justify-content:space-between;align-items:center;margin-bottom:12px}");
    w.s(".deck-name{font-size:18px;font-weight:bold;color:#a78bfa}");
    w.s(".card-form{display:flex;gap:8px;margin-bottom:10px}");
    w.s(".card{background:#0f0f1a;padding:12px;border-radius:8px;margin-bottom:6px;cursor:pointer;text-align:center;min-height:80px;display:flex;align-items:center;justify-content:center;font-size:16px;transition:all 0.3s;perspective:1000px}");
    w.s(".card .front{color:#e0e0e0}.card .back{display:none;color:#a78bfa}");
    w.s(".card.flipped .front{display:none}.card.flipped .back{display:block}");
    w.s(".card-btns{display:flex;gap:8px;justify-content:center;margin-top:8px}.correct{background:#238636}.wrong{background:#da3633}");
    w.s(".accuracy{font-size:13px;color:#888;margin-top:4px}");
    w.s("</style></head><body><div class='c'><h1>Flashcard Quiz</h1>");

    w.s("<div class='stats'><div class='stat'><div class='v'>"); w.n(total);
    w.s("</div><div class='l'>Reviewed</div></div><div class='stat'><div class='v'>"); w.n(right);
    w.s("</div><div class='l'>Correct</div></div><div class='stat'><div class='v'>"); w.n(pct);
    w.s("%</div><div class='l'>Accuracy</div></div></div>");

    w.s("<div class='form'><input id='deckName' placeholder='New deck name'><button onclick='addDeck()'>Create Deck</button></div>");

    // Render decks with cards
    if !decks.is_empty() {
        let db = decks.as_bytes();
        let mut p = 0;
        while p <= db.len() {
            let ss = p;
            while p < db.len() && db[p] != b',' { p += 1; }
            let name = unsafe { core::str::from_utf8_unchecked(&db[ss..p]) };
            p += 1;
            if name.is_empty() { continue; }

            let mut dk = [0u8; 32]; let mut dp = 0;
            for &b in b"fq_d_" { dk[dp] = b; dp += 1; }
            for &b in name.as_bytes() { if dp < 32 { dk[dp] = b; dp += 1; } }
            let key = unsafe { core::str::from_utf8_unchecked(&dk[..dp]) };
            let cards_data = kv_read(key).unwrap_or("");

            // Count cards
            let mut card_count: u32 = 0;
            if !cards_data.is_empty() {
                let cb = cards_data.as_bytes();
                card_count = 1;
                let mut ci = 0;
                while ci < cb.len() { if cb[ci] == b'\n' { card_count += 1; } ci += 1; }
            }

            w.s("<div class='deck'><div class='deck-header'><span class='deck-name'>"); w.s(name);
            w.s("</span><span>"); w.n(card_count); w.s(" cards</span></div>");
            w.s("<div class='card-form'><input id='f_"); w.s(name); w.s("' placeholder='Front'><input id='b_"); w.s(name);
            w.s("' placeholder='Back'><button onclick=\"addCard('"); w.s(name); w.s("')\">Add</button></div>");

            // Render cards
            if !cards_data.is_empty() {
                let cb = cards_data.as_bytes();
                let mut cp = 0; let mut idx: u32 = 0;
                while cp < cb.len() {
                    let ls = cp;
                    while cp < cb.len() && cb[cp] != b'\n' { cp += 1; }
                    let line = unsafe { core::str::from_utf8_unchecked(&cb[ls..cp]) };
                    if cp < cb.len() { cp += 1; }
                    let lb = line.as_bytes();
                    let mut pipes = [0usize; 3]; let mut pi = 0; let mut li = 0;
                    while li < lb.len() && pi < 3 { if lb[li] == b'|' { pipes[pi] = li; pi += 1; } li += 1; }
                    if pi >= 3 {
                        let front = &line[..pipes[0]];
                        let back = &line[pipes[0]+1..pipes[1]];
                        let correct_count = parse_u32(&line[pipes[1]+1..pipes[2]]);
                        let total_count = parse_u32(&line[pipes[2]+1..]);
                        let acc = if total_count > 0 { (correct_count * 100) / total_count } else { 0 };

                        w.s("<div class='card' id='card_"); w.s(name); w.s("_"); w.n(idx);
                        w.s("' onclick=\"flip('"); w.s(name); w.s("',"); w.n(idx); w.s(")\">");
                        w.s("<span class='front'>"); w.s(front); w.s("</span>");
                        w.s("<span class='back'>"); w.s(back); w.s("</span></div>");
                        w.s("<div class='card-btns' id='btns_"); w.s(name); w.s("_"); w.n(idx);
                        w.s("' style='display:none'><button class='correct' onclick=\"answer('"); w.s(name); w.s("',"); w.n(idx); w.s(",1)\">Correct</button>");
                        w.s("<button class='wrong' onclick=\"answer('"); w.s(name); w.s("',"); w.n(idx); w.s(",0)\">Wrong</button></div>");
                        if total_count > 0 {
                            w.s("<div class='accuracy'>"); w.n(correct_count); w.s("/"); w.n(total_count); w.s(" ("); w.n(acc); w.s("%)</div>");
                        }
                    }
                    idx += 1;
                }
            }
            w.s("</div>");
        }
    }

    w.s("</div><script>const B=location.pathname;");
    w.s("async function addDeck(){const n=document.getElementById('deckName').value.trim();if(!n)return;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add_deck',name:n})});location.reload();}");
    w.s("async function addCard(deck){const f=document.getElementById('f_'+deck).value.trim();const b=document.getElementById('b_'+deck).value.trim();if(!f||!b)return;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add_card',deck:deck,front:f,back:b})});location.reload();}");
    w.s("function flip(deck,idx){document.getElementById('card_'+deck+'_'+idx).classList.toggle('flipped');document.getElementById('btns_'+deck+'_'+idx).style.display='flex';}");
    w.s("async function answer(deck,idx,correct){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'answer',deck:deck,card:String(idx),correct:String(correct)})});location.reload();}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
