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
    unsafe {
        response(status, body.as_ptr(), body.len() as i32, content_type.as_ptr(), content_type.len() as i32);
    }
}

fn host_log(level: i32, msg: &str) {
    unsafe { log(level, msg.as_ptr(), msg.len() as i32); }
}

fn find_json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let key_bytes = key.as_bytes();
    let json_bytes = json.as_bytes();
    let mut i = 0;
    while i + key_bytes.len() + 3 < json_bytes.len() {
        if json_bytes[i] == b'"' {
            let start = i + 1;
            if start + key_bytes.len() < json_bytes.len()
                && &json_bytes[start..start + key_bytes.len()] == key_bytes
                && json_bytes[start + key_bytes.len()] == b'"'
            {
                let mut j = start + key_bytes.len() + 1;
                while j < json_bytes.len() && (json_bytes[j] == b':' || json_bytes[j] == b' ') { j += 1; }
                if j < json_bytes.len() && json_bytes[j] == b'"' {
                    let val_start = j + 1;
                    let mut val_end = val_start;
                    while val_end < json_bytes.len() && json_bytes[val_end] != b'"' { val_end += 1; }
                    return core::str::from_utf8(&json_bytes[val_start..val_end]).ok();
                }
            }
        }
        i += 1;
    }
    None
}

fn kv_read(key: &str) -> Option<&'static str> {
    unsafe {
        let result = kv_get(key.as_ptr(), key.len() as i32);
        if result < 0 { return None; }
        let ptr = (result >> 32) as *const u8;
        let len = (result & 0xFFFFFFFF) as usize;
        let bytes = core::slice::from_raw_parts(ptr, len);
        core::str::from_utf8(bytes).ok()
    }
}

fn kv_write(key: &str, value: &str) {
    unsafe {
        kv_set(key.as_ptr(), key.len() as i32, value.as_ptr(), value.len() as i32);
    }
}

static mut BUF: [u8; 65536] = [0u8; 65536];

struct BufWriter {
    pos: usize,
}

impl BufWriter {
    fn new() -> Self { Self { pos: 0 } }

    fn push_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        unsafe {
            let end = (self.pos + bytes.len()).min(BUF.len());
            let copy_len = end - self.pos;
            BUF[self.pos..end].copy_from_slice(&bytes[..copy_len]);
            self.pos = end;
        }
    }

    fn push_num(&mut self, mut n: u32) {
        if n == 0 { self.push_str("0"); return; }
        let mut digits = [0u8; 10];
        let mut i = 0;
        while n > 0 { digits[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
        while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = digits[i]; self.pos += 1; } } }
    }

    fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) }
    }
}

fn parse_num(s: &str) -> u32 {
    let mut n: u32 = 0;
    for &b in s.as_bytes() {
        if b >= b'0' && b <= b'9' { n = n * 10 + (b - b'0') as u32; }
    }
    n
}

fn num_to_str<'a>(buf: &'a mut [u8; 10], mut n: u32) -> &'a str {
    if n == 0 { buf[0] = b'0'; return unsafe { core::str::from_utf8_unchecked(&buf[..1]) }; }
    let mut pos = 0;
    while n > 0 { buf[pos] = b'0' + (n % 10) as u8; n /= 10; pos += 1; }
    buf[..pos].reverse();
    unsafe { core::str::from_utf8_unchecked(&buf[..pos]) }
}

