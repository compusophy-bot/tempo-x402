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

fn parse_num(s: &str) -> u32 {
    let mut n: u32 = 0;
    for &b in s.as_bytes() {
        if b >= b'0' && b <= b'9' {
            n = n * 10 + (b - b'0') as u32;
        }
    }
    n
}

fn make_key<'a>(buf: &'a mut [u8; 32], prefix: &str, num: u32) -> &'a str {
    let pb = prefix.as_bytes();
    let mut pos = 0;
    while pos < pb.len() && pos < 32 {
        buf[pos] = pb[pos];
        pos += 1;
    }
    let mut n = num;
    if n == 0 { buf[pos] = b'0'; pos += 1; }
    else {
        let start = pos;
        while n > 0 { buf[pos] = b'0' + (n % 10) as u8; n /= 10; pos += 1; }
        buf[start..pos].reverse();
    }
    unsafe { core::str::from_utf8_unchecked(&buf[..pos]) }
}

fn num_to_str<'a>(buf: &'a mut [u8; 10], mut n: u32) -> &'a str {
    if n == 0 { buf[0] = b'0'; return unsafe { core::str::from_utf8_unchecked(&buf[..1]) }; }
    let mut pos = 0;
    while n > 0 { buf[pos] = b'0' + (n % 10) as u8; n /= 10; pos += 1; }
    buf[..pos].reverse();
    unsafe { core::str::from_utf8_unchecked(&buf[..pos]) }
}

