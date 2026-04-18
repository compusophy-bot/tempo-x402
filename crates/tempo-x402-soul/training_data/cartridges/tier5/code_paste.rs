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

fn make_paste_key(id: u32, suffix: &str) -> &'static str {
    static mut KBUF: [u8; 64] = [0u8; 64];
    let prefix = b"paste_";
    let id_s = num_to_str(id);
    let ib = id_s.as_bytes();
    let sb = suffix.as_bytes();
    let total = prefix.len() + ib.len() + 1 + sb.len();
    unsafe {
        KBUF[..prefix.len()].copy_from_slice(prefix);
        KBUF[prefix.len()..prefix.len()+ib.len()].copy_from_slice(ib);
        KBUF[prefix.len()+ib.len()] = b'_';
        KBUF[prefix.len()+ib.len()+1..total].copy_from_slice(sb);
        core::str::from_utf8_unchecked(&KBUF[..total])
    }
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

fn render_home() {
    let count = parse_u32(kv_read("paste_count").unwrap_or("0"));
    let mut p = 0;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Code Paste</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:'JetBrains Mono','Fira Code',monospace;background:#0d1117;color:#c9d1d9;min-height:100vh;padding:20px}
.container{max-width:900px;margin:0 auto}
h1{color:#f0883e;font-size:1.8em;margin-bottom:5px}
.tagline{color:#8b949e;margin-bottom:25px}
.create-box{background:#161b22;border:1px solid #30363d;border-radius:10px;padding:20px;margin-bottom:25px}
.create-box label{color:#58a6ff;font-weight:600;display:block;margin-bottom:8px}
.create-box input{width:100%;background:#0d1117;color:#c9d1d9;border:1px solid #30363d;padding:10px;border-radius:5px;font-size:1em;margin-bottom:15px;font-family:inherit}
.create-box textarea{width:100%;height:250px;background:#0d1117;color:#c9d1d9;border:1px solid #30363d;padding:15px;border-radius:5px;font-size:0.95em;font-family:'JetBrains Mono',monospace;line-height:1.6;resize:vertical;tab-size:4}
.create-box select{background:#0d1117;color:#c9d1d9;border:1px solid #30363d;padding:8px 12px;border-radius:5px;font-size:1em;margin-bottom:15px}
.row{display:flex;gap:15px;align-items:end}
.row>div{flex:1}
.btn{background:#238636;color:#fff;border:none;padding:10px 25px;border-radius:6px;cursor:pointer;font-size:1em;font-weight:600}
.btn:hover{background:#2ea043}
.recent{background:#161b22;border:1px solid #30363d;border-radius:10px;padding:20px}
.recent h2{color:#58a6ff;margin-bottom:15px}
.paste-item{display:flex;justify-content:space-between;align-items:center;padding:10px 15px;border-bottom:1px solid #21262d}
.paste-item:hover{background:#1c2128}
.paste-item a{color:#f0883e;text-decoration:none;font-weight:600}
.paste-item a:hover{text-decoration:underline}
.paste-meta{color:#8b949e;font-size:0.85em}
.lang-badge{background:#30363d;color:#8b949e;padding:2px 8px;border-radius:4px;font-size:0.8em;margin-left:10px}
.views{color:#3fb950}
</style></head><body>
<div class="container">
<h1>Code Paste</h1>
<p class="tagline">Share code snippets instantly</p>
<div class="create-box">
<div class="row">
<div><label>Title</label><input type="text" id="title" placeholder="Untitled paste"></div>
<div><label>Language</label><select id="lang">
<option value="text">Plain Text</option><option value="rust">Rust</option>
<option value="python">Python</option><option value="javascript">JavaScript</option>
<option value="html">HTML</option><option value="css">CSS</option>
<option value="json">JSON</option><option value="sql">SQL</option>
<option value="bash">Bash</option><option value="go">Go</option>
</select></div>
</div>
<label>Content</label>
<textarea id="content" placeholder="Paste your code here..."></textarea>
<button class="btn" onclick="createPaste()">Create Paste</button>
</div>
<div class="recent"><h2>Recent Pastes ("##);
    p = buf_write(p, num_to_str(count));
    p = buf_write(p, r##" total)</h2>"##);

    // List recent pastes (newest first)
    if count > 0 {
        let mut i = count;
        let limit = if count > 20 { count - 20 } else { 0 };
        while i > limit {
            i -= 1;
            let title_key = make_paste_key(i, "title");
            let lang_key = make_paste_key(i, "lang");
            let views_key = make_paste_key(i, "views");
            let title = kv_read(title_key).unwrap_or("Untitled");
            let lang = kv_read(lang_key).unwrap_or("text");
            let views = kv_read(views_key).unwrap_or("0");
            p = buf_write(p, r##"<div class="paste-item"><div><a href="?view="##);
            p = buf_write(p, num_to_str(i));
            p = buf_write(p, r##"">"##);
            p = buf_write(p, title);
            p = buf_write(p, r##"</a><span class="lang-badge">"##);
            p = buf_write(p, lang);
            p = buf_write(p, r##"</span></div><span class="paste-meta"><span class="views">"##);
            p = buf_write(p, views);
            p = buf_write(p, r##" views</span></span></div>"##);
        }
    } else {
        p = buf_write(p, r##"<p style="color:#8b949e;text-align:center;padding:20px">No pastes yet. Create one above!</p>"##);
    }

    p = buf_write(p, r##"</div></div>
<script>
function createPaste(){
  const t=document.getElementById('title').value||'Untitled';
  const l=document.getElementById('lang').value;
  const c=document.getElementById('content').value;
  if(!c){alert('Content is required');return;}
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'create',title:t,lang:l,content:c})})
  .then(r=>r.json()).then(d=>{if(d.id!==undefined)location.href='?view='+d.id;else location.reload();});
}
</script></body></html>"##);
    respond(200, buf_as_str(p), "text/html");
}

fn render_view(id: u32) {
    let title = kv_read(make_paste_key(id, "title")).unwrap_or("Untitled");
    let lang = kv_read(make_paste_key(id, "lang")).unwrap_or("text");
    let content = kv_read(make_paste_key(id, "content")).unwrap_or("");
    let views_str = kv_read(make_paste_key(id, "views")).unwrap_or("0");
    let views = parse_u32(views_str) + 1;
    kv_write(make_paste_key(id, "views"), num_to_str(views));

    let mut p = 0;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>"##);
    p = buf_write(p, title);
    p = buf_write(p, r##" - Code Paste</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:'JetBrains Mono',monospace;background:#0d1117;color:#c9d1d9;min-height:100vh;padding:20px}
.container{max-width:900px;margin:0 auto}
.header{display:flex;justify-content:space-between;align-items:center;margin-bottom:20px}
h1{color:#f0883e;font-size:1.5em}
.back{color:#58a6ff;text-decoration:none}
.back:hover{text-decoration:underline}
.meta{display:flex;gap:15px;margin-bottom:15px;color:#8b949e;font-size:0.9em}
.lang-badge{background:#30363d;color:#c9d1d9;padding:3px 10px;border-radius:4px}
.code-box{background:#161b22;border:1px solid #30363d;border-radius:10px;overflow:hidden}
.code-header{background:#1c2128;padding:10px 15px;display:flex;justify-content:space-between;border-bottom:1px solid #30363d}
.code-header span{color:#8b949e}
.copy-btn{background:#30363d;color:#c9d1d9;border:none;padding:5px 15px;border-radius:4px;cursor:pointer;font-size:0.85em}
.copy-btn:hover{background:#484f58}
pre{padding:20px;overflow-x:auto;line-height:1.6;font-size:0.95em;white-space:pre-wrap;word-break:break-all}
.raw-link{color:#58a6ff;text-decoration:none;margin-top:15px;display:inline-block}
</style></head><body>
<div class="container">
<div class="header"><h1>"##);
    p = buf_write(p, title);
    p = buf_write(p, r##"</h1><a class="back" href="?">Back to list</a></div>
<div class="meta"><span class="lang-badge">"##);
    p = buf_write(p, lang);
    p = buf_write(p, r##"</span><span>"##);
    p = buf_write(p, num_to_str(views));
    p = buf_write(p, r##" views</span><span>Paste ##"##);
    p = buf_write(p, num_to_str(id));
    p = buf_write(p, r##"</span></div>
<div class="code-box">
<div class="code-header"><span>"##);
    p = buf_write(p, lang);
    p = buf_write(p, r##"</span><button class="copy-btn" onclick="copyCode()">Copy</button></div>
<pre id="code-content">"##);
    p = buf_write(p, content);
    p = buf_write(p, r##"</pre></div>
<a class="raw-link" href="?raw="##);
    p = buf_write(p, num_to_str(id));
    p = buf_write(p, r##"">View Raw</a>
</div>
<script>
function copyCode(){
  const t=document.getElementById('code-content').textContent;
  navigator.clipboard.writeText(t).then(()=>{
    document.querySelector('.copy-btn').textContent='Copied!';
    setTimeout(()=>document.querySelector('.copy-btn').textContent='Copy',2000);
  });
}
</script></body></html>"##);
    respond(200, buf_as_str(p), "text/html");
}

#[no_mangle]
pub extern "C" fn x402_handle(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let path = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(path_ptr, path_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    host_log(1, "Code paste request");

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("");
        if action == "create" {
            let title = find_json_str(body, "title").unwrap_or("Untitled");
            let lang = find_json_str(body, "lang").unwrap_or("text");
            let content = find_json_str(body, "content").unwrap_or("");
            let count = parse_u32(kv_read("paste_count").unwrap_or("0"));
            kv_write(make_paste_key(count, "title"), title);
            kv_write(make_paste_key(count, "lang"), lang);
            kv_write(make_paste_key(count, "content"), content);
            kv_write(make_paste_key(count, "views"), "0");
            kv_write("paste_count", num_to_str(count + 1));
            // Return the ID
            let mut rp = 0;
            rp = buf_write(rp, r##"{"id":"##);
            rp = buf_write(rp, num_to_str(count));
            rp = buf_write(rp, r##"}"##);
            respond(200, buf_as_str(rp), "application/json");
        } else {
            respond(400, r##"{"error":"unknown action"}"##, "application/json");
        }
        return;
    }

    let query = if let Some(qi) = path.as_bytes().iter().position(|&b| b == b'?') {
        &path[qi + 1..]
    } else {
        ""
    };

    if let Some(id_str) = find_query_param(query, "raw") {
        let id = parse_u32(id_str);
        let content = kv_read(make_paste_key(id, "content")).unwrap_or("");
        respond(200, content, "text/plain");
    } else if let Some(id_str) = find_query_param(query, "view") {
        render_view(parse_u32(id_str));
    } else {
        render_home();
    }
}