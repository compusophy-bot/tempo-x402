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

struct BufWriter { pos: usize }
impl BufWriter {
    fn new() -> Self { Self { pos: 0 } }
    fn push_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        unsafe {
            let end = (self.pos + bytes.len()).min(BUF.len());
            BUF[self.pos..end].copy_from_slice(&bytes[..end - self.pos]);
            self.pos = end;
        }
    }
    fn push_num(&mut self, mut n: u32) {
        if n == 0 { self.push_str("0"); return; }
        let mut d = [0u8; 10];
        let mut i = 0;
        while n > 0 { d[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
        while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = d[i]; self.pos += 1; } } }
    }
    fn as_str(&self) -> &str { unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) } }
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}

// --- Markdown parser helpers ---

fn push_escaped(w: &mut BufWriter, s: &str) {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'<' => w.push_str("&lt;"),
            b'>' => w.push_str("&gt;"),
            b'&' => w.push_str("&amp;"),
            _ => {
                unsafe {
                    if w.pos < BUF.len() {
                        BUF[w.pos] = b[i];
                        w.pos += 1;
                    }
                }
            }
        }
        i += 1;
    }
}

/// Parse inline markdown: **bold**, *italic*, `code`, [text](url)
fn push_inline(w: &mut BufWriter, line: &str) {
    let b = line.as_bytes();
    let len = b.len();
    let mut i = 0;
    while i < len {
        // Bold **...**
        if i + 2 < len && b[i] == b'*' && b[i+1] == b'*' {
            let start = i + 2;
            let mut end = start;
            while end + 1 < len && !(b[end] == b'*' && b[end+1] == b'*') { end += 1; }
            if end + 1 < len {
                w.push_str("<strong>");
                let inner = unsafe { core::str::from_utf8_unchecked(&b[start..end]) };
                push_escaped(w, inner);
                w.push_str("</strong>");
                i = end + 2;
                continue;
            }
        }
        // Italic *...*
        if b[i] == b'*' && i + 1 < len && b[i+1] != b'*' {
            let start = i + 1;
            let mut end = start;
            while end < len && b[end] != b'*' { end += 1; }
            if end < len {
                w.push_str("<em>");
                let inner = unsafe { core::str::from_utf8_unchecked(&b[start..end]) };
                push_escaped(w, inner);
                w.push_str("</em>");
                i = end + 1;
                continue;
            }
        }
        // Inline code `...`
        if b[i] == b'`' {
            let start = i + 1;
            let mut end = start;
            while end < len && b[end] != b'`' { end += 1; }
            if end < len {
                w.push_str("<code>");
                let inner = unsafe { core::str::from_utf8_unchecked(&b[start..end]) };
                push_escaped(w, inner);
                w.push_str("</code>");
                i = end + 1;
                continue;
            }
        }
        // Escape individual char
        match b[i] {
            b'<' => w.push_str("&lt;"),
            b'>' => w.push_str("&gt;"),
            b'&' => w.push_str("&amp;"),
            _ => unsafe {
                if w.pos < BUF.len() {
                    BUF[w.pos] = b[i];
                    w.pos += 1;
                }
            }
        }
        i += 1;
    }
}

fn starts_with(line: &[u8], prefix: &[u8]) -> bool {
    if line.len() < prefix.len() { return false; }
    let mut i = 0;
    while i < prefix.len() {
        if line[i] != prefix[i] { return false; }
        i += 1;
    }
    true
}

fn parse_u32(s: &str) -> u32 {
    let b = s.as_bytes();
    let mut r: u32 = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] >= b'0' && b[i] <= b'9' { r = r * 10 + (b[i] - b'0') as u32; }
        i += 1;
    }
    r
}