#[no_mangle]
pub extern "C" fn handle(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let path = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(path_ptr, path_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if method.as_bytes()[0] == b'P' {
        if let Some(action) = find_json_str(body, "action") {
            if bytes_eq(action.as_bytes(), b"create") {
                let title = find_json_str(body, "title").unwrap_or("Untitled");
                let content = find_json_str(body, "content").unwrap_or("");
                let count = parse_num(kv_read("note_count").unwrap_or("0"));
                let new_id = count + 1;

                let mut w = BufWriter::new();
                w.push_str("{\"title\":\"");
                w.push_str(title);
                w.push_str("\",\"content\":\"");
                w.push_str(content);
                w.push_str("\"}");

                let mut kb = [0u8; 32];
                kv_write(make_key(&mut kb, "note_", new_id), w.as_str());
                let mut nb = [0u8; 10];
                kv_write("note_count", num_to_str(&mut nb, new_id));

                // Add to index
                let idx = kv_read("note_index").unwrap_or("");
                let mut iw = BufWriter::new();
                if idx.len() > 0 { iw.push_str(idx); iw.push_str(","); }
                iw.push_num(new_id);
                kv_write("note_index", iw.as_str());

                respond(200, "{\"ok\":true}", "application/json");
            } else if bytes_eq(action.as_bytes(), b"update") {
                if let Some(id_str) = find_json_str(body, "id") {
                    let id = parse_num(id_str);
                    let title = find_json_str(body, "title").unwrap_or("Untitled");
                    let content = find_json_str(body, "content").unwrap_or("");
                    let mut w = BufWriter::new();
                    w.push_str("{\"title\":\"");
                    w.push_str(title);
                    w.push_str("\",\"content\":\"");
                    w.push_str(content);
                    w.push_str("\"}");
                    let mut kb = [0u8; 32];
                    kv_write(make_key(&mut kb, "note_", id), w.as_str());
                    respond(200, "{\"ok\":true}", "application/json");
                } else {
                    respond(400, "{\"error\":\"missing id\"}", "application/json");
                }
            } else if bytes_eq(action.as_bytes(), b"delete") {
                if let Some(id_str) = find_json_str(body, "id") {
                    let id = parse_num(id_str);
                    let mut kb = [0u8; 32];
                    kv_write(make_key(&mut kb, "note_", id), "");
                    // Remove from index
                    let idx = kv_read("note_index").unwrap_or("");
                    let mut iw = BufWriter::new();
                    let bytes = idx.as_bytes();
                    let mut p = 0;
                    let mut first = true;
                    while p <= bytes.len() {
                        let start = p;
                        while p < bytes.len() && bytes[p] != b',' { p += 1; }
                        let segment = unsafe { core::str::from_utf8_unchecked(&bytes[start..p]) };
                        if p < bytes.len() { p += 1; }
                        if parse_num(segment) == id { continue; }
                        if segment.len() == 0 { continue; }
                        if !first { iw.push_str(","); }
                        iw.push_str(segment);
                        first = false;
                    }
                    kv_write("note_index", iw.as_str());
                    respond(200, "{\"ok\":true}", "application/json");
                } else {
                    respond(400, "{\"error\":\"missing id\"}", "application/json");
                }
            } else {
                respond(400, "{\"error\":\"unknown action\"}", "application/json");
            }
        } else {
            respond(400, "{\"error\":\"missing action\"}", "application/json");
        }
        return;
    }

    // GET: render notes app
    let idx = kv_read("note_index").unwrap_or("");
    let mut w = BufWriter::new();
    w.push_str("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Notes</title><style>");
    w.push_str("*{margin:0;padding:0;box-sizing:border-box}");
    w.push_str("body{background:#1e1e2e;color:#cdd6f4;font-family:'Segoe UI',sans-serif;height:100vh;display:flex}");
    w.push_str(".sidebar{width:280px;background:#181825;border-right:1px solid #313244;height:100vh;overflow-y:auto;flex-shrink:0}");
    w.push_str(".sidebar h2{padding:20px;color:#cba6f7;font-size:18px;border-bottom:1px solid #313244}");
    w.push_str(".new-btn{width:calc(100% - 24px);margin:12px;padding:10px;background:#cba6f7;color:#1e1e2e;border:none;border-radius:8px;font-size:14px;font-weight:bold;cursor:pointer}");
    w.push_str(".new-btn:hover{background:#b48ef0}");
    w.push_str(".note-list{list-style:none}");
    w.push_str(".note-item{padding:12px 20px;cursor:pointer;border-bottom:1px solid #313244;transition:background 0.15s}");
    w.push_str(".note-item:hover{background:#313244}");
    w.push_str(".note-item.active{background:#45475a;border-left:3px solid #cba6f7}");
    w.push_str(".note-item .title{font-weight:600;font-size:14px;color:#cdd6f4}");
    w.push_str(".note-item .preview{font-size:12px;color:#6c7086;margin-top:4px;overflow:hidden;white-space:nowrap;text-overflow:ellipsis}");
    w.push_str(".editor{flex:1;display:flex;flex-direction:column;height:100vh}");
    w.push_str(".editor-header{padding:16px 24px;border-bottom:1px solid #313244;display:flex;align-items:center;gap:12px}");
    w.push_str(".title-input{flex:1;background:transparent;border:none;color:#cdd6f4;font-size:22px;font-weight:bold;outline:none}");
    w.push_str(".delete-btn{padding:8px 16px;background:#45475a;color:#f38ba8;border:none;border-radius:6px;cursor:pointer;font-size:13px}");
    w.push_str(".delete-btn:hover{background:#f38ba8;color:#1e1e2e}");
    w.push_str(".save-btn{padding:8px 16px;background:#a6e3a1;color:#1e1e2e;border:none;border-radius:6px;cursor:pointer;font-size:13px;font-weight:bold}");
    w.push_str(".content-area{flex:1;padding:24px;background:#1e1e2e}");
    w.push_str(".content-area textarea{width:100%;height:100%;background:transparent;border:none;color:#cdd6f4;font-size:15px;line-height:1.7;resize:none;outline:none;font-family:'Segoe UI',sans-serif}");
    w.push_str(".empty-state{flex:1;display:flex;align-items:center;justify-content:center;color:#6c7086;font-size:18px}");
    w.push_str("</style></head><body>");
    w.push_str("<div class='sidebar'><h2>My Notes</h2>");
    w.push_str("<button class='new-btn' onclick='createNote()'>+ New Note</button>");
    w.push_str("<ul class='note-list' id='noteList'>");

    // Render note list from index
    let bytes = idx.as_bytes();
    let mut p = 0;
    let mut note_count: u32 = 0;
    while p <= bytes.len() && idx.len() > 0 {
        let start = p;
        while p < bytes.len() && bytes[p] != b',' { p += 1; }
        let segment = unsafe { core::str::from_utf8_unchecked(&bytes[start..p]) };
        if p < bytes.len() { p += 1; }
        if segment.len() == 0 { continue; }
        let id = parse_num(segment);
        let mut kb = [0u8; 32];
        if let Some(data) = kv_read(make_key(&mut kb, "note_", id)) {
            if data.len() == 0 { continue; }
            let title = find_json_str(data, "title").unwrap_or("Untitled");
            let content = find_json_str(data, "content").unwrap_or("");
            w.push_str("<li class='note-item' onclick='selectNote(");
            w.push_num(id);
            w.push_str(")' id='ni_");
            w.push_num(id);
            w.push_str("'><div class='title'>");
            w.push_str(title);
            w.push_str("</div><div class='preview'>");
            // Show first 50 chars of content
            let preview_len = if content.len() > 50 { 50 } else { content.len() };
            w.push_str(unsafe { core::str::from_utf8_unchecked(&content.as_bytes()[..preview_len]) });
            w.push_str("</div></li>");
            note_count += 1;
        }
    }

    w.push_str("</ul></div>");
    w.push_str("<div class='editor' id='editorPane'>");
    w.push_str("<div class='empty-state' id='emptyState'>Select or create a note</div>");
    w.push_str("<div id='editorContent' style='display:none;height:100%;display:flex;flex-direction:column'>");
    w.push_str("<div class='editor-header'><input class='title-input' id='noteTitle' placeholder='Note title'>");
    w.push_str("<button class='save-btn' onclick='saveNote()'>Save</button>");
    w.push_str("<button class='delete-btn' onclick='deleteNote()'>Delete</button></div>");
    w.push_str("<div class='content-area'><textarea id='noteContent' placeholder='Start writing...'></textarea></div>");
    w.push_str("</div></div>");

    w.push_str("<script>");
    w.push_str("const BASE=location.pathname;let currentId=null;const notes={};");
    // Embed note data in JS
    let bytes2 = idx.as_bytes();
    let mut p2 = 0;
    while p2 <= bytes2.len() && idx.len() > 0 {
        let start = p2;
        while p2 < bytes2.len() && bytes2[p2] != b',' { p2 += 1; }
        let seg = unsafe { core::str::from_utf8_unchecked(&bytes2[start..p2]) };
        if p2 < bytes2.len() { p2 += 1; }
        if seg.len() == 0 { continue; }
        let id = parse_num(seg);
        let mut kb = [0u8; 32];
        if let Some(data) = kv_read(make_key(&mut kb, "note_", id)) {
            if data.len() == 0 { continue; }
            let title = find_json_str(data, "title").unwrap_or("Untitled");
            let content = find_json_str(data, "content").unwrap_or("");
            w.push_str("notes[");
            w.push_num(id);
            w.push_str("]={title:'");
            w.push_str(title);
            w.push_str("',content:'");
            w.push_str(content);
            w.push_str("'};");
        }
    }
    w.push_str("function selectNote(id){currentId=id;document.getElementById('emptyState').style.display='none';document.getElementById('editorContent').style.display='flex';document.getElementById('noteTitle').value=notes[id].title;document.getElementById('noteContent').value=notes[id].content;document.querySelectorAll('.note-item').forEach(el=>el.classList.remove('active'));const el=document.getElementById('ni_'+id);if(el)el.classList.add('active');}");
    w.push_str("async function createNote(){const r=await fetch(BASE,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'create',title:'New Note',content:''})});location.reload();}");
    w.push_str("async function saveNote(){if(!currentId)return;const title=document.getElementById('noteTitle').value;const content=document.getElementById('noteContent').value;await fetch(BASE,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'update',id:String(currentId),title,content})});location.reload();}");
    w.push_str("async function deleteNote(){if(!currentId)return;if(!confirm('Delete this note?'))return;await fetch(BASE,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',id:String(currentId)})});location.reload();}");
    w.push_str("</script></body></html>");

    respond(200, w.as_str(), "text/html");
}

fn bytes_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    let mut i = 0;
    while i < a.len() { if a[i] != b[i] { return false; } i += 1; }
    true
}
