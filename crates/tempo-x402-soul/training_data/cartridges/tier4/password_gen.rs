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

#[no_mangle]
pub extern "C" fn handle_request(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    // Password generation is entirely client-side via JS for security
    // The server just serves the HTML app and persists generation count in KV

    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };

    if method == "POST" {
        // Increment generation counter
        let count_str = kv_read("pw_gen_count").unwrap_or("0");
        let count = parse_usize(count_str) + 1;
        static mut CNT_BUF: [u8; 20] = [0u8; 20];
        let mut ci = 0;
        let mut cn = count;
        if cn == 0 { unsafe { CNT_BUF[0] = b'0'; } ci = 1; } else {
            while cn > 0 { unsafe { CNT_BUF[ci] = b'0' + (cn % 10) as u8; } cn /= 10; ci += 1; }
        }
        // Reverse
        let mut lo = 0;
        let mut hi = ci - 1;
        while lo < hi {
            unsafe { let tmp = CNT_BUF[lo]; CNT_BUF[lo] = CNT_BUF[hi]; CNT_BUF[hi] = tmp; }
            lo += 1;
            hi -= 1;
        }
        kv_write("pw_gen_count", unsafe { core::str::from_utf8_unchecked(&CNT_BUF[..ci]) });
    }

    let page = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Password Generator</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#0c0c0c;color:#e0e0e0;font-family:'Segoe UI',sans-serif;min-height:100vh;padding:20px;display:flex;flex-direction:column;align-items:center}
