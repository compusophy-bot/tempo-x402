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

// --- Helpers ---

fn parse_u32(s: &str) -> u32 {
    let b = s.as_bytes();
    let mut r: u32 = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] >= b'0' && b[i] <= b'9' {
            r = r.wrapping_mul(10).wrapping_add((b[i] - b'0') as u32);
        }
        i += 1;
    }
    r
}

fn make_msg_key(buf: &mut [u8; 32], idx: u32) -> &str {
    let prefix = b"chat_msg_";
    let mut p = 0;
    let mut i = 0;
    while i < prefix.len() { buf[p] = prefix[i]; p += 1; i += 1; }
    // write number
    if idx == 0 {
        buf[p] = b'0'; p += 1;
    } else {
        let mut d = [0u8; 10];
        let mut di = 0;
        let mut n = idx;
        while n > 0 { d[di] = b'0' + (n % 10) as u8; n /= 10; di += 1; }
        while di > 0 { di -= 1; buf[p] = d[di]; p += 1; }
    }
    unsafe { core::str::from_utf8_unchecked(&buf[..p]) }
}

fn html_escape_char(c: u8) -> Option<&'static str> {
    match c {
        b'<' => Some("&lt;"),
        b'>' => Some("&gt;"),
        b'&' => Some("&amp;"),
        b'"' => Some("&quot;"),
        _ => None,
    }
}

fn push_escaped(w: &mut BufWriter, s: &str) {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match html_escape_char(bytes[i]) {
            Some(esc) => w.push_str(esc),
            None => {
                let ch = &s[i..i+1];
                w.push_str(ch);
            }
        }
        i += 1;
    }
}