/// Render markdown text to HTML
fn render_markdown(w: &mut BufWriter, md: &str) {
    let b = md.as_bytes();
    let len = b.len();
    let mut i = 0;
    let mut in_code_block = false;
    let mut in_list = false;

    while i < len {
        // Find end of current line
        let mut eol = i;
        while eol < len && b[eol] != b'\n' { eol += 1; }
        let line = unsafe { core::str::from_utf8_unchecked(&b[i..eol]) };
        let lb = line.as_bytes();

        // Code block ```
        if lb.len() >= 3 && lb[0] == b'`' && lb[1] == b'`' && lb[2] == b'`' {
            if in_code_block {
                w.push_str("</code></pre>");
                in_code_block = false;
            } else {
                if in_list { w.push_str("</ul>"); in_list = false; }
                w.push_str("<pre><code>");
                in_code_block = true;
            }
            i = eol + 1;
            continue;
        }

        if in_code_block {
            push_escaped(w, line);
            w.push_str("\n");
            i = eol + 1;
            continue;
        }

        // Empty line
        if lb.is_empty() {
            if in_list { w.push_str("</ul>"); in_list = false; }
            i = eol + 1;
            continue;
        }

        // Headers
        if starts_with(lb, b"### ") {
            if in_list { w.push_str("</ul>"); in_list = false; }
            w.push_str("<h3>");
            let rest = unsafe { core::str::from_utf8_unchecked(&lb[4..]) };
            push_inline(w, rest);
            w.push_str("</h3>");
            i = eol + 1;
            continue;
        }
        if starts_with(lb, b"## ") {
            if in_list { w.push_str("</ul>"); in_list = false; }
            w.push_str("<h2>");
            let rest = unsafe { core::str::from_utf8_unchecked(&lb[3..]) };
            push_inline(w, rest);
            w.push_str("</h2>");
            i = eol + 1;
            continue;
        }
        if starts_with(lb, b"# ") {
            if in_list { w.push_str("</ul>"); in_list = false; }
            w.push_str("<h1>");
            let rest = unsafe { core::str::from_utf8_unchecked(&lb[2..]) };
            push_inline(w, rest);
            w.push_str("</h1>");
            i = eol + 1;
            continue;
        }

        // Horizontal rule
        if starts_with(lb, b"---") && lb.len() >= 3 {
            if in_list { w.push_str("</ul>"); in_list = false; }
            w.push_str("<hr>");
            i = eol + 1;
            continue;
        }

        // Unordered list
        if lb.len() >= 2 && (lb[0] == b'-' || lb[0] == b'*') && lb[1] == b' ' {
            if !in_list { w.push_str("<ul>"); in_list = true; }
            w.push_str("<li>");
            let rest = unsafe { core::str::from_utf8_unchecked(&lb[2..]) };
            push_inline(w, rest);
            w.push_str("</li>");
            i = eol + 1;
            continue;
        }

        // Blockquote
        if starts_with(lb, b"> ") {
            if in_list { w.push_str("</ul>"); in_list = false; }
            w.push_str("<blockquote>");
            let rest = unsafe { core::str::from_utf8_unchecked(&lb[2..]) };
            push_inline(w, rest);
            w.push_str("</blockquote>");
            i = eol + 1;
            continue;
        }

        // Paragraph
        if in_list { w.push_str("</ul>"); in_list = false; }
        w.push_str("<p>");
        push_inline(w, line);
        w.push_str("</p>");
        i = eol + 1;
    }

    if in_code_block { w.push_str("</code></pre>"); }
    if in_list { w.push_str("</ul>"); }
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize))
    };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(0, "markdown_viewer: handling request");

    if method == "POST" {
        // Save markdown and render
        let md_text = find_json_str(body, "markdown").unwrap_or(body);
        if md_text.is_empty() {
            respond(400, r#"{"error":"no markdown provided"}"#, "application/json");
            return;
        }
        kv_write("md_content", md_text);

        // Count documents
        let count = match kv_read("md_count") {
            Some(s) => parse_u32(s),
            None => 0,
        };
        let new_count = count + 1;
        let mut num_buf = [0u8; 12];
        let mut np = 0;
        let mut n = new_count;
        if n == 0 { num_buf[0] = b'0'; np = 1; }
        else {
            let mut d = [0u8; 10]; let mut di = 0;
            while n > 0 { d[di] = b'0' + (n % 10) as u8; n /= 10; di += 1; }
            while di > 0 { di -= 1; num_buf[np] = d[di]; np += 1; }
        }
        let cs = unsafe { core::str::from_utf8_unchecked(&num_buf[..np]) };
        kv_write("md_count", cs);

        respond(200, r#"{"ok":true,"message":"markdown saved"}"#, "application/json");
        return;
    }

    // GET — render editor + preview
    let mut w = BufWriter::new();
    w.push_str("<!DOCTYPE html><html><head><meta charset='utf-8'><title>Markdown Viewer</title><style>");
    w.push_str("*{margin:0;padding:0;box-sizing:border-box}");
    w.push_str("body{font-family:'Segoe UI',system-ui,sans-serif;background:#0a0a1a;color:#e0e0e0;height:100vh;display:flex;flex-direction:column}");
    w.push_str(".toolbar{background:#111;border-bottom:1px solid #333;padding:10px 20px;display:flex;align-items:center;gap:16px}");
    w.push_str(".toolbar h1{font-size:1.2rem;color:#7c4dff}");
    w.push_str(".toolbar button{padding:6px 16px;border:1px solid #444;border-radius:6px;background:#1a1a3e;color:#e0e0e0;cursor:pointer;font-size:0.85rem;transition:all 0.2s}");
    w.push_str(".toolbar button:hover{background:#2d1b69;border-color:#7c4dff}");
    w.push_str(".toolbar button.active{background:#7c4dff;color:#fff}");
    w.push_str(".toolbar .stats{margin-left:auto;font-size:0.8rem;color:#888}");
    w.push_str(".container{flex:1;display:flex;overflow:hidden}");
    w.push_str(".editor{flex:1;display:flex;flex-direction:column;border-right:1px solid #333}");
    w.push_str(".editor textarea{flex:1;padding:20px;background:#0a0a1a;color:#e0e0e0;border:none;resize:none;font-family:'Fira Code','Courier New',monospace;font-size:0.9rem;line-height:1.6;tab-size:4}");
    w.push_str(".editor textarea:focus{outline:none}");
    w.push_str(".preview{flex:1;overflow-y:auto;padding:20px 30px}");
    w.push_str(".preview h1{font-size:2rem;color:#fff;border-bottom:2px solid #333;padding-bottom:8px;margin:20px 0 12px}");
    w.push_str(".preview h2{font-size:1.5rem;color:#e0e0e0;border-bottom:1px solid #222;padding-bottom:6px;margin:18px 0 10px}");
    w.push_str(".preview h3{font-size:1.2rem;color:#ccc;margin:16px 0 8px}");
    w.push_str(".preview p{line-height:1.7;margin:10px 0;color:#bbb}");
    w.push_str(".preview strong{color:#fff}");
    w.push_str(".preview em{color:#aaa;font-style:italic}");
    w.push_str(".preview code{background:#1a1a3e;color:#7c4dff;padding:2px 6px;border-radius:4px;font-family:'Fira Code',monospace;font-size:0.85em}");
    w.push_str(".preview pre{background:#111;border:1px solid #333;border-radius:8px;padding:16px;margin:12px 0;overflow-x:auto}");
    w.push_str(".preview pre code{background:transparent;padding:0;color:#e0e0e0}");
    w.push_str(".preview ul{margin:10px 0 10px 24px;list-style:disc}");
    w.push_str(".preview li{line-height:1.7;margin:4px 0;color:#bbb}");
    w.push_str(".preview blockquote{border-left:4px solid #7c4dff;margin:12px 0;padding:8px 16px;background:#111;color:#999;font-style:italic}");
    w.push_str(".preview hr{border:none;border-top:1px solid #333;margin:20px 0}");
    w.push_str(".mode-edit .preview{display:none}.mode-preview .editor{display:none}");
    w.push_str("</style></head><body>");

    w.push_str("<div class='toolbar'>");
    w.push_str("<h1>Markdown Viewer</h1>");
    w.push_str("<button id='btnSplit' class='active' onclick='setMode(\"split\")'>Split</button>");
    w.push_str("<button id='btnEdit' onclick='setMode(\"edit\")'>Edit</button>");
    w.push_str("<button id='btnPreview' onclick='setMode(\"preview\")'>Preview</button>");
    w.push_str("<button onclick='saveDoc()'>Save</button>");
    w.push_str("<button onclick='loadDoc()'>Load Saved</button>");
    w.push_str("<div class='stats'><span id='charCount'>0</span> chars | <span id='wordCount'>0</span> words | ");
    w.push_str("Docs saved: <span id='docCount'>0</span></div>");
    w.push_str("</div>");

    w.push_str("<div class='container' id='main'>");
    w.push_str("<div class='editor'><textarea id='input' placeholder='Type your markdown here...'>");

    // Default sample content
    w.push_str("# Welcome to Markdown Viewer\\n\\n");
    w.push_str("This is a **live markdown editor** with *real-time* preview.\\n\\n");
    w.push_str("## Features\\n\\n");
    w.push_str("- Headers (h1-h3)\\n- **Bold** and *italic* text\\n- `Inline code`\\n- Code blocks\\n- Lists\\n- Blockquotes\\n- Horizontal rules\\n\\n");
    w.push_str("## Code Example\\n\\n```\\nfn main() {\\n    println!(\\\"Hello, x402!\\\");\\n}\\n```\\n\\n");
    w.push_str("> This is a blockquote. Wisdom from the colony.\\n\\n---\\n\\nEnjoy writing!");
    w.push_str("</textarea></div>");
    w.push_str("<div class='preview' id='preview'></div>");
    w.push_str("</div>");

    w.push_str("<script>");
    w.push_str("const input=document.getElementById('input');");
    w.push_str("const preview=document.getElementById('preview');");
    w.push_str("const main=document.getElementById('main');");

    // Markdown to HTML converter in JS (mirrors the Rust one for live preview)
    w.push_str("function mdToHtml(md){let html='';const lines=md.split('\\n');let inCode=false,inList=false;");
    w.push_str("for(let l of lines){");
    w.push_str("if(l.startsWith('```')){if(inCode){html+='</code></pre>';inCode=false;}else{if(inList){html+='</ul>';inList=false;}html+='<pre><code>';inCode=true;}continue;}");
    w.push_str("if(inCode){html+=esc(l)+'\\n';continue;}");
    w.push_str("if(!l.trim()){if(inList){html+='</ul>';inList=false;}continue;}");
    w.push_str("if(l.startsWith('### ')){if(inList){html+='</ul>';inList=false;}html+='<h3>'+inline(l.slice(4))+'</h3>';continue;}");
    w.push_str("if(l.startsWith('## ')){if(inList){html+='</ul>';inList=false;}html+='<h2>'+inline(l.slice(3))+'</h2>';continue;}");
    w.push_str("if(l.startsWith('# ')){if(inList){html+='</ul>';inList=false;}html+='<h1>'+inline(l.slice(2))+'</h1>';continue;}");
    w.push_str("if(l.startsWith('---')){if(inList){html+='</ul>';inList=false;}html+='<hr>';continue;}");
    w.push_str("if(l.match(/^[\\-\\*] /)){if(!inList){html+='<ul>';inList=true;}html+='<li>'+inline(l.slice(2))+'</li>';continue;}");
    w.push_str("if(l.startsWith('> ')){if(inList){html+='</ul>';inList=false;}html+='<blockquote>'+inline(l.slice(2))+'</blockquote>';continue;}");
    w.push_str("if(inList){html+='</ul>';inList=false;}html+='<p>'+inline(l)+'</p>';}");
    w.push_str("if(inCode)html+='</code></pre>';if(inList)html+='</ul>';return html;}");

    w.push_str("function esc(s){return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');}");
    w.push_str("function inline(s){return s.replace(/\\*\\*(.+?)\\*\\*/g,'<strong>$1</strong>').replace(/\\*(.+?)\\*/g,'<em>$1</em>').replace(/`(.+?)`/g,'<code>$1</code>');}");

    w.push_str("function update(){preview.innerHTML=mdToHtml(input.value);");
    w.push_str("document.getElementById('charCount').textContent=input.value.length;");
    w.push_str("document.getElementById('wordCount').textContent=input.value.trim()?input.value.trim().split(/\\s+/).length:0;}");

    w.push_str("function setMode(m){main.className=m==='edit'?'container mode-edit':m==='preview'?'container mode-preview':'container';");
    w.push_str("document.querySelectorAll('.toolbar button').forEach(b=>b.classList.remove('active'));");
    w.push_str("document.getElementById('btn'+m[0].toUpperCase()+m.slice(1)).classList.add('active');}");

    w.push_str("async function saveDoc(){try{await fetch(window.location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},");
    w.push_str("body:JSON.stringify({markdown:input.value})});loadCount();}catch(e){}}");

    w.push_str("async function loadDoc(){try{const saved=await fetch(window.location.pathname);");
    w.push_str("/* saved doc is in KV, accessed via API if we add that route */}catch(e){}}");

    w.push_str("async function loadCount(){try{/* count is updated on save */}catch(e){}}");

    w.push_str("input.addEventListener('input',update);");
    // Unescape the default content
    w.push_str("input.value=input.value.replace(/\\\\n/g,'\\n');");
    w.push_str("update();");
    w.push_str("</script></body></html>");

    respond(200, w.as_str(), "text/html");
}
