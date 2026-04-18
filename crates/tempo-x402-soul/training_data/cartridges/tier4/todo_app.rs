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
        if n == 0 {
            self.push_str("0");
            return;
        }
        let mut digits = [0u8; 10];
        let mut i = 0;
        while n > 0 {
            digits[i] = b'0' + (n % 10) as u8;
            n /= 10;
            i += 1;
        }
        while i > 0 {
            i -= 1;
            unsafe {
                if self.pos < BUF.len() {
                    BUF[self.pos] = digits[i];
                    self.pos += 1;
                }
            }
        }
    }

    fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) }
    }
}

fn starts_with(haystack: &[u8], needle: &[u8]) -> bool {
    if haystack.len() < needle.len() { return false; }
    let mut i = 0;
    while i < needle.len() {
        if haystack[i] != needle[i] { return false; }
        i += 1;
    }
    true
}

#[no_mangle]
pub extern "C" fn handle(method_ptr: *const u8, method_len: i32, _path_ptr: *const u8, _path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if starts_with(method.as_bytes(), b"POST") {
        if let Some(text) = find_json_str(body, "text") {
            let existing = kv_read("todos").unwrap_or("");
            let mut w = BufWriter::new();
            if existing.len() > 0 {
                w.push_str(existing);
                w.push_str("\n");
            }
            w.push_str("[ ] ");
            w.push_str(text);
            kv_write("todos", w.as_str());
            respond(200, "{\"ok\":true}", "application/json");
        } else if let Some(action) = find_json_str(body, "action") {
            if let Some(idx_str) = find_json_str(body, "index") {
                let idx = parse_num(idx_str);
                let existing = kv_read("todos").unwrap_or("");
                let mut w = BufWriter::new();
                let mut line_num: u32 = 0;
                let bytes = existing.as_bytes();
                let mut pos = 0;
                while pos <= bytes.len() {
                    let line_start = pos;
                    while pos < bytes.len() && bytes[pos] != b'\n' { pos += 1; }
                    let line_end = pos;
                    if pos < bytes.len() { pos += 1; }
                    if line_start == line_end && pos > bytes.len() { break; }
                    let line = unsafe { core::str::from_utf8_unchecked(&bytes[line_start..line_end]) };
                    if line.len() == 0 && line_start == line_end { line_num += 1; continue; }
                    if starts_with(action.as_bytes(), b"delete") && line_num == idx {
                        line_num += 1;
                        continue;
                    }
                    if line_num > 0 && w.pos > 0 { w.push_str("\n"); }
                    if starts_with(action.as_bytes(), b"toggle") && line_num == idx {
                        if starts_with(line.as_bytes(), b"[x]") {
                            w.push_str("[ ] ");
                            if line.len() > 4 { w.push_str(&line[4..]); }
                        } else {
                            w.push_str("[x] ");
                            if line.len() > 4 { w.push_str(&line[4..]); }
                        }
                    } else {
                        w.push_str(line);
                    }
                    line_num += 1;
                }
                kv_write("todos", w.as_str());
                respond(200, "{\"ok\":true}", "application/json");
            } else {
                respond(400, "{\"error\":\"missing index\"}", "application/json");
            }
        } else {
            respond(400, "{\"error\":\"missing text or action\"}", "application/json");
        }
        return;
    }

    // GET — return HTML app
    let todos = kv_read("todos").unwrap_or("");
    let mut w = BufWriter::new();
    w.push_str("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Todo App</title><style>");
    w.push_str("*{margin:0;padding:0;box-sizing:border-box}");
    w.push_str("body{background:#1a1a2e;color:#e0e0e0;font-family:'Segoe UI',sans-serif;min-height:100vh;display:flex;justify-content:center;padding:40px 20px}");
    w.push_str(".container{max-width:600px;width:100%}");
    w.push_str("h1{text-align:center;color:#e94560;margin-bottom:30px;font-size:2em}");
    w.push_str(".input-row{display:flex;gap:10px;margin-bottom:20px}");
    w.push_str("input[type=text]{flex:1;padding:12px 16px;border:2px solid #16213e;background:#0f3460;color:#e0e0e0;border-radius:8px;font-size:16px;outline:none}");
    w.push_str("input[type=text]:focus{border-color:#e94560}");
    w.push_str("button{padding:12px 24px;background:#e94560;color:#fff;border:none;border-radius:8px;cursor:pointer;font-size:16px;font-weight:bold;transition:background 0.2s}");
    w.push_str("button:hover{background:#c73652}");
    w.push_str(".todo-item{display:flex;align-items:center;gap:12px;padding:14px 16px;background:#16213e;border-radius:8px;margin-bottom:8px;transition:all 0.2s}");
    w.push_str(".todo-item:hover{background:#1a2744}");
    w.push_str(".todo-item.done .todo-text{text-decoration:line-through;opacity:0.5}");
    w.push_str(".check-btn{width:28px;height:28px;border-radius:50%;border:2px solid #e94560;background:transparent;cursor:pointer;display:flex;align-items:center;justify-content:center;color:#e94560;font-size:14px;flex-shrink:0}");
    w.push_str(".check-btn.checked{background:#e94560;color:#fff}");
    w.push_str(".todo-text{flex:1;font-size:16px}");
    w.push_str(".del-btn{padding:6px 12px;background:#533;color:#e94560;border:none;border-radius:6px;cursor:pointer;font-size:14px}");
    w.push_str(".del-btn:hover{background:#744}");
    w.push_str(".count{text-align:center;margin-top:20px;color:#888;font-size:14px}");
    w.push_str("</style></head><body><div class='container'><h1>Todo List</h1>");
    w.push_str("<div class='input-row'><input type='text' id='inp' placeholder='What needs to be done?' onkeydown=\"if(event.key==='Enter')addTodo()\"><button onclick='addTodo()'>Add</button></div>");
    w.push_str("<div id='list'>");

    // Render existing todos
    let bytes = todos.as_bytes();
    let mut pos = 0;
    let mut total: u32 = 0;
    let mut done: u32 = 0;
    let mut idx: u32 = 0;
    while pos < bytes.len() {
        let line_start = pos;
        while pos < bytes.len() && bytes[pos] != b'\n' { pos += 1; }
        let line = unsafe { core::str::from_utf8_unchecked(&bytes[line_start..pos]) };
        if pos < bytes.len() { pos += 1; }
        if line.len() < 4 { continue; }
        let is_done = starts_with(line.as_bytes(), b"[x]");
        let text = if line.len() > 4 { &line[4..] } else { "" };
        total += 1;
        if is_done { done += 1; }
        w.push_str("<div class='todo-item");
        if is_done { w.push_str(" done"); }
        w.push_str("'><button class='check-btn");
        if is_done { w.push_str(" checked"); }
        w.push_str("' onclick='toggleTodo(");
        w.push_num(idx);
        w.push_str(")'>");
        if is_done { w.push_str("&#10003;"); }
        w.push_str("</button><span class='todo-text'>");
        w.push_str(text);
        w.push_str("</span><button class='del-btn' onclick='deleteTodo(");
        w.push_num(idx);
        w.push_str(")'>Delete</button></div>");
        idx += 1;
    }

    w.push_str("</div><div class='count'>");
    w.push_num(done);
    w.push_str(" / ");
    w.push_num(total);
    w.push_str(" completed</div></div>");
    w.push_str("<script>");
    w.push_str("const BASE=location.pathname;");
    w.push_str("async function addTodo(){const inp=document.getElementById('inp');const text=inp.value.trim();if(!text)return;await fetch(BASE,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({text})});inp.value='';location.reload();}");
    w.push_str("async function toggleTodo(i){await fetch(BASE,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'toggle',index:String(i)})});location.reload();}");
    w.push_str("async function deleteTodo(i){await fetch(BASE,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',index:String(i)})});location.reload();}");
    w.push_str("</script></body></html>");

    respond(200, w.as_str(), "text/html");
}

fn parse_num(s: &str) -> u32 {
    let mut n: u32 = 0;
    for &b in s.as_bytes() {
        if b >= b'0' && b <= b'9' {
            n = n * 10 + (b - b'0') as u32;
        }
    }
    n
}