h1{color:#00ff88;margin:20px 0;font-size:2.2em;text-shadow:0 0 15px rgba(0,255,136,0.3)}
.container{width:100%;max-width:550px}
.gen-card{background:#1a1a1a;border-radius:20px;padding:30px;margin-bottom:20px;border:1px solid #333;position:relative;overflow:hidden}
.gen-card::before{content:'';position:absolute;top:0;left:0;right:0;height:2px;background:linear-gradient(90deg,#00ff88,#00ccff,#ff00ff)}
.password-display{background:#0c0c0c;border:2px solid #333;border-radius:12px;padding:20px;text-align:center;margin-bottom:20px;position:relative;word-break:break-all}
.password-text{font-family:'Courier New',monospace;font-size:1.6em;font-weight:bold;color:#00ff88;letter-spacing:1px;min-height:1.6em}
.strength-bar{height:6px;background:#333;border-radius:3px;margin:12px 0;overflow:hidden}
.strength-fill{height:100%;border-radius:3px;transition:width 0.3s,background 0.3s}
.strength-label{text-align:center;font-size:0.85em;margin-bottom:15px;font-weight:bold}
.options{margin-bottom:20px}
.option-row{display:flex;align-items:center;justify-content:space-between;padding:12px 0;border-bottom:1px solid #222}
.option-row:last-child{border-bottom:none}
.option-label{font-size:1em;color:#ccc}
.length-control{display:flex;align-items:center;gap:12px}
.length-control input[type=range]{width:150px;height:6px;-webkit-appearance:none;background:#333;border-radius:3px}
.length-control input[type=range]::-webkit-slider-thumb{-webkit-appearance:none;width:18px;height:18px;border-radius:50%;background:#00ff88;cursor:pointer}
.length-val{font-family:monospace;font-size:1.1em;color:#00ff88;min-width:30px;text-align:center}
.toggle{width:48px;height:26px;background:#333;border-radius:13px;position:relative;cursor:pointer;transition:background 0.3s}
.toggle.on{background:#00ff88}
.toggle::after{content:'';position:absolute;width:22px;height:22px;border-radius:50%;background:#fff;top:2px;left:2px;transition:left 0.3s}
.toggle.on::after{left:24px}
.btn-row{display:flex;gap:10px}
.btn{flex:1;padding:14px;border:none;border-radius:12px;font-size:1em;cursor:pointer;font-weight:bold;transition:transform 0.1s}
.btn:hover{transform:scale(1.02)}
.btn-gen{background:#00ff88;color:#0c0c0c}
.btn-copy{background:#333;color:#e0e0e0}
.btn-copy.copied{background:#00ff88;color:#0c0c0c}
.history{background:#1a1a1a;border-radius:20px;padding:20px;border:1px solid #333}
.history h2{color:#00ccff;margin-bottom:15px}
.hist-item{display:flex;justify-content:space-between;align-items:center;padding:10px;background:#0c0c0c;border-radius:8px;margin-bottom:6px;font-family:monospace;font-size:0.9em}
.hist-item .pw{color:#00ff88;word-break:break-all;flex:1;margin-right:10px}
.hist-item .copy-sm{background:#333;color:#ccc;border:none;border-radius:6px;padding:4px 10px;cursor:pointer;font-size:0.85em}
.hist-item .copy-sm:hover{background:#00ff88;color:#0c0c0c}
.tips{margin-top:15px;padding:15px;background:#111;border-radius:10px;border-left:3px solid #00ccff}
.tips h3{color:#00ccff;margin-bottom:8px;font-size:0.95em}
.tips ul{list-style:none;padding:0}
.tips li{color:#888;font-size:0.85em;padding:3px 0}
.tips li::before{content:'>';color:#00ccff;margin-right:8px}
.empty-hist{color:#555;text-align:center;padding:15px}
</style></head><body>
<h1>&#128272; Password Generator</h1>
<div class="container">
<div class="gen-card">
<div class="password-display"><div class="password-text" id="pwDisplay">Click Generate</div></div>
<div class="strength-bar"><div class="strength-fill" id="strengthFill" style="width:0%"></div></div>
<div class="strength-label" id="strengthLabel">-</div>
<div class="options">
<div class="option-row"><span class="option-label">Length</span><div class="length-control"><input type="range" min="8" max="64" value="16" id="lenSlider" oninput="updateLen()"><span class="length-val" id="lenVal">16</span></div></div>
<div class="option-row"><span class="option-label">Uppercase (A-Z)</span><div class="toggle on" id="togUpper" onclick="toggleOpt(this)"></div></div>
<div class="option-row"><span class="option-label">Lowercase (a-z)</span><div class="toggle on" id="togLower" onclick="toggleOpt(this)"></div></div>
<div class="option-row"><span class="option-label">Numbers (0-9)</span><div class="toggle on" id="togNumbers" onclick="toggleOpt(this)"></div></div>
<div class="option-row"><span class="option-label">Symbols (!@#$%...)</span><div class="toggle on" id="togSymbols" onclick="toggleOpt(this)"></div></div>
</div>
<div class="btn-row">
<button class="btn btn-gen" onclick="generate()">&#9889; Generate</button>
<button class="btn btn-copy" id="copyBtn" onclick="copyPw()">&#128203; Copy</button>
</div>
</div>
<div class="history"><h2>Recent Passwords</h2><div id="histList"><div class="empty-hist">No passwords generated yet</div></div></div>
<div class="tips"><h3>Security Tips</h3><ul>
<li>Use at least 16 characters for important accounts</li>
<li>Never reuse passwords across sites</li>
<li>Enable two-factor authentication where possible</li>
<li>Use a password manager to store your passwords</li>
</ul></div>
</div>
<script>
var history=[];
function updateLen(){document.getElementById('lenVal').textContent=document.getElementById('lenSlider').value}
function toggleOpt(el){el.classList.toggle('on')}
function isOn(id){return document.getElementById(id).classList.contains('on')}
function generate(){
  var len=parseInt(document.getElementById('lenSlider').value);
  var chars='';
  if(isOn('togUpper'))chars+='ABCDEFGHIJKLMNOPQRSTUVWXYZ';
  if(isOn('togLower'))chars+='abcdefghijklmnopqrstuvwxyz';
  if(isOn('togNumbers'))chars+='0123456789';
  if(isOn('togSymbols'))chars+='!@#$%^&*()_+-=[]{}|;:,.<>?';
  if(chars.length===0){alert('Enable at least one character type');return}
  var arr=new Uint32Array(len);
  crypto.getRandomValues(arr);
  var pw='';
  for(var i=0;i<len;i++)pw+=chars[arr[i]%chars.length];
  document.getElementById('pwDisplay').textContent=pw;
  updateStrength(pw);
  history.unshift(pw);
  if(history.length>10)history.pop();
  renderHistory();
  fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:'{}'});
}
function updateStrength(pw){
  var score=0;
  if(pw.length>=8)score+=1;
  if(pw.length>=12)score+=1;
  if(pw.length>=16)score+=1;
  if(pw.length>=24)score+=1;
  if(/[a-z]/.test(pw)&&/[A-Z]/.test(pw))score+=1;
  if(/[0-9]/.test(pw))score+=1;
  if(/[^a-zA-Z0-9]/.test(pw))score+=1;
  var pct=Math.min(score*15,100);
  var fill=document.getElementById('strengthFill');
  var label=document.getElementById('strengthLabel');
  fill.style.width=pct+'%';
  if(pct<30){fill.style.background='#ff4444';label.textContent='Weak';label.style.color='#ff4444'}
  else if(pct<60){fill.style.background='#ffaa00';label.textContent='Fair';label.style.color='#ffaa00'}
  else if(pct<85){fill.style.background='#00ccff';label.textContent='Strong';label.style.color='#00ccff'}
  else{fill.style.background='#00ff88';label.textContent='Very Strong';label.style.color='#00ff88'}
}
function copyPw(){
  var pw=document.getElementById('pwDisplay').textContent;
  if(pw==='Click Generate')return;
  navigator.clipboard.writeText(pw).then(function(){
    var btn=document.getElementById('copyBtn');
    btn.textContent='Copied!';btn.classList.add('copied');
    setTimeout(function(){btn.textContent='Copy';btn.classList.remove('copied')},1500);
  });
}
function renderHistory(){
  var el=document.getElementById('histList');
  if(history.length===0){el.innerHTML='<div class="empty-hist">No passwords generated yet</div>';return}
  var html='';
  for(var i=0;i<history.length;i++){
    html+='<div class="hist-item"><span class="pw">'+history[i]+'</span><button class="copy-sm" onclick="copyText(\''+history[i]+'\')">Copy</button></div>';
  }
  el.innerHTML=html;
}
function copyText(t){navigator.clipboard.writeText(t)}
</script></body></html>"##;

    respond(200, page, "text/html");
}

fn parse_usize(s: &str) -> usize {
    let mut n = 0usize;
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() { if b[i] >= b'0' && b[i] <= b'9' { n = n * 10 + (b[i] - b'0') as usize; } i += 1; }
    n
}
