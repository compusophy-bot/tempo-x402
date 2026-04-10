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

fn render_page() {
    let leaderboard = kv_read("typing_leaderboard").unwrap_or("");
    let mut p = 0;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Typing Speed Test</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:'Courier New',monospace;background:#0a0e17;color:#c9d1d9;min-height:100vh;display:flex;flex-direction:column;align-items:center;padding:30px}
h1{color:#58a6ff;font-size:2em;margin-bottom:10px}
.subtitle{color:#8b949e;margin-bottom:30px}
.test-area{background:#161b22;border:2px solid #30363d;border-radius:12px;padding:30px;width:100%;max-width:800px;margin-bottom:20px}
.text-display{font-size:1.3em;line-height:1.8;margin-bottom:20px;min-height:100px;letter-spacing:0.5px}
.text-display .correct{color:#3fb950}
.text-display .wrong{color:#f85149;text-decoration:underline}
.text-display .current{background:#58a6ff;color:#0a0e17;padding:0 2px;border-radius:2px}
.text-display .pending{color:#6e7681}
#input-area{width:100%;background:#0d1117;color:#c9d1d9;border:2px solid #30363d;padding:15px;font-size:1.2em;font-family:'Courier New',monospace;border-radius:8px;outline:none}
#input-area:focus{border-color:#58a6ff}
.stats{display:flex;gap:20px;justify-content:center;margin:20px 0}
.stat{background:#161b22;border:1px solid #30363d;border-radius:8px;padding:15px 25px;text-align:center}
.stat .value{font-size:2em;color:#58a6ff;font-weight:bold}
.stat .label{color:#8b949e;font-size:0.85em;margin-top:5px}
.controls{display:flex;gap:10px;margin:15px 0}
.btn{background:#238636;color:#fff;border:none;padding:10px 25px;border-radius:6px;cursor:pointer;font-size:1em;font-weight:600}
.btn:hover{background:#2ea043}
.btn.secondary{background:#30363d}
.btn.secondary:hover{background:#484f58}
.leaderboard{background:#161b22;border:1px solid #30363d;border-radius:12px;padding:20px;width:100%;max-width:800px;margin-top:20px}
.leaderboard h2{color:#58a6ff;margin-bottom:15px}
.lb-entry{display:flex;justify-content:space-between;padding:8px 12px;border-bottom:1px solid #21262d}
.lb-entry .rank{color:#f0883e;font-weight:bold;width:30px}
.lb-entry .name{color:#c9d1d9;flex:1}
.lb-entry .wpm{color:#3fb950;font-weight:bold}
.lb-entry .acc{color:#8b949e;margin-left:15px}
.result-modal{display:none;position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(0,0,0,0.8);z-index:100;align-items:center;justify-content:center}
.result-box{background:#161b22;border:2px solid #58a6ff;border-radius:12px;padding:40px;text-align:center;max-width:500px}
.result-box h2{color:#58a6ff;font-size:1.8em;margin-bottom:20px}
.result-box .big-wpm{font-size:4em;color:#3fb950;font-weight:bold}
.save-form{margin-top:20px}
.save-form input{background:#0d1117;color:#c9d1d9;border:1px solid #30363d;padding:10px;border-radius:5px;font-size:1em;margin-right:10px}
</style></head><body>
<h1>Typing Speed Test</h1>
<p class="subtitle">Test your typing speed and accuracy</p>
<div class="stats">
  <div class="stat"><div class="value" id="wpm">0</div><div class="label">WPM</div></div>
  <div class="stat"><div class="value" id="accuracy">100%</div><div class="label">Accuracy</div></div>
  <div class="stat"><div class="value" id="timer">0:00</div><div class="label">Time</div></div>
  <div class="stat"><div class="value" id="chars">0</div><div class="label">Characters</div></div>
</div>
<div class="test-area">
  <div class="text-display" id="display"></div>
  <textarea id="input-area" placeholder="Start typing to begin..." rows="3"></textarea>
</div>
<div class="controls">
  <button class="btn" onclick="newTest()">New Test</button>
  <button class="btn secondary" onclick="newTest('short')">Short</button>
  <button class="btn secondary" onclick="newTest('long')">Long</button>
</div>
<div class="result-modal" id="result-modal">
  <div class="result-box">
    <h2>Test Complete!</h2>
    <div class="big-wpm" id="final-wpm">0</div>
    <p style="color:#8b949e">words per minute</p>
    <div class="stats" style="margin:20px 0">
      <div class="stat"><div class="value" id="final-acc">0%</div><div class="label">Accuracy</div></div>
      <div class="stat"><div class="value" id="final-time">0s</div><div class="label">Time</div></div>
    </div>
    <div class="save-form">
      <input type="text" id="player-name" placeholder="Your name" maxlength="20">
      <button class="btn" onclick="saveScore()">Save Score</button>
    </div>
    <button class="btn secondary" onclick="closeResult()" style="margin-top:10px">Close</button>
  </div>
</div>
<div class="leaderboard"><h2>Leaderboard</h2><div id="lb-list">"##);

    // Render leaderboard entries
    if leaderboard.is_empty() {
        p = buf_write(p, r##"<p style="color:#8b949e;text-align:center">No scores yet. Be the first!</p>"##);
    } else {
        let lb = leaderboard.as_bytes();
        let mut start = 0;
        let mut rank: u32 = 1;
        let mut idx = 0;
        while idx <= lb.len() && rank <= 10 {
            if idx == lb.len() || lb[idx] == b';' {
                if idx > start {
                    // entry = "name:wpm:accuracy"
                    if let Ok(entry) = core::str::from_utf8(&lb[start..idx]) {
                        let eb = entry.as_bytes();
                        let mut c1 = 0;
                        while c1 < eb.len() && eb[c1] != b':' { c1 += 1; }
                        let mut c2 = c1 + 1;
                        while c2 < eb.len() && eb[c2] != b':' { c2 += 1; }
                        if c1 < eb.len() && c2 < eb.len() {
                            let name = core::str::from_utf8(&eb[..c1]).unwrap_or("?");
                            let wpm = core::str::from_utf8(&eb[c1+1..c2]).unwrap_or("0");
                            let acc = core::str::from_utf8(&eb[c2+1..]).unwrap_or("0");
                            p = buf_write(p, r##"<div class="lb-entry"><span class="rank">##"##);
                            p = buf_write(p, num_to_str(rank));
                            p = buf_write(p, r##"</span><span class="name">"##);
                            p = buf_write(p, name);
                            p = buf_write(p, r##"</span><span class="wpm">"##);
                            p = buf_write(p, wpm);
                            p = buf_write(p, r##" WPM</span><span class="acc">"##);
                            p = buf_write(p, acc);
                            p = buf_write(p, r##"%</span></div>"##);
                            rank += 1;
                        }
                    }
                }
                start = idx + 1;
            }
            idx += 1;
        }
    }

    p = buf_write(p, r##"</div></div>
<script>
const texts={
short:["The quick brown fox jumps over the lazy dog near the riverbank.",
"Pack my box with five dozen liquor jugs and send them quickly.",
"How vexingly quick daft zebras jump over the sleeping wolves."],
medium:["Programming is the art of telling another human being what one wants the computer to do. It requires patience, logic, and creativity to write clean and maintainable code that solves real problems efficiently.",
"The best way to predict the future is to invent it. Every line of code we write shapes the digital world around us. Technology is not just a tool but an extension of human thought and ambition.",
"In the beginning was the command line. Developers across the world type millions of lines of code each day, building systems that power everything from simple websites to complex artificial intelligence."],
long:["Software engineering is fundamentally about managing complexity. As systems grow larger, the interactions between components become exponentially more difficult to reason about. Good architecture patterns like separation of concerns, dependency injection, and event-driven design help us tame this complexity. The best engineers are those who can hold multiple levels of abstraction in their mind simultaneously while making decisions that will remain sound as requirements evolve over months and years of continued development."]
};
let currentText='';
let startTime=0;
let timerInterval=null;
let finished=false;
let lastWpm=0;
let lastAcc=0;
function newTest(len){
  len=len||'medium';
  const arr=texts[len]||texts.medium;
  currentText=arr[Math.floor(Math.random()*arr.length)];
  document.getElementById('input-area').value='';
  document.getElementById('input-area').disabled=false;
  finished=false;startTime=0;
  if(timerInterval)clearInterval(timerInterval);
  updateDisplay('');
  document.getElementById('wpm').textContent='0';
  document.getElementById('accuracy').textContent='100%';
  document.getElementById('timer').textContent='0:00';
  document.getElementById('chars').textContent='0';
  document.getElementById('input-area').focus();
}
function updateDisplay(typed){
  let html='';
  for(let i=0;i<currentText.length;i++){
    if(i<typed.length){
      if(typed[i]===currentText[i])html+='<span class="correct">'+esc(currentText[i])+'</span>';
      else html+='<span class="wrong">'+esc(currentText[i])+'</span>';
    }else if(i===typed.length){
      html+='<span class="current">'+esc(currentText[i])+'</span>';
    }else{
      html+='<span class="pending">'+esc(currentText[i])+'</span>';
    }
  }
  document.getElementById('display').innerHTML=html;
}
function esc(c){return c==='<'?'&lt;':c==='>'?'&gt;':c==='&'?'&amp;':c;}
document.getElementById('input-area').addEventListener('input',function(e){
  if(finished||!currentText)return;
  const typed=e.target.value;
  if(!startTime){
    startTime=Date.now();
    timerInterval=setInterval(updateTimer,100);
  }
  updateDisplay(typed);
  const elapsed=(Date.now()-startTime)/1000/60;
  const words=typed.trim().split(/\s+/).filter(w=>w).length;
  const wpm=elapsed>0?Math.round(words/elapsed):0;
  let correct=0;
  for(let i=0;i<typed.length;i++){if(i<currentText.length&&typed[i]===currentText[i])correct++;}
  const acc=typed.length>0?Math.round(correct/typed.length*100):100;
  document.getElementById('wpm').textContent=wpm;
  document.getElementById('accuracy').textContent=acc+'%';
  document.getElementById('chars').textContent=typed.length;
  if(typed.length>=currentText.length){
    finished=true;
    clearInterval(timerInterval);
    lastWpm=wpm;lastAcc=acc;
    document.getElementById('final-wpm').textContent=wpm;
    document.getElementById('final-acc').textContent=acc+'%';
    document.getElementById('final-time').textContent=Math.round((Date.now()-startTime)/1000)+'s';
    document.getElementById('result-modal').style.display='flex';
    document.getElementById('input-area').disabled=true;
  }
});
function updateTimer(){
  if(!startTime)return;
  const s=Math.floor((Date.now()-startTime)/1000);
  const m=Math.floor(s/60);
  const sec=s%60;
  document.getElementById('timer').textContent=m+':'+(sec<10?'0':'')+sec;
}
function saveScore(){
  const name=document.getElementById('player-name').value||'Anonymous';
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'score',name:name,wpm:''+lastWpm,accuracy:''+lastAcc})})
  .then(()=>location.reload());
}
function closeResult(){document.getElementById('result-modal').style.display='none';}
newTest();
</script></body></html>"##);
    respond(200, buf_as_str(p), "text/html");
}

#[no_mangle]
pub extern "C" fn x402_handle(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    host_log(1, "Typing test request");

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("");
        if action == "score" {
            let name = find_json_str(body, "name").unwrap_or("Anon");
            let wpm = find_json_str(body, "wpm").unwrap_or("0");
            let accuracy = find_json_str(body, "accuracy").unwrap_or("0");
            let existing = kv_read("typing_leaderboard").unwrap_or("");
            // Build new entry
            let mut entry = [0u8; 128];
            let nb = name.as_bytes();
            let wb = wpm.as_bytes();
            let ab = accuracy.as_bytes();
            let mut ep = 0;
            entry[..nb.len()].copy_from_slice(nb);
            ep += nb.len();
            entry[ep] = b':'; ep += 1;
            entry[ep..ep+wb.len()].copy_from_slice(wb);
            ep += wb.len();
            entry[ep] = b':'; ep += 1;
            entry[ep..ep+ab.len()].copy_from_slice(ab);
            ep += ab.len();
            let entry_str = unsafe { core::str::from_utf8_unchecked(&entry[..ep]) };

            // Prepend to leaderboard (simple: newest first, sorted by WPM client-side would be better but keep server simple)
            let mut new_lb = [0u8; 4096];
            let mut lp = 0;
            new_lb[..ep].copy_from_slice(&entry[..ep]);
            lp += ep;
            if !existing.is_empty() {
                new_lb[lp] = b';'; lp += 1;
                let eb = existing.as_bytes();
                let copy_len = eb.len().min(4096 - lp);
                new_lb[lp..lp+copy_len].copy_from_slice(&eb[..copy_len]);
                lp += copy_len;
            }
            let new_lb_str = unsafe { core::str::from_utf8_unchecked(&new_lb[..lp]) };
            kv_write("typing_leaderboard", new_lb_str);
            respond(200, r##"{"ok":true}"##, "application/json");
        } else {
            respond(400, r##"{"error":"unknown"}"##, "application/json");
        }
        return;
    }

    render_page();
}