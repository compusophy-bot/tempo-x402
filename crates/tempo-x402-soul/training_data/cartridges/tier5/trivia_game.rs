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

struct Question {
    text: &'static str,
    options: [&'static str; 4],
    answer: u32, // 0-3
}

const QUESTIONS: [Question; 10] = [
    Question { text: "What is the largest planet in our solar system?", options: ["Mars", "Jupiter", "Saturn", "Neptune"], answer: 1 },
    Question { text: "Which language is the Linux kernel primarily written in?", options: ["C++", "Rust", "C", "Assembly"], answer: 2 },
    Question { text: "What year was Bitcoin's whitepaper published?", options: ["2006", "2008", "2010", "2012"], answer: 1 },
    Question { text: "Which element has the atomic number 79?", options: ["Silver", "Platinum", "Gold", "Copper"], answer: 2 },
    Question { text: "What does CPU stand for?", options: ["Central Processing Unit", "Central Program Utility", "Computer Processing Unit", "Core Processing Unit"], answer: 0 },
    Question { text: "Which protocol operates on port 443?", options: ["HTTP", "FTP", "HTTPS", "SSH"], answer: 2 },
    Question { text: "What is the speed of light in km/s (approx)?", options: ["150,000", "300,000", "500,000", "1,000,000"], answer: 1 },
    Question { text: "Who created the Rust programming language?", options: ["Guido van Rossum", "Bjarne Stroustrup", "Graydon Hoare", "James Gosling"], answer: 2 },
    Question { text: "What is the base of the hexadecimal number system?", options: ["8", "10", "12", "16"], answer: 3 },
    Question { text: "Which data structure uses LIFO ordering?", options: ["Queue", "Stack", "Heap", "Tree"], answer: 1 },
];

fn get_leaderboard_count() -> u32 {
    parse_u32(kv_read("tv_lb_count").unwrap_or("0"))
}

fn render_quiz() {
    let mut p = 0;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Trivia Challenge</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:'Segoe UI',sans-serif;background:#0f1117;color:#e0e0e0;min-height:100vh}
.header{background:#1a1d23;padding:20px 30px;text-align:center;border-bottom:2px solid #f6ad55}
.header h1{color:#f6ad55;font-size:1.8em}
.header p{color:#a0aec0;margin-top:5px}
.container{max-width:800px;margin:0 auto;padding:30px 20px}
.question{background:#1a1d23;border:1px solid #2d3748;border-radius:10px;padding:20px;margin-bottom:15px}
.question .num{color:#f6ad55;font-weight:700;font-size:0.9em;margin-bottom:8px}
.question .text{font-size:1.1em;margin-bottom:15px;line-height:1.4}
.options{display:grid;grid-template-columns:1fr 1fr;gap:8px}
.option{background:#2d3748;border:2px solid #4a5568;border-radius:8px;padding:10px 15px;cursor:pointer;transition:all 0.2s}
.option:hover{border-color:#f6ad55;background:#2d3748}
.option.selected{border-color:#f6ad55;background:#44337a}
.option input{display:none}
.submit-area{text-align:center;margin:25px 0}
.btn{background:#f6ad55;color:#1a1d23;border:none;padding:12px 35px;border-radius:8px;font-size:1.1em;font-weight:700;cursor:pointer}
.btn:hover{background:#ed8936}
.btn-secondary{background:#4a5568;color:#e0e0e0}
.name-input{background:#2d3748;color:#e0e0e0;border:1px solid #4a5568;padding:10px 15px;border-radius:6px;font-size:1em;margin-right:10px;width:200px}
.progress{background:#2d3748;height:6px;border-radius:3px;margin:15px 0}
.progress-bar{background:#f6ad55;height:100%;border-radius:3px;transition:width 0.3s}
.leaderboard{background:#1a1d23;border-radius:10px;padding:20px;margin-top:30px}
.leaderboard h2{color:#f6ad55;margin-bottom:15px}
.lb-row{display:flex;justify-content:space-between;padding:10px 15px;border-bottom:1px solid #2d3748}
.lb-row:last-child{border-bottom:none}
.lb-rank{color:#f6e05e;font-weight:700;width:30px}
.lb-name{flex:1;margin-left:10px}
.lb-score{color:#48bb78;font-weight:600}
.result{background:#1a1d23;border-radius:10px;padding:30px;text-align:center;margin:20px 0}
.result .score{font-size:3em;color:#f6ad55;font-weight:700}
.result .label{color:#a0aec0;margin-top:5px;font-size:1.1em}
</style></head><body>
<div class="header"><h1>Trivia Challenge</h1><p>10 questions &middot; Test your knowledge</p></div>
<div class="container">
<div class="progress"><div class="progress-bar" id="progress" style="width:0%"></div></div>
<form id="quiz-form">"##);

    let mut qi = 0u32;
    while qi < 10 {
        let q = &QUESTIONS[qi as usize];
        p = buf_write(p, r##"<div class="question"><div class="num">Question "##);
        p = buf_write(p, num_to_str(qi + 1));
        p = buf_write(p, r##" of 10</div><div class="text">"##);
        p = buf_write(p, q.text);
        p = buf_write(p, r##"</div><div class="options">"##);
        let mut oi = 0u32;
        while oi < 4 {
            p = buf_write(p, r##"<label class="option" onclick="selectOpt(this,'q"##);
            p = buf_write(p, num_to_str(qi));
            p = buf_write(p, r##"')"><input type="radio" name="q"##);
            p = buf_write(p, num_to_str(qi));
            p = buf_write(p, r##"" value=""##);
            p = buf_write(p, num_to_str(oi));
            p = buf_write(p, r##"">"##);
            p = buf_write(p, q.options[oi as usize]);
            p = buf_write(p, r##"</label>"##);
            oi += 1;
        }
        p = buf_write(p, r##"</div></div>"##);
        qi += 1;
    }

    p = buf_write(p, r##"</form>
<div class="submit-area">
<input type="text" class="name-input" id="player-name" placeholder="Your name">
<button class="btn" onclick="submitQuiz()">Submit Answers</button>
</div>
<div class="result" id="result" style="display:none">
<div class="score" id="score-val"></div>
<div class="label" id="score-label"></div>
<button class="btn btn-secondary" onclick="location.reload()" style="margin-top:15px">Play Again</button>
</div>
<div class="leaderboard"><h2>Leaderboard</h2><div id="lb-list">"##);

    // Render leaderboard
    let lb_count = get_leaderboard_count();
    static mut LB_SCORES: [u32; 50] = [0u32; 50];
    static mut LB_IDXS: [u32; 50] = [0u32; 50];
    let mut total = 0u32;
    let mut li = 0u32;
    while li < lb_count && (total as usize) < 50 {
        let mut kbuf = [0u8; 24];
        let prefix = b"tv_lb_";
        kbuf[..prefix.len()].copy_from_slice(prefix);
        let ns = num_to_str(li);
        let nb = ns.as_bytes();
        kbuf[prefix.len()..prefix.len()+nb.len()].copy_from_slice(nb);
        let key = unsafe { core::str::from_utf8_unchecked(&kbuf[..prefix.len()+nb.len()]) };
        if let Some(data) = kv_read(key) {
            let db = data.as_bytes();
            let mut pipe = 0;
            let mut fi = 0;
            while fi < db.len() { if db[fi] == b'|' { pipe = fi; break; } fi += 1; }
            if pipe > 0 {
                let score_s = unsafe { core::str::from_utf8_unchecked(&db[pipe+1..]) };
                unsafe {
                    LB_SCORES[total as usize] = parse_u32(score_s);
                    LB_IDXS[total as usize] = li;
                }
                total += 1;
            }
        }
        li += 1;
    }
    // Sort descending
    if total > 1 {
        let mut sorted = false;
        while !sorted {
            sorted = true;
            let mut k = 0usize;
            while k + 1 < total as usize {
                unsafe {
                    if LB_SCORES[k] < LB_SCORES[k + 1] {
                        let ts = LB_SCORES[k]; LB_SCORES[k] = LB_SCORES[k+1]; LB_SCORES[k+1] = ts;
                        let ti = LB_IDXS[k]; LB_IDXS[k] = LB_IDXS[k+1]; LB_IDXS[k+1] = ti;
                        sorted = false;
                    }
                }
                k += 1;
            }
        }
    }
    let show = if total > 10 { 10 } else { total };
    let mut ri = 0u32;
    while ri < show {
        let idx = unsafe { LB_IDXS[ri as usize] };
        let mut kbuf2 = [0u8; 24];
        let prefix2 = b"tv_lb_";
        kbuf2[..prefix2.len()].copy_from_slice(prefix2);
        let ns2 = num_to_str(idx);
        let nb2 = ns2.as_bytes();
        kbuf2[prefix2.len()..prefix2.len()+nb2.len()].copy_from_slice(nb2);
        let key2 = unsafe { core::str::from_utf8_unchecked(&kbuf2[..prefix2.len()+nb2.len()]) };
        if let Some(data) = kv_read(key2) {
            let db = data.as_bytes();
            let mut pipe = 0;
            let mut fi = 0;
            while fi < db.len() { if db[fi] == b'|' { pipe = fi; break; } fi += 1; }
            let name = unsafe { core::str::from_utf8_unchecked(&db[..pipe]) };
            let score_s = unsafe { core::str::from_utf8_unchecked(&db[pipe+1..]) };
            p = buf_write(p, r##"<div class="lb-row"><span class="lb-rank">"##);
            p = buf_write(p, num_to_str(ri + 1));
            p = buf_write(p, r##".</span><span class="lb-name">"##);
            p = buf_write(p, name);
            p = buf_write(p, r##"</span><span class="lb-score">"##);
            p = buf_write(p, score_s);
            p = buf_write(p, r##"/10</span></div>"##);
        }
        ri += 1;
    }
    if show == 0 {
        p = buf_write(p, r##"<div class="lb-row" style="color:#a0aec0">No scores yet. Be the first!</div>"##);
    }

    p = buf_write(p, r##"</div></div></div>
<script>
var answered=0;
function selectOpt(el,name){
  var opts=document.querySelectorAll('input[name="'+name+'"]');
  for(var i=0;i<opts.length;i++)opts[i].parentElement.classList.remove('selected');
  el.classList.add('selected');
  el.querySelector('input').checked=true;
  answered=0;for(var q=0;q<10;q++){var r=document.querySelector('input[name="q'+q+'"]:checked');if(r)answered++;}
  document.getElementById('progress').style.width=(answered*10)+'%';
}
function submitQuiz(){
  var answers='';
  for(var q=0;q<10;q++){
    var r=document.querySelector('input[name="q'+q+'"]:checked');
    answers+=r?r.value:'9';
    if(q<9)answers+=',';
  }
  var name=document.getElementById('player-name').value||'Anonymous';
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'submit',name:name,answers:answers})})
  .then(function(r){return r.json();})
  .then(function(d){
    document.getElementById('quiz-form').style.display='none';
    document.getElementById('result').style.display='block';
    document.getElementById('score-val').textContent=d.score+'/10';
    var pct=d.score*10;
    var msg=pct>=80?'Excellent!':pct>=60?'Good job!':pct>=40?'Not bad!':'Keep studying!';
    document.getElementById('score-label').textContent=msg+' ('+pct+'%)';
    document.getElementById('progress').style.width='100%';
  });
}
</script></body></html>"##);
    respond(200, buf_as_str(p), "text/html");
}

#[no_mangle]
pub extern "C" fn x402_handle(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    host_log(1, "Trivia game request");

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("");
        if action == "submit" {
            let name = find_json_str(body, "name").unwrap_or("Anonymous");
            let answers_str = find_json_str(body, "answers").unwrap_or("");
            // Parse answers: "0,1,2,3,..."
            let ab = answers_str.as_bytes();
            let mut score = 0u32;
            let mut qi = 0u32;
            let mut start = 0;
            let mut idx = 0;
            while idx <= ab.len() && qi < 10 {
                if idx == ab.len() || ab[idx] == b',' {
                    if idx > start {
                        let ans_s = unsafe { core::str::from_utf8_unchecked(&ab[start..idx]) };
                        let ans = parse_u32(ans_s);
                        if ans == QUESTIONS[qi as usize].answer {
                            score += 1;
                        }
                    }
                    qi += 1;
                    start = idx + 1;
                }
                idx += 1;
            }

            // Save to leaderboard
            let lb_count = get_leaderboard_count();
            let mut kbuf = [0u8; 24];
            let prefix = b"tv_lb_";
            kbuf[..prefix.len()].copy_from_slice(prefix);
            let cs = num_to_str(lb_count);
            let cb = cs.as_bytes();
            kbuf[prefix.len()..prefix.len()+cb.len()].copy_from_slice(cb);
            let key = unsafe { core::str::from_utf8_unchecked(&kbuf[..prefix.len()+cb.len()]) };
            // Store "name|score"
            let mut val = [0u8; 256];
            let nb = name.as_bytes();
            val[..nb.len()].copy_from_slice(nb);
            val[nb.len()] = b'|';
            let ss = num_to_str(score);
            let sb = ss.as_bytes();
            val[nb.len()+1..nb.len()+1+sb.len()].copy_from_slice(sb);
            let vstr = unsafe { core::str::from_utf8_unchecked(&val[..nb.len()+1+sb.len()]) };
            kv_write(key, vstr);
            kv_write("tv_lb_count", num_to_str(lb_count + 1));

            // Return score
            let mut rbuf = [0u8; 32];
            let pre = b"{\"score\":";
            rbuf[..pre.len()].copy_from_slice(pre);
            let scs = num_to_str(score);
            let scb = scs.as_bytes();
            rbuf[pre.len()..pre.len()+scb.len()].copy_from_slice(scb);
            rbuf[pre.len()+scb.len()] = b'}';
            let rstr = unsafe { core::str::from_utf8_unchecked(&rbuf[..pre.len()+scb.len()+1]) };
            respond(200, rstr, "application/json");
        } else {
            respond(400, r##"{"error":"unknown action"}"##, "application/json");
        }
        return;
    }

    render_quiz();
}
