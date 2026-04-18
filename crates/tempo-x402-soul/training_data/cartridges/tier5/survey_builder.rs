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

fn find_query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    let kb = key.as_bytes();
    let qb = query.as_bytes();
    let mut i = 0;
    while i + kb.len() < qb.len() {
        if (i == 0 || qb[i - 1] == b'&' || qb[i - 1] == b'?') && &qb[i..i + kb.len()] == kb && i + kb.len() < qb.len() && qb[i + kb.len()] == b'=' {
            let vs = i + kb.len() + 1;
            let mut ve = vs;
            while ve < qb.len() && qb[ve] != b'&' { ve += 1; }
            return core::str::from_utf8(&qb[vs..ve]).ok();
        }
        i += 1;
    }
    None
}

// Survey: surv_{id}_title, surv_{id}_questions = "q1|opt1,opt2,opt3;q2|opt1,opt2;..."
// Results: surv_{id}_results_{q_idx}_{opt_idx} = count
// surv_count = total surveys, surv_{id}_responses = total response count

fn make_surv_key(id: u32, suffix: &str) -> &'static str {
    static mut SKBUF: [u8; 48] = [0u8; 48];
    let prefix = b"surv_";
    let id_s = num_to_str(id);
    let ib = id_s.as_bytes();
    let sb = suffix.as_bytes();
    let mut pos = 0;
    unsafe {
        SKBUF[..prefix.len()].copy_from_slice(prefix);
        pos += prefix.len();
        SKBUF[pos..pos+ib.len()].copy_from_slice(ib);
        pos += ib.len();
        SKBUF[pos] = b'_'; pos += 1;
        SKBUF[pos..pos+sb.len()].copy_from_slice(sb);
        pos += sb.len();
        core::str::from_utf8_unchecked(&SKBUF[..pos])
    }
}

fn make_result_key(surv_id: u32, q_idx: u32, opt_idx: u32) -> &'static str {
    static mut RKBUF: [u8; 48] = [0u8; 48];
    let mut pos = 0;
    let prefix = b"sr_";
    let si = num_to_str(surv_id);
    let qi = num_to_str(q_idx);
    let oi = num_to_str(opt_idx);
    unsafe {
        RKBUF[..prefix.len()].copy_from_slice(prefix);
        pos += prefix.len();
        let sb = si.as_bytes();
        RKBUF[pos..pos+sb.len()].copy_from_slice(sb);
        pos += sb.len();
        RKBUF[pos] = b'_'; pos += 1;
        let qb = qi.as_bytes();
        RKBUF[pos..pos+qb.len()].copy_from_slice(qb);
        pos += qb.len();
        RKBUF[pos] = b'_'; pos += 1;
        let ob = oi.as_bytes();
        RKBUF[pos..pos+ob.len()].copy_from_slice(ob);
        pos += ob.len();
        core::str::from_utf8_unchecked(&RKBUF[..pos])
    }
}

