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

// Trivia questions: question|optA|optB|optC|optD|correct_idx(0-3)
static QUESTIONS: &[&str] = &[
    "What planet is known as the Red Planet?|Venus|Mars|Jupiter|Saturn|1",
    "What is the largest ocean on Earth?|Atlantic|Indian|Pacific|Arctic|2",
    "Who painted the Mona Lisa?|Michelangelo|Da Vinci|Raphael|Donatello|1",
    "What is the chemical symbol for gold?|Ag|Fe|Au|Cu|2",
    "How many bones are in the adult human body?|186|206|226|246|1",
    "What year did the Titanic sink?|1910|1912|1914|1916|1",
    "What is the smallest prime number?|0|1|2|3|2",
    "Which element has atomic number 1?|Helium|Hydrogen|Lithium|Carbon|1",
    "What is the capital of Australia?|Sydney|Melbourne|Canberra|Perth|2",
    "How many continents are there?|5|6|7|8|2",
    "What language has the most native speakers?|English|Spanish|Mandarin|Hindi|2",
    "What is the speed of light in km/s?|200000|300000|400000|500000|1",
    "Who wrote Romeo and Juliet?|Dickens|Shakespeare|Austen|Twain|1",
    "What is the hardest natural substance?|Ruby|Sapphire|Diamond|Emerald|2",
    "Which country has the most people?|USA|India|China|Indonesia|1",
    "What gas do plants absorb?|Oxygen|Nitrogen|CO2|Hydrogen|2",
    "How many strings does a violin have?|3|4|5|6|1",
    "What is the boiling point of water in Celsius?|90|95|100|105|2",
    "Who discovered penicillin?|Pasteur|Fleming|Koch|Jenner|1",
    "What is the largest mammal?|Elephant|Blue Whale|Giraffe|Hippo|1",
];

