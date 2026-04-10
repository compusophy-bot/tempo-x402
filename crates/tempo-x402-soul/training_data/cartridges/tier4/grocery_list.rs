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

fn kv_read(key: &str) -> Option<&'static str> {
    unsafe {
        let result = kv_get(key.as_ptr(), key.len() as i32);
        if result < 0 { return None; }
        let ptr = (result >> 32) as *const u8;
        let len = (result & 0xFFFFFFFF) as usize;
        core::str::from_utf8(core::slice::from_raw_parts(ptr, len)).ok()
    }
}

fn kv_write(key: &str, value: &str) {
    unsafe { kv_set(key.as_ptr(), key.len() as i32, value.as_ptr(), value.len() as i32); }
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

static mut BUF: [u8; 65536] = [0u8; 65536];
struct BufWriter { pos: usize }
impl BufWriter {
    fn new() -> Self { Self { pos: 0 } }
    fn push(&mut self, s: &str) {
        let b = s.as_bytes();
        unsafe {
            let end = (self.pos + b.len()).min(BUF.len());
            BUF[self.pos..end].copy_from_slice(&b[..end - self.pos]);
            self.pos = end;
        }
    }
    fn push_num(&mut self, mut n: u32) {
        if n == 0 { self.push("0"); return; }
        let mut d = [0u8; 10]; let mut i = 0;
        while n > 0 { d[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
        while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = d[i]; self.pos += 1; } } }
    }
    fn as_str(&self) -> &str { unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) } }
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 { unsafe { SCRATCH.as_mut_ptr() } }

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize)) };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(0, "grocery_list: handling request");

    if method == "POST" {
        if let Some(item) = find_json_str(body, "item") {
            let category = find_json_str(body, "category").unwrap_or("other");
            let existing = kv_read("groceries").unwrap_or("");
            let mut w = BufWriter::new();
            if !existing.is_empty() { w.push(existing); w.push("\n"); }
            w.push("[ ] ");
            w.push(category);
            w.push("|");
            w.push(item);
            kv_write("groceries", w.as_str());
            respond(200, r#"{"ok":true}"#, "application/json");
        } else if let Some(action) = find_json_str(body, "action") {
            if let Some(idx_s) = find_json_str(body, "index") {
                let idx = parse_u32(idx_s);
                let existing = kv_read("groceries").unwrap_or("");
                let mut w = BufWriter::new();
                let bytes = existing.as_bytes();
                let mut pos = 0;
                let mut line_num: u32 = 0;
                while pos <= bytes.len() {
                    let ls = pos;
                    while pos < bytes.len() && bytes[pos] != b'\n' { pos += 1; }
                    let le = pos;
                    if pos < bytes.len() { pos += 1; }
                    if ls == le && pos > bytes.len() { break; }
                    let line = unsafe { core::str::from_utf8_unchecked(&bytes[ls..le]) };
                    if line.is_empty() { line_num += 1; continue; }
                    if action == "delete" && line_num == idx { line_num += 1; continue; }
                    if w.pos > 0 { w.push("\n"); }
                    if action == "toggle" && line_num == idx {
                        if line.as_bytes().get(1) == Some(&b'x') {
                            w.push("[ ] ");
                            if line.len() > 4 { w.push(&line[4..]); }
                        } else {
                            w.push("[x] ");
                            if line.len() > 4 { w.push(&line[4..]); }
                        }
                    } else {
                        w.push(line);
                    }
                    line_num += 1;
                }
                kv_write("groceries", w.as_str());
                respond(200, r#"{"ok":true}"#, "application/json");
            } else if action == "clear_done" {
                let existing = kv_read("groceries").unwrap_or("");
                let mut w = BufWriter::new();
                let bytes = existing.as_bytes();
                let mut pos = 0;
                while pos < bytes.len() {
                    let ls = pos;
                    while pos < bytes.len() && bytes[pos] != b'\n' { pos += 1; }
                    let line = unsafe { core::str::from_utf8_unchecked(&bytes[ls..pos]) };
                    if pos < bytes.len() { pos += 1; }
                    if line.as_bytes().get(1) == Some(&b'x') { continue; }
                    if !line.is_empty() {
                        if w.pos > 0 { w.push("\n"); }
                        w.push(line);
                    }
                }
                kv_write("groceries", w.as_str());
                respond(200, r#"{"ok":true}"#, "application/json");
            } else {
                respond(400, r#"{"error":"unknown action"}"#, "application/json");
            }
        } else {
            respond(400, r#"{"error":"missing item or action"}"#, "application/json");
        }
        return;
    }

    // GET — render HTML
    let items = kv_read("groceries").unwrap_or("");
    let mut w = BufWriter::new();
    w.push("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Grocery List</title><style>");
    w.push("*{margin:0;padding:0;box-sizing:border-box}body{background:#0d1117;color:#c9d1d9;font-family:'Segoe UI',sans-serif;padding:30px 20px;display:flex;justify-content:center}");
    w.push(".c{max-width:600px;width:100%}h1{color:#58a6ff;text-align:center;margin-bottom:20px}");
    w.push(".add{display:flex;gap:8px;margin-bottom:20px}.add input,.add select{padding:10px;background:#161b22;border:1px solid #30363d;color:#c9d1d9;border-radius:6px;font-size:14px}");
    w.push(".add input{flex:1}.add select{width:120px}");
    w.push("button{padding:10px 18px;background:#238636;color:#fff;border:none;border-radius:6px;cursor:pointer;font-size:14px}button:hover{background:#2ea043}");
    w.push(".cat{margin-bottom:16px}.cat-title{font-size:13px;color:#8b949e;text-transform:uppercase;letter-spacing:1px;margin-bottom:8px;padding-bottom:4px;border-bottom:1px solid #21262d}");
    w.push(".item{display:flex;align-items:center;gap:10px;padding:10px 12px;background:#161b22;border-radius:6px;margin-bottom:4px}");
    w.push(".item.done .txt{text-decoration:line-through;opacity:0.4}");
    w.push(".chk{width:20px;height:20px;cursor:pointer}.txt{flex:1;font-size:15px}");
    w.push(".del{background:#da3633;padding:4px 10px;border-radius:4px;font-size:12px}");
    w.push(".actions{display:flex;gap:10px;margin-bottom:20px}.actions button{background:#21262d;font-size:13px;padding:8px 14px}");
    w.push("</style></head><body><div class='c'><h1>Grocery List</h1>");
    w.push("<div class='add'><input id='inp' placeholder='Add item...'><select id='cat'><option value='produce'>Produce</option><option value='dairy'>Dairy</option><option value='meat'>Meat</option><option value='bakery'>Bakery</option><option value='frozen'>Frozen</option><option value='drinks'>Drinks</option><option value='other'>Other</option></select><button onclick='addItem()'>Add</button></div>");
    w.push("<div class='actions'><button onclick='clearDone()'>Clear Completed</button></div>");
    w.push("<div id='list'>");

    // Group by category
    let categories = ["produce", "dairy", "meat", "bakery", "frozen", "drinks", "other"];
    let mut idx: u32 = 0;
    let mut cat_i = 0;
    while cat_i < categories.len() {
        let cat = categories[cat_i];
        let mut found = false;
        let bytes = items.as_bytes();
        let mut pos = 0;
        let mut cur_idx: u32 = 0;
        while pos < bytes.len() {
            let ls = pos;
            while pos < bytes.len() && bytes[pos] != b'\n' { pos += 1; }
            let line = unsafe { core::str::from_utf8_unchecked(&bytes[ls..pos]) };
            if pos < bytes.len() { pos += 1; }
            if line.len() < 4 { cur_idx += 1; continue; }
            let content = &line[4..];
            // Find category separator
            let cb = content.as_bytes();
            let mut sep = 0;
            while sep < cb.len() && cb[sep] != b'|' { sep += 1; }
            if sep < cb.len() {
                let line_cat = &content[..sep];
                if line_cat == cat {
                    if !found {
                        w.push("<div class='cat'><div class='cat-title'>");
                        w.push(cat);
                        w.push("</div>");
                        found = true;
                    }
                    let is_done = line.as_bytes().get(1) == Some(&b'x');
                    let item_name = &content[sep + 1..];
                    w.push("<div class='item");
                    if is_done { w.push(" done"); }
                    w.push("'><input type='checkbox' class='chk'");
                    if is_done { w.push(" checked"); }
                    w.push(" onchange='toggle(");
                    w.push_num(cur_idx);
                    w.push(")'><span class='txt'>");
                    w.push(item_name);
                    w.push("</span><button class='del' onclick='del(");
                    w.push_num(cur_idx);
                    w.push(")'>X</button></div>");
                }
            }
            cur_idx += 1;
        }
        if found { w.push("</div>"); }
        cat_i += 1;
    }

    w.push("</div></div>");
    w.push("<script>const B=location.pathname;");
    w.push("async function addItem(){const i=document.getElementById('inp');const c=document.getElementById('cat');const t=i.value.trim();if(!t)return;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({item:t,category:c.value})});location.reload();}");
    w.push("async function toggle(i){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'toggle',index:String(i)})});location.reload();}");
    w.push("async function del(i){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',index:String(i)})});location.reload();}");
    w.push("async function clearDone(){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'clear_done'})});location.reload();}");
    w.push("document.getElementById('inp').addEventListener('keydown',e=>{if(e.key==='Enter')addItem();});");
    w.push("</script></body></html>");
    respond(200, w.as_str(), "text/html");
}

fn parse_u32(s: &str) -> u32 {
    let mut n: u32 = 0;
    for &b in s.as_bytes() { if b >= b'0' && b <= b'9' { n = n * 10 + (b - b'0') as u32; } }
    n
}