fn render_survey_list() {
    let count = parse_u32(kv_read("surv_count").unwrap_or("0"));
    let mut p = 0;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Survey Builder</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:'Segoe UI',sans-serif;background:#fafbfc;color:#333;min-height:100vh;padding:20px}
.container{max-width:800px;margin:0 auto}
h1{color:#6f42c1;font-size:2em;margin-bottom:5px}
.subtitle{color:#6a737d;margin-bottom:25px}
.create-box{background:#fff;border:2px solid #e1e4e8;border-radius:12px;padding:25px;margin-bottom:25px}
.create-box h2{color:#6f42c1;margin-bottom:15px}
.create-box input{width:100%;padding:12px;border:2px solid #e1e4e8;border-radius:8px;font-size:1em;margin-bottom:10px}
.question-builder{background:#f6f8fa;border-radius:8px;padding:15px;margin-bottom:10px}
.question-builder label{font-weight:600;margin-bottom:5px;display:block}
.question-builder input{margin-bottom:8px}
.btn{background:#6f42c1;color:#fff;border:none;padding:10px 20px;border-radius:8px;cursor:pointer;font-weight:600;font-size:1em}
.btn:hover{background:#5a32a3}
.btn-outline{background:transparent;color:#6f42c1;border:2px solid #6f42c1}
.btn-outline:hover{background:#6f42c1;color:#fff}
.btn-sm{padding:6px 12px;font-size:0.9em}
.survey-list{display:grid;gap:15px}
.survey-card{background:#fff;border:2px solid #e1e4e8;border-radius:12px;padding:20px;transition:border-color 0.2s}
.survey-card:hover{border-color:#6f42c1}
.survey-title{font-size:1.3em;font-weight:600;color:#24292e;margin-bottom:5px}
.survey-meta{color:#6a737d;font-size:0.9em;margin-bottom:10px}
.survey-actions{display:flex;gap:10px}
.questions-list{margin-top:10px}
.q-item{padding:5px 0;color:#6a737d;font-size:0.9em}
</style></head><body>
<div class="container">
<h1>Survey Builder</h1>
<p class="subtitle">Create surveys, collect responses, view results</p>
<div class="create-box">
<h2>Create New Survey</h2>
<input type="text" id="survey-title" placeholder="Survey title">
<div id="questions-container">
<div class="question-builder" id="q-0">
<label>Question 1</label>
<input type="text" class="q-text" placeholder="Question text">
<input type="text" class="q-opts" placeholder="Options (comma separated): Yes, No, Maybe">
</div>
</div>
<div style="display:flex;gap:10px;margin-top:10px">
<button class="btn btn-outline btn-sm" onclick="addQuestion()">+ Add Question</button>
<button class="btn" onclick="createSurvey()">Create Survey</button>
</div>
</div>
<h2 style="margin-bottom:15px;color:#6f42c1">Surveys ("##);
    p = buf_write(p, num_to_str(count));
    p = buf_write(p, r##")</h2><div class="survey-list">"##);

    if count == 0 {
        p = buf_write(p, r##"<p style="color:#6a737d;text-align:center;padding:20px">No surveys yet.</p>"##);
    }
    let mut si: u32 = 0;
    while si < count {
        let title = kv_read(make_surv_key(si, "title")).unwrap_or("Untitled");
        let responses = kv_read(make_surv_key(si, "responses")).unwrap_or("0");
        let questions = kv_read(make_surv_key(si, "questions")).unwrap_or("");
        let q_count = if questions.is_empty() { 0 } else {
            questions.as_bytes().iter().filter(|&&b| b == b';').count() as u32 + 1
        };
        p = buf_write(p, r##"<div class="survey-card"><div class="survey-title">"##);
        p = buf_write(p, title);
        p = buf_write(p, r##"</div><div class="survey-meta">"##);
        p = buf_write(p, num_to_str(q_count));
        p = buf_write(p, r##" questions &middot; "##);
        p = buf_write(p, responses);
        p = buf_write(p, r##" responses</div><div class="survey-actions">
<a class="btn btn-sm" href="?take="##);
        p = buf_write(p, num_to_str(si));
        p = buf_write(p, r##"">Take Survey</a>
<a class="btn btn-outline btn-sm" href="?results="##);
        p = buf_write(p, num_to_str(si));
        p = buf_write(p, r##"">View Results</a></div></div>"##);
        si += 1;
    }

    p = buf_write(p, r##"</div></div>
<script>
let qCount=1;
function addQuestion(){
  const c=document.getElementById('questions-container');
  const d=document.createElement('div');
  d.className='question-builder';
  d.innerHTML='<label>Question '+(qCount+1)+'</label><input type="text" class="q-text" placeholder="Question text"><input type="text" class="q-opts" placeholder="Options (comma separated)">';
  c.appendChild(d);
  qCount++;
}
function createSurvey(){
  const title=document.getElementById('survey-title').value;
  if(!title){alert('Title required');return;}
  const qs=[];
  document.querySelectorAll('.question-builder').forEach(qb=>{
    const t=qb.querySelector('.q-text').value;
    const o=qb.querySelector('.q-opts').value;
    if(t&&o)qs.push(t+'|'+o);
  });
  if(!qs.length){alert('Add at least one question');return;}
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'create',title:title,questions:qs.join(';')})})
  .then(()=>location.reload());
}
</script></body></html>"##);
    respond(200, buf_as_str(p), "text/html");
}

fn render_take_survey(surv_id: u32) {
    let title = kv_read(make_surv_key(surv_id, "title")).unwrap_or("Survey");
    let questions = kv_read(make_surv_key(surv_id, "questions")).unwrap_or("");

    let mut p = 0;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Take: "##);
    p = buf_write(p, title);
    p = buf_write(p, r##"</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:'Segoe UI',sans-serif;background:#fafbfc;color:#333;padding:20px}
.container{max-width:700px;margin:0 auto}
h1{color:#6f42c1;margin-bottom:20px}
.question-card{background:#fff;border:2px solid #e1e4e8;border-radius:12px;padding:20px;margin-bottom:15px}
.q-text{font-size:1.2em;font-weight:600;margin-bottom:12px}
.q-num{color:#6f42c1;font-size:0.85em;margin-bottom:5px}
.option{display:block;padding:10px 15px;margin:5px 0;border:2px solid #e1e4e8;border-radius:8px;cursor:pointer;transition:all 0.2s}
.option:hover{border-color:#6f42c1;background:#f6f0ff}
.option input{margin-right:10px}
.btn{background:#6f42c1;color:#fff;border:none;padding:12px 30px;border-radius:8px;cursor:pointer;font-weight:600;font-size:1.1em}
.btn:hover{background:#5a32a3}
.back{color:#6f42c1;text-decoration:none;display:block;margin-bottom:15px}
</style></head><body>
<div class="container">
<a class="back" href="?">Back to surveys</a>
<h1>"##);
    p = buf_write(p, title);
    p = buf_write(p, r##"</h1><form id="survey-form">"##);

    // Parse and render questions
    let qb = questions.as_bytes();
    let mut es = 0;
    let mut ei = 0;
    let mut qi: u32 = 0;
    while ei <= qb.len() {
        if ei == qb.len() || qb[ei] == b';' {
            if ei > es {
                if let Ok(q_entry) = core::str::from_utf8(&qb[es..ei]) {
                    let qeb = q_entry.as_bytes();
                    if let Some(pp) = qeb.iter().position(|&b| b == b'|') {
                        let q_text = core::str::from_utf8(&qeb[..pp]).unwrap_or("?");
                        let opts_str = core::str::from_utf8(&qeb[pp+1..]).unwrap_or("");
                        p = buf_write(p, r##"<div class="question-card"><div class="q-num">Question "##);
                        p = buf_write(p, num_to_str(qi + 1));
                        p = buf_write(p, r##"</div><div class="q-text">"##);
                        p = buf_write(p, q_text);
                        p = buf_write(p, r##"</div>"##);

                        // Parse options
                        let ob = opts_str.as_bytes();
                        let mut os = 0;
                        let mut oi_idx: u32 = 0;
                        let mut oei = 0;
                        while oei <= ob.len() {
                            if oei == ob.len() || ob[oei] == b',' {
                                if oei > os {
                                    let opt_text = core::str::from_utf8(&ob[os..oei]).unwrap_or("?");
                                    let opt_trimmed = opt_text.trim_start();
                                    p = buf_write(p, r##"<label class="option"><input type="radio" name="q"##);
                                    p = buf_write(p, num_to_str(qi));
                                    p = buf_write(p, r##"" value=""##);
                                    p = buf_write(p, num_to_str(oi_idx));
                                    p = buf_write(p, r##"">"##);
                                    p = buf_write(p, opt_trimmed);
                                    p = buf_write(p, r##"</label>"##);
                                    oi_idx += 1;
                                }
                                os = oei + 1;
                            }
                            oei += 1;
                        }
                        p = buf_write(p, r##"</div>"##);
                        qi += 1;
                    }
                }
            }
            es = ei + 1;
        }
        ei += 1;
    }

    p = buf_write(p, r##"<button class="btn" type="button" onclick="submitSurvey()">Submit Response</button>
</form></div>
<script>
function submitSurvey(){
  const answers={};
  let total="##);
    p = buf_write(p, num_to_str(qi));
    p = buf_write(p, r##";
  for(let i=0;i<total;i++){
    const r=document.querySelector('input[name="q'+i+'"]:checked');
    if(r)answers['q'+i]=r.value;
  }
  if(Object.keys(answers).length<total){alert('Please answer all questions');return;}
  let ansStr='';
  for(let i=0;i<total;i++){ansStr+=(i>0?';':'')+answers['q'+i];}
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'submit',survey:'"##);
    p = buf_write(p, num_to_str(surv_id));
    p = buf_write(p, r##"',answers:ansStr})})
  .then(()=>{alert('Thank you!');location.href='?results="##);
    p = buf_write(p, num_to_str(surv_id));
    p = buf_write(p, r##"';});
}
</script></body></html>"##);
    respond(200, buf_as_str(p), "text/html");
}

fn render_results(surv_id: u32) {
    let title = kv_read(make_surv_key(surv_id, "title")).unwrap_or("Survey");
    let questions = kv_read(make_surv_key(surv_id, "questions")).unwrap_or("");
    let responses = parse_u32(kv_read(make_surv_key(surv_id, "responses")).unwrap_or("0"));

    let mut p = 0;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Results: "##);
    p = buf_write(p, title);
    p = buf_write(p, r##"</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:'Segoe UI',sans-serif;background:#fafbfc;color:#333;padding:20px}
.container{max-width:700px;margin:0 auto}
h1{color:#6f42c1;margin-bottom:5px}
.meta{color:#6a737d;margin-bottom:25px}
.result-card{background:#fff;border:2px solid #e1e4e8;border-radius:12px;padding:20px;margin-bottom:15px}
.q-text{font-size:1.1em;font-weight:600;margin-bottom:15px}
.opt-row{display:flex;align-items:center;gap:10px;margin-bottom:8px}
.opt-label{min-width:120px;font-size:0.95em}
.bar-bg{flex:1;height:24px;background:#e1e4e8;border-radius:4px;overflow:hidden}
.bar-fill{height:100%;background:#6f42c1;border-radius:4px;transition:width 0.5s;min-width:1px}
.opt-count{min-width:60px;text-align:right;font-size:0.9em;color:#6a737d}
.back{color:#6f42c1;text-decoration:none;display:block;margin-bottom:15px}
</style></head><body>
<div class="container">
<a class="back" href="?">Back to surveys</a>
<h1>"##);
    p = buf_write(p, title);
    p = buf_write(p, r##"</h1><p class="meta">"##);
    p = buf_write(p, num_to_str(responses));
    p = buf_write(p, r##" responses</p>"##);

    let qb = questions.as_bytes();
    let mut es = 0;
    let mut ei = 0;
    let mut qi: u32 = 0;
    while ei <= qb.len() {
        if ei == qb.len() || qb[ei] == b';' {
            if ei > es {
                if let Ok(q_entry) = core::str::from_utf8(&qb[es..ei]) {
                    let qeb = q_entry.as_bytes();
                    if let Some(pp) = qeb.iter().position(|&b| b == b'|') {
                        let q_text = core::str::from_utf8(&qeb[..pp]).unwrap_or("?");
                        let opts_str = core::str::from_utf8(&qeb[pp+1..]).unwrap_or("");
                        p = buf_write(p, r##"<div class="result-card"><div class="q-text">"##);
                        p = buf_write(p, q_text);
                        p = buf_write(p, r##"</div>"##);

                        let ob = opts_str.as_bytes();
                        let mut os = 0;
                        let mut oi: u32 = 0;
                        let mut oei = 0;
                        while oei <= ob.len() {
                            if oei == ob.len() || ob[oei] == b',' {
                                if oei > os {
                                    let opt_text = core::str::from_utf8(&ob[os..oei]).unwrap_or("?");
                                    let opt_trimmed = opt_text.trim_start();
                                    let rk = make_result_key(surv_id, qi, oi);
                                    let count = parse_u32(kv_read(rk).unwrap_or("0"));
                                    let pct = if responses > 0 { (count * 100) / responses } else { 0 };
                                    p = buf_write(p, r##"<div class="opt-row"><span class="opt-label">"##);
                                    p = buf_write(p, opt_trimmed);
                                    p = buf_write(p, r##"</span><div class="bar-bg"><div class="bar-fill" style="width:"##);
                                    p = buf_write(p, num_to_str(pct));
                                    p = buf_write(p, r##"%"></div></div><span class="opt-count">"##);
                                    p = buf_write(p, num_to_str(count));
                                    p = buf_write(p, " (");
                                    p = buf_write(p, num_to_str(pct));
                                    p = buf_write(p, r##"%)</span></div>"##);
                                    oi += 1;
                                }
                                os = oei + 1;
                            }
                            oei += 1;
                        }
                        p = buf_write(p, r##"</div>"##);
                        qi += 1;
                    }
                }
            }
            es = ei + 1;
        }
        ei += 1;
    }

    p = buf_write(p, r##"</div></body></html>"##);
    respond(200, buf_as_str(p), "text/html");
}

#[no_mangle]
pub extern "C" fn x402_handle(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let path = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(path_ptr, path_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    host_log(1, "Survey builder request");

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("");

        if action == "create" {
            let title = find_json_str(body, "title").unwrap_or("Survey");
            let questions = find_json_str(body, "questions").unwrap_or("");
            let count = parse_u32(kv_read("surv_count").unwrap_or("0"));
            kv_write(make_surv_key(count, "title"), title);
            kv_write(make_surv_key(count, "questions"), questions);
            kv_write(make_surv_key(count, "responses"), "0");
            kv_write("surv_count", num_to_str(count + 1));
            respond(200, r##"{"ok":true}"##, "application/json");
        } else if action == "submit" {
            let surv_id = parse_u32(find_json_str(body, "survey").unwrap_or("0"));
            let answers = find_json_str(body, "answers").unwrap_or("");
            // answers = "opt_idx;opt_idx;..." one per question
            let ab = answers.as_bytes();
            let mut as_ = 0;
            let mut ae = 0;
            let mut qi: u32 = 0;
            while ae <= ab.len() {
                if ae == ab.len() || ab[ae] == b';' {
                    if ae > as_ {
                        if let Ok(opt_str) = core::str::from_utf8(&ab[as_..ae]) {
                            let opt_idx = parse_u32(opt_str);
                            let rk = make_result_key(surv_id, qi, opt_idx);
                            let current = parse_u32(kv_read(rk).unwrap_or("0"));
                            kv_write(rk, num_to_str(current + 1));
                        }
                    }
                    qi += 1;
                    as_ = ae + 1;
                }
                ae += 1;
            }
            let resp_count = parse_u32(kv_read(make_surv_key(surv_id, "responses")).unwrap_or("0")) + 1;
            kv_write(make_surv_key(surv_id, "responses"), num_to_str(resp_count));
            respond(200, r##"{"ok":true}"##, "application/json");
        } else {
            respond(400, r##"{"error":"unknown"}"##, "application/json");
        }
        return;
    }

    let query = if let Some(qi) = path.as_bytes().iter().position(|&b| b == b'?') {
        &path[qi + 1..]
    } else {
        ""
    };

    if let Some(id_str) = find_query_param(query, "take") {
        render_take_survey(parse_u32(id_str));
    } else if let Some(id_str) = find_query_param(query, "results") {
        render_results(parse_u32(id_str));
    } else {
        render_survey_list();
    }
}