fn render_game() {
    let games_played = parse_u32(kv_read("trivia_games").unwrap_or("0"));
    let high_score = kv_read("trivia_highscore").unwrap_or("0");
    let high_name = kv_read("trivia_highname").unwrap_or("Nobody");

    let mut p = 0;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Trivia Game</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:'Segoe UI',sans-serif;background:linear-gradient(135deg,#1a1a2e,#16213e,#0f3460);color:#e0e0e0;min-height:100vh;display:flex;justify-content:center;padding:20px}
.container{max-width:700px;width:100%}
h1{color:#e94560;text-align:center;font-size:2.2em;margin-bottom:5px}
.subtitle{color:#8b949e;text-align:center;margin-bottom:25px}
.stats-bar{display:flex;gap:15px;justify-content:center;margin-bottom:25px}
.stat-pill{background:rgba(255,255,255,0.1);padding:8px 20px;border-radius:20px;font-size:0.9em}
.stat-pill strong{color:#e94560}
.game-area{background:rgba(255,255,255,0.05);border:1px solid rgba(255,255,255,0.1);border-radius:15px;padding:30px}
.question-num{color:#e94560;font-size:0.9em;margin-bottom:10px;font-weight:600}
.question{font-size:1.4em;font-weight:600;margin-bottom:25px;line-height:1.4}
.options{display:grid;grid-template-columns:1fr 1fr;gap:12px}
.option{background:rgba(255,255,255,0.08);border:2px solid rgba(255,255,255,0.15);border-radius:10px;padding:15px;cursor:pointer;font-size:1.05em;transition:all 0.3s;text-align:center}
.option:hover{background:rgba(233,69,96,0.2);border-color:#e94560}
.option.correct{background:rgba(46,204,113,0.3);border-color:#2ecc71}
.option.wrong{background:rgba(231,76,60,0.3);border-color:#e74c3c}
.option.disabled{pointer-events:none;opacity:0.7}
.progress{height:6px;background:rgba(255,255,255,0.1);border-radius:3px;margin-bottom:20px;overflow:hidden}
.progress-fill{height:100%;background:linear-gradient(90deg,#e94560,#0f3460);transition:width 0.5s;border-radius:3px}
.score-display{text-align:center;font-size:1.2em;margin:15px 0}
.score-display strong{color:#e94560;font-size:1.5em}
.result-area{text-align:center;padding:20px}
.result-area h2{font-size:2em;margin-bottom:10px}
.result-area .final-score{font-size:4em;font-weight:bold;color:#e94560}
.result-area .grade{font-size:1.3em;margin:10px 0;color:#2ecc71}
.btn{background:#e94560;color:#fff;border:none;padding:12px 30px;border-radius:8px;cursor:pointer;font-size:1.1em;font-weight:600;margin-top:15px}
.btn:hover{background:#c13552}
.feedback{text-align:center;margin-top:15px;font-size:1.1em;min-height:30px}
.leaderboard{background:rgba(255,255,255,0.05);border:1px solid rgba(255,255,255,0.1);border-radius:15px;padding:20px;margin-top:25px}
.leaderboard h2{color:#e94560;margin-bottom:15px}
.lb-row{display:flex;justify-content:space-between;padding:8px 10px;border-bottom:1px solid rgba(255,255,255,0.05)}
.lb-row .rank{color:#f0883e;width:30px}
.lb-row .score{color:#2ecc71;font-weight:bold}
.name-input{background:rgba(255,255,255,0.1);color:#e0e0e0;border:1px solid rgba(255,255,255,0.2);padding:10px;border-radius:5px;font-size:1em;margin-right:10px}
</style></head><body>
<div class="container">
<h1>Trivia Challenge</h1>
<p class="subtitle">Test your knowledge - 10 questions per round</p>
<div class="stats-bar">
<div class="stat-pill">Games: <strong>"##);
    p = buf_write(p, num_to_str(games_played));
    p = buf_write(p, r##"</strong></div>
<div class="stat-pill">High Score: <strong>"##);
    p = buf_write(p, high_score);
    p = buf_write(p, r##"/10</strong> by "##);
    p = buf_write(p, high_name);
    p = buf_write(p, r##"</div></div>
<div class="game-area" id="game-area">
<div class="progress"><div class="progress-fill" id="progress" style="width:0%"></div></div>
<div class="question-num" id="q-num">Question 1 of 10</div>
<div class="question" id="question">Loading...</div>
<div class="score-display">Score: <strong id="score">0</strong>/10</div>
<div class="options" id="options"></div>
<div class="feedback" id="feedback"></div>
</div>
<div class="leaderboard"><h2>Top Scores</h2><div id="lb">"##);

    let lb = kv_read("trivia_lb").unwrap_or("");
    if lb.is_empty() {
        p = buf_write(p, r##"<p style="color:#8b949e;text-align:center">No scores yet.</p>"##);
    } else {
        let lbb = lb.as_bytes();
        let mut es = 0;
        let mut ei = 0;
        let mut rank: u32 = 1;
        while ei <= lbb.len() && rank <= 10 {
            if ei == lbb.len() || lbb[ei] == b';' {
                if ei > es {
                    if let Ok(entry) = core::str::from_utf8(&lbb[es..ei]) {
                        let eb = entry.as_bytes();
                        if let Some(cp) = eb.iter().position(|&b| b == b':') {
                            let name = core::str::from_utf8(&eb[..cp]).unwrap_or("?");
                            let sc = core::str::from_utf8(&eb[cp+1..]).unwrap_or("0");
                            p = buf_write(p, r##"<div class="lb-row"><span class="rank">"##);
                            p = buf_write(p, num_to_str(rank));
                            p = buf_write(p, r##".</span><span>"##);
                            p = buf_write(p, name);
                            p = buf_write(p, r##"</span><span class="score">"##);
                            p = buf_write(p, sc);
                            p = buf_write(p, r##"/10</span></div>"##);
                            rank += 1;
                        }
                    }
                }
                es = ei + 1;
            }
            ei += 1;
        }
    }

    p = buf_write(p, r##"</div></div></div>
<script>
const questions=["##);

    let mut qi = 0;
    while qi < QUESTIONS.len() {
        if qi > 0 { p = buf_write(p, ","); }
        p = buf_write(p, r##"""##);
        p = buf_write(p, QUESTIONS[qi]);
        p = buf_write(p, r##"""##);
        qi += 1;
    }

    p = buf_write(p, r##"];
let gameQs=[];
let current=0;
let score=0;
let answered=false;

function shuffle(a){for(let i=a.length-1;i>0;i--){const j=Math.floor(Math.random()*(i+1));[a[i],a[j]]=[a[j],a[i]];}return a;}

function startGame(){
  gameQs=shuffle([...Array(questions.length).keys()]).slice(0,10);
  current=0;score=0;answered=false;
  document.getElementById('score').textContent='0';
  showQuestion();
}

function showQuestion(){
  if(current>=10){showResult();return;}
  answered=false;
  const q=questions[gameQs[current]];
  const parts=q.split('|');
  document.getElementById('q-num').textContent='Question '+(current+1)+' of 10';
  document.getElementById('question').textContent=parts[0];
  document.getElementById('progress').style.width=(current*10)+'%';
  document.getElementById('feedback').textContent='';
  let html='';
  for(let i=0;i<4;i++){
    html+='<div class="option" onclick="answer('+i+','+parts[5]+')">'+parts[i+1]+'</div>';
  }
  document.getElementById('options').innerHTML=html;
}

function answer(idx,correct){
  if(answered)return;
  answered=true;
  const opts=document.querySelectorAll('.option');
  opts.forEach((o,i)=>{
    o.classList.add('disabled');
    if(i===correct)o.classList.add('correct');
    if(i===idx&&i!==correct)o.classList.add('wrong');
  });
  if(idx===correct){
    score++;
    document.getElementById('score').textContent=score;
    document.getElementById('feedback').innerHTML='<span style="color:#2ecc71">Correct!</span>';
  }else{
    document.getElementById('feedback').innerHTML='<span style="color:#e74c3c">Wrong! The answer was: '+document.querySelectorAll('.option')[correct].textContent+'</span>';
  }
  setTimeout(()=>{current++;showQuestion();},1800);
}

function showResult(){
  const grade=score>=9?'Genius!':score>=7?'Great job!':score>=5?'Not bad!':score>=3?'Keep trying!':'Better luck next time!';
  document.getElementById('game-area').innerHTML='<div class="result-area"><h2>Round Complete!</h2><div class="final-score">'+score+'</div><p>out of 10</p><div class="grade">'+grade+'</div><div><input class="name-input" type="text" id="pname" placeholder="Your name" maxlength="20"><button class="btn" onclick="saveAndRestart()">Save Score</button></div><button class="btn" style="background:#333" onclick="location.reload()">Play Again</button></div>';
}

function saveAndRestart(){
  const name=document.getElementById('pname').value||'Anonymous';
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'save',name:name,score:''+score})})
  .then(()=>location.reload());
}

startGame();
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
        if action == "save" {
            let name = find_json_str(body, "name").unwrap_or("Anon");
            let score_str = find_json_str(body, "score").unwrap_or("0");
            let score = parse_u32(score_str);

            let games = parse_u32(kv_read("trivia_games").unwrap_or("0")) + 1;
            kv_write("trivia_games", num_to_str(games));

            let current_high = parse_u32(kv_read("trivia_highscore").unwrap_or("0"));
            if score > current_high {
                kv_write("trivia_highscore", score_str);
                kv_write("trivia_highname", name);
            }

            let existing = kv_read("trivia_lb").unwrap_or("");
            let mut entry = [0u8; 64];
            let nb = name.as_bytes();
            let sb = score_str.as_bytes();
            entry[..nb.len()].copy_from_slice(nb);
            entry[nb.len()] = b':';
            entry[nb.len()+1..nb.len()+1+sb.len()].copy_from_slice(sb);
            let elen = nb.len() + 1 + sb.len();
            let entry_str = unsafe { core::str::from_utf8_unchecked(&entry[..elen]) };

            if existing.is_empty() {
                kv_write("trivia_lb", entry_str);
            } else {
                let mut nbuf = [0u8; 4096];
                nbuf[..elen].copy_from_slice(&entry[..elen]);
                nbuf[elen] = b';';
                let eb = existing.as_bytes();
                let copy_len = eb.len().min(4096 - elen - 1);
                nbuf[elen+1..elen+1+copy_len].copy_from_slice(&eb[..copy_len]);
                let nv = unsafe { core::str::from_utf8_unchecked(&nbuf[..elen+1+copy_len]) };
                kv_write("trivia_lb", nv);
            }

            respond(200, r##"{"ok":true}"##, "application/json");
        } else {
            respond(400, r##"{"error":"unknown"}"##, "application/json");
        }
        return;
    }

    render_game();
}