// --- Main handler ---

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize))
    };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let path = find_json_str(request, "path").unwrap_or("/");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(0, "chat_room: handling request");

    if method == "POST" {
        // Post a new message: body has "username" and "message"
        let username = find_json_str(body, "username").unwrap_or("anon");
        let message = find_json_str(body, "message").unwrap_or("");

        if message.is_empty() {
            respond(400, r#"{"error":"message required"}"#, "application/json");
            return;
        }

        // Get current count
        let count = match kv_read("chat_count") {
            Some(s) => parse_u32(s),
            None => 0,
        };
        let new_count = count + 1;

        // Store message as "username|message"
        let mut key_buf = [0u8; 32];
        let key = make_msg_key(&mut key_buf, new_count);

        // Build value in scratch area
        let mut vp = 0usize;
        let ub = username.as_bytes();
        let mb = message.as_bytes();
        unsafe {
            let mut i = 0;
            while i < ub.len() && vp < 4096 { SCRATCH[vp] = ub[i]; vp += 1; i += 1; }
            if vp < 4096 { SCRATCH[vp] = b'|'; vp += 1; }
            i = 0;
            while i < mb.len() && vp < 4096 { SCRATCH[vp] = mb[i]; vp += 1; i += 1; }
            let val = core::str::from_utf8_unchecked(&SCRATCH[..vp]);
            kv_write(key, val);
        }

        // Update count
        let mut num_buf = [0u8; 12];
        let mut np = 0;
        if new_count == 0 {
            num_buf[0] = b'0'; np = 1;
        } else {
            let mut d = [0u8; 10];
            let mut di = 0;
            let mut n = new_count;
            while n > 0 { d[di] = b'0' + (n % 10) as u8; n /= 10; di += 1; }
            while di > 0 { di -= 1; num_buf[np] = d[di]; np += 1; }
        }
        let count_str = unsafe { core::str::from_utf8_unchecked(&num_buf[..np]) };
        kv_write("chat_count", count_str);

        respond(200, r#"{"ok":true}"#, "application/json");
        return;
    }

    // GET /api/messages — return JSON of last 20 messages
    if path == "/api/messages" {
        let count = match kv_read("chat_count") {
            Some(s) => parse_u32(s),
            None => 0,
        };

        let mut w = BufWriter::new();
        w.push_str(r#"{"messages":["#);

        let start = if count > 20 { count - 20 + 1 } else { 1 };
        let mut first = true;
        let mut idx = start;
        while idx <= count {
            let mut key_buf = [0u8; 32];
            let key = make_msg_key(&mut key_buf, idx);
            if let Some(entry) = kv_read(key) {
                if !first { w.push_str(","); }
                first = false;
                // Split on '|'
                let eb = entry.as_bytes();
                let mut split = 0;
                while split < eb.len() && eb[split] != b'|' { split += 1; }
                let name = unsafe { core::str::from_utf8_unchecked(&eb[..split]) };
                let msg = if split + 1 < eb.len() {
                    unsafe { core::str::from_utf8_unchecked(&eb[split+1..]) }
                } else { "" };
                w.push_str(r#"{"user":""#);
                w.push_str(name);
                w.push_str(r#"","text":""#);
                w.push_str(msg);
                w.push_str(r#"","id":"#);
                w.push_num(idx);
                w.push_str("}");
            }
            idx += 1;
        }
        w.push_str("]}");
        respond(200, w.as_str(), "application/json");
        return;
    }

    // GET / — render chat room HTML
    let mut w = BufWriter::new();
    w.push_str("<!DOCTYPE html><html><head><meta charset='utf-8'><title>Chat Room</title><style>");
    w.push_str("*{margin:0;padding:0;box-sizing:border-box}");
    w.push_str("body{font-family:'Segoe UI',system-ui,sans-serif;background:#0a0a1a;color:#e0e0e0;height:100vh;display:flex;flex-direction:column}");
    w.push_str(".header{background:linear-gradient(135deg,#1a1a3e,#2d1b69);padding:16px 24px;border-bottom:1px solid #333;display:flex;align-items:center;gap:12px}");
    w.push_str(".header h1{font-size:1.3rem;color:#7c4dff}");
    w.push_str(".header .status{width:8px;height:8px;border-radius:50%;background:#4caf50;animation:pulse 2s infinite}");
    w.push_str("@keyframes pulse{0%,100%{opacity:1}50%{opacity:0.4}}");
    w.push_str(".online{font-size:0.8rem;color:#888}");
    w.push_str("#messages{flex:1;overflow-y:auto;padding:16px 24px;display:flex;flex-direction:column;gap:8px}");
    w.push_str(".msg{background:#151530;border-radius:12px;padding:10px 16px;max-width:75%;animation:fadeIn 0.3s ease}");
    w.push_str(".msg.self{align-self:flex-end;background:#2d1b69}");
    w.push_str(".msg .name{font-size:0.75rem;color:#7c4dff;font-weight:600;margin-bottom:4px}");
    w.push_str(".msg .text{font-size:0.9rem;line-height:1.4;word-break:break-word}");
    w.push_str(".msg .time{font-size:0.65rem;color:#555;margin-top:4px;text-align:right}");
    w.push_str("@keyframes fadeIn{from{opacity:0;transform:translateY(8px)}to{opacity:1;transform:translateY(0)}}");
    w.push_str(".input-area{background:#111;border-top:1px solid #333;padding:12px 24px;display:flex;gap:10px}");
    w.push_str("#username{width:120px;padding:10px;border-radius:8px;border:1px solid #333;background:#1a1a3e;color:#e0e0e0;font-size:0.85rem}");
    w.push_str("#msgInput{flex:1;padding:10px 16px;border-radius:8px;border:1px solid #333;background:#1a1a3e;color:#e0e0e0;font-size:0.9rem}");
    w.push_str("#msgInput:focus,#username:focus{outline:none;border-color:#7c4dff}");
    w.push_str("#sendBtn{padding:10px 24px;border-radius:8px;border:none;background:#7c4dff;color:#fff;font-weight:600;cursor:pointer;transition:background 0.2s}");
    w.push_str("#sendBtn:hover{background:#651fff}");
    w.push_str("#sendBtn:disabled{background:#444;cursor:not-allowed}");
    w.push_str(".empty{text-align:center;color:#555;margin-top:40%;font-size:1.1rem}");
    w.push_str("</style></head><body>");

    w.push_str("<div class='header'><div class='status'></div><h1>x402 Chat Room</h1><span class='online' id='onlineCount'></span></div>");
    w.push_str("<div id='messages'><div class='empty'>No messages yet. Say something!</div></div>");
    w.push_str("<div class='input-area'>");
    w.push_str("<input id='username' placeholder='Name' maxlength='20' value='anon'>");
    w.push_str("<input id='msgInput' placeholder='Type a message...' maxlength='500' autocomplete='off'>");
    w.push_str("<button id='sendBtn'>Send</button>");
    w.push_str("</div>");

    w.push_str("<script>");
    w.push_str("const msgDiv=document.getElementById('messages');");
    w.push_str("const msgInput=document.getElementById('msgInput');");
    w.push_str("const sendBtn=document.getElementById('sendBtn');");
    w.push_str("const usernameInput=document.getElementById('username');");
    w.push_str("let lastId=0;let myName='anon';");

    // Send message function
    w.push_str("async function sendMsg(){");
    w.push_str("const text=msgInput.value.trim();if(!text)return;");
    w.push_str("myName=usernameInput.value.trim()||'anon';");
    w.push_str("sendBtn.disabled=true;msgInput.value='';");
    w.push_str("try{await fetch(window.location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},");
    w.push_str("body:JSON.stringify({username:myName,message:text})});");
    w.push_str("await fetchMessages();}catch(e){console.error(e);}");
    w.push_str("sendBtn.disabled=false;msgInput.focus();}");

    // Fetch messages function
    w.push_str("async function fetchMessages(){try{");
    w.push_str("const r=await fetch(window.location.pathname+'/api/messages');");
    w.push_str("const d=await r.json();");
    w.push_str("if(!d.messages||d.messages.length===0)return;");
    w.push_str("const newMsgs=d.messages.filter(m=>m.id>lastId);");
    w.push_str("if(newMsgs.length===0)return;");
    w.push_str("if(lastId===0){msgDiv.innerHTML='';}");
    w.push_str("newMsgs.forEach(m=>{");
    w.push_str("const div=document.createElement('div');div.className='msg'+(m.user===myName?' self':'');");
    w.push_str("div.innerHTML='<div class=\"name\">'+m.user+'</div><div class=\"text\">'+m.text.replace(/</g,'&lt;')+'</div>';");
    w.push_str("msgDiv.appendChild(div);lastId=Math.max(lastId,m.id);});");
    w.push_str("msgDiv.scrollTop=msgDiv.scrollHeight;");
    w.push_str("}catch(e){console.error(e);}}");

    // Event listeners + auto-refresh
    w.push_str("sendBtn.addEventListener('click',sendMsg);");
    w.push_str("msgInput.addEventListener('keydown',e=>{if(e.key==='Enter')sendMsg();});");
    w.push_str("fetchMessages();setInterval(fetchMessages,3000);");
    w.push_str("</script></body></html>");

    respond(200, w.as_str(), "text/html");
}