#[no_mangle]
pub extern "C" fn handle(method_ptr: *const u8, method_len: i32, _path_ptr: *const u8, _path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if method.as_bytes()[0] == b'P' {
        // POST: record answer and return next question / results
        if let Some(action) = find_json_str(body, "action") {
            if action.as_bytes() == b"answer" {
                let q_idx = parse_num(find_json_str(body, "question").unwrap_or("0"));
                let answer = parse_num(find_json_str(body, "answer_idx").unwrap_or("99"));
                let correct_answers: [u32; 8] = [1, 2, 0, 3, 1, 2, 0, 3];
                let is_correct = if (q_idx as usize) < 8 { answer == correct_answers[q_idx as usize] } else { false };

                let score = parse_num(kv_read("quiz_score").unwrap_or("0"));
                if is_correct {
                    let mut nb = [0u8; 10];
                    kv_write("quiz_score", num_to_str(&mut nb, score + 1));
                }
                let mut nb2 = [0u8; 10];
                kv_write("quiz_question", num_to_str(&mut nb2, q_idx + 1));

                let mut w = BufWriter::new();
                w.push_str("{\"correct\":");
                if is_correct { w.push_str("true"); } else { w.push_str("false"); }
                w.push_str(",\"score\":");
                w.push_num(if is_correct { score + 1 } else { score });
                w.push_str(",\"next\":");
                w.push_num(q_idx + 1);
                w.push_str(",\"total\":8}");
                respond(200, w.as_str(), "application/json");
            } else if action.as_bytes() == b"reset" {
                kv_write("quiz_score", "0");
                kv_write("quiz_question", "0");
                respond(200, "{\"ok\":true}", "application/json");
            } else {
                respond(400, "{\"error\":\"unknown\"}", "application/json");
            }
        } else {
            respond(400, "{\"error\":\"missing action\"}", "application/json");
        }
        return;
    }

    // Track attempts
    let attempts = parse_num(kv_read("quiz_attempts").unwrap_or("0"));

    let mut w = BufWriter::new();
    w.push_str("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Quiz</title><style>");
    w.push_str("*{margin:0;padding:0;box-sizing:border-box}");
    w.push_str("body{background:#0f172a;color:#e2e8f0;font-family:'Segoe UI',sans-serif;min-height:100vh;display:flex;justify-content:center;padding:40px 20px}");
    w.push_str(".container{max-width:640px;width:100%}");
    w.push_str("h1{text-align:center;color:#38bdf8;margin-bottom:8px;font-size:2em}");
    w.push_str(".subtitle{text-align:center;color:#64748b;margin-bottom:32px}");
    w.push_str(".progress{background:#1e293b;border-radius:10px;height:12px;margin-bottom:24px;overflow:hidden}");
    w.push_str(".progress-bar{height:100%;background:linear-gradient(90deg,#38bdf8,#818cf8);border-radius:10px;transition:width 0.5s}");
    w.push_str(".question-card{background:#1e293b;border-radius:16px;padding:32px;margin-bottom:24px}");
    w.push_str(".q-number{color:#38bdf8;font-size:14px;font-weight:bold;text-transform:uppercase;letter-spacing:2px;margin-bottom:12px}");
    w.push_str(".q-text{font-size:20px;line-height:1.5;margin-bottom:24px;font-weight:500}");
    w.push_str(".options{display:flex;flex-direction:column;gap:10px}");
    w.push_str(".option{padding:14px 20px;background:#0f172a;border:2px solid #334155;border-radius:10px;cursor:pointer;font-size:16px;transition:all 0.2s;text-align:left}");
    w.push_str(".option:hover{border-color:#38bdf8;background:#1a2744}");
    w.push_str(".option.correct{border-color:#22c55e;background:#22c55e22;color:#22c55e}");
    w.push_str(".option.wrong{border-color:#ef4444;background:#ef444422;color:#ef4444}");
    w.push_str(".option.disabled{pointer-events:none;opacity:0.6}");
    w.push_str(".results{text-align:center;padding:40px}");
    w.push_str(".score-circle{width:160px;height:160px;border-radius:50%;margin:0 auto 24px;display:flex;align-items:center;justify-content:center;font-size:48px;font-weight:bold}");
    w.push_str(".score-good{background:#22c55e33;color:#22c55e;border:4px solid #22c55e}");
    w.push_str(".score-ok{background:#f59e0b33;color:#f59e0b;border:4px solid #f59e0b}");
    w.push_str(".score-bad{background:#ef444433;color:#ef4444;border:4px solid #ef4444}");
    w.push_str(".restart-btn{padding:14px 32px;background:#38bdf8;color:#0f172a;border:none;border-radius:10px;font-size:16px;font-weight:bold;cursor:pointer;margin-top:20px}");
    w.push_str(".stats{text-align:center;color:#64748b;margin-top:12px;font-size:14px}");
    w.push_str(".feedback{padding:12px 16px;border-radius:8px;margin-top:16px;font-size:15px;text-align:center}");
    w.push_str(".feedback.right{background:#22c55e22;color:#22c55e}");
    w.push_str(".feedback.wrong{background:#ef444422;color:#ef4444}");
    w.push_str("</style></head><body><div class='container'>");
    w.push_str("<h1>Knowledge Quiz</h1><p class='subtitle'>Test your general knowledge</p>");
    w.push_str("<div class='stats'>");
    w.push_num(attempts);
    w.push_str(" quizzes taken</div>");
    w.push_str("<div class='progress'><div class='progress-bar' id='progressBar' style='width:0%'></div></div>");
    w.push_str("<div id='quizArea'></div></div>");

    w.push_str("<script>");
    w.push_str("const BASE=location.pathname;");
    w.push_str("const questions=[");
    w.push_str("{q:'What is the capital of Japan?',opts:['Beijing','Tokyo','Seoul','Bangkok'],correct:1},");
    w.push_str("{q:'Which planet is known as the Red Planet?',opts:['Venus','Jupiter','Mars','Saturn'],correct:2},");
    w.push_str("{q:'What is the chemical symbol for gold?',opts:['Au','Ag','Fe','Cu'],correct:0},");
    w.push_str("{q:'Who painted the Mona Lisa?',opts:['Michelangelo','Van Gogh','Picasso','Da Vinci'],correct:3},");
    w.push_str("{q:'What is the largest ocean on Earth?',opts:['Atlantic','Pacific','Indian','Arctic'],correct:1},");
    w.push_str("{q:'In what year did World War II end?',opts:['1943','1944','1945','1946'],correct:2},");
    w.push_str("{q:'What is the speed of light in km/s (approx)?',opts:['300,000','150,000','500,000','1,000,000'],correct:0},");
    w.push_str("{q:'Which element has atomic number 1?',opts:['Helium','Oxygen','Carbon','Hydrogen'],correct:3}");
    w.push_str("];");
    w.push_str("let current=0,score=0,answered=false;");
    w.push_str("function render(){const area=document.getElementById('quizArea');document.getElementById('progressBar').style.width=(current/questions.length*100)+'%';");
    w.push_str("if(current>=questions.length){area.innerHTML=renderResults();return;}");
    w.push_str("const q=questions[current];let html='<div class=\"question-card\"><div class=\"q-number\">Question '+(current+1)+' of '+questions.length+'</div>';");
    w.push_str("html+='<div class=\"q-text\">'+q.q+'</div><div class=\"options\">';");
    w.push_str("q.opts.forEach((opt,i)=>{html+='<button class=\"option\" id=\"opt'+i+'\" onclick=\"answer('+i+')\">'+opt+'</button>';});");
    w.push_str("html+='</div><div id=\"feedback\"></div></div>';area.innerHTML=html;answered=false;}");
    w.push_str("async function answer(idx){if(answered)return;answered=true;const q=questions[current];const correct=q.correct===idx;");
    w.push_str("document.getElementById('opt'+idx).classList.add(correct?'correct':'wrong');");
    w.push_str("document.getElementById('opt'+q.correct).classList.add('correct');");
    w.push_str("document.querySelectorAll('.option').forEach(el=>el.classList.add('disabled'));");
    w.push_str("if(correct)score++;");
    w.push_str("document.getElementById('feedback').innerHTML='<div class=\"feedback '+(correct?'right':'wrong')+'\">'+(correct?'Correct!':'Wrong! The answer was: '+q.opts[q.correct])+'</div>';");
    w.push_str("await fetch(BASE,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'answer',question:String(current),answer_idx:String(idx)})});");
    w.push_str("setTimeout(()=>{current++;render();},1500);}");
    w.push_str("function renderResults(){const pct=Math.round(score/questions.length*100);let cls='score-bad';if(pct>=75)cls='score-good';else if(pct>=50)cls='score-ok';");
    w.push_str("return '<div class=\"results\"><div class=\"score-circle '+cls+'\">'+score+'/'+questions.length+'</div><h2>'+pct+'% Correct</h2><p style=\"color:#64748b;margin-top:8px\">'+(pct>=75?'Excellent!':pct>=50?'Good effort!':'Keep studying!')+'</p><button class=\"restart-btn\" onclick=\"restart()\">Try Again</button></div>';}");
    w.push_str("async function restart(){await fetch(BASE,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'reset'})});current=0;score=0;render();}");
    w.push_str("render();");
    w.push_str("</script></body></html>");

    respond(200, w.as_str(), "text/html");
}
