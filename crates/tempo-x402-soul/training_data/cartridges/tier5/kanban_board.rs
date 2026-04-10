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

// Items stored as: kb_item_{id} = "column|title|color"
// Column lists: kb_todo, kb_doing, kb_done = "id1,id2,id3"

fn make_item_key(id: u32) -> (&'static str, usize) {
    let prefix = b"kb_item_";
    let id_s = num_to_str(id);
    let ib = id_s.as_bytes();
    static mut KBUF: [u8; 32] = [0u8; 32];
    unsafe {
        KBUF[..prefix.len()].copy_from_slice(prefix);
        KBUF[prefix.len()..prefix.len()+ib.len()].copy_from_slice(ib);
        (core::str::from_utf8_unchecked(&KBUF[..prefix.len()+ib.len()]), prefix.len()+ib.len())
    }
}

fn get_item_count() -> u32 {
    parse_u32(kv_read("kb_next_id").unwrap_or("0"))
}

fn render_column(p: &mut usize, col_name: &str, col_key: &str, color: &str) {
    *p = buf_write(*p, r##"<div class="column"><div class="col-header" style="background:"##);
    *p = buf_write(*p, color);
    *p = buf_write(*p, r##""><h2>"##);
    *p = buf_write(*p, col_name);
    *p = buf_write(*p, r##"</h2><span class="count" id="count-"##);
    *p = buf_write(*p, col_key);
    *p = buf_write(*p, r##"">0</span></div><div class="items" id="col-"##);
    *p = buf_write(*p, col_key);
    *p = buf_write(*p, r##"">"##);

    let ids_str = kv_read(col_key).unwrap_or("");
    let ids_bytes = ids_str.as_bytes();
    let mut start = 0;
    let mut count: u32 = 0;
    let mut idx = 0;
    while idx <= ids_bytes.len() {
        if idx == ids_bytes.len() || ids_bytes[idx] == b',' {
            if idx > start {
                if let Ok(id_str) = core::str::from_utf8(&ids_bytes[start..idx]) {
                    let id = parse_u32(id_str);
                    let (item_key, _) = make_item_key(id);
                    if let Some(item_data) = kv_read(item_key) {
                        // item_data = "title|priority"
                        let item_bytes = item_data.as_bytes();
                        let pipe_pos = item_bytes.iter().position(|&b| b == b'|');
                        let title = if let Some(pp) = pipe_pos {
                            core::str::from_utf8(&item_bytes[..pp]).unwrap_or(item_data)
                        } else { item_data };
                        let priority = if let Some(pp) = pipe_pos {
                            core::str::from_utf8(&item_bytes[pp+1..]).unwrap_or("medium")
                        } else { "medium" };
                        let badge_color = match priority {
                            "high" => "#e74c3c",
                            "low" => "#3498db",
                            _ => "#f39c12",
                        };
                        *p = buf_write(*p, r##"<div class="card" data-id=""##);
                        *p = buf_write(*p, num_to_str(id));
                        *p = buf_write(*p, r##""><div class="card-title">"##);
                        *p = buf_write(*p, title);
                        *p = buf_write(*p, r##"</div><span class="badge" style="background:"##);
                        *p = buf_write(*p, badge_color);
                        *p = buf_write(*p, r##"">"##);
                        *p = buf_write(*p, priority);
                        *p = buf_write(*p, r##"</span><div class="card-actions">"##);
                        if col_key != "kb_todo" {
                            *p = buf_write(*p, r##"<button onclick="moveItem("##);
                            *p = buf_write(*p, num_to_str(id));
                            *p = buf_write(*p, r##",'left')">&#9664;</button>"##);
                        }
                        if col_key != "kb_done" {
                            *p = buf_write(*p, r##"<button onclick="moveItem("##);
                            *p = buf_write(*p, num_to_str(id));
                            *p = buf_write(*p, r##",'right')">&#9654;</button>"##);
                        }
                        *p = buf_write(*p, r##"<button onclick="deleteItem("##);
                        *p = buf_write(*p, num_to_str(id));
                        *p = buf_write(*p, r##")" class="del">&#10005;</button>"##);
                        *p = buf_write(*p, r##"</div></div>"##);
                        count += 1;
                    }
                }
            }
            start = idx + 1;
        }
        idx += 1;
    }
    *p = buf_write(*p, r##"</div></div>"##);
}

fn render_board() {
    let mut p = 0;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Kanban Board</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:'Segoe UI',sans-serif;background:#1a1d23;color:#e0e0e0;min-height:100vh}
.header{background:#2d3748;padding:15px 30px;display:flex;align-items:center;justify-content:space-between}
.header h1{color:#63b3ed;font-size:1.5em}
.board{display:flex;gap:20px;padding:20px;min-height:calc(100vh - 70px)}
.column{flex:1;background:#2d3748;border-radius:10px;overflow:hidden;display:flex;flex-direction:column}
.col-header{padding:15px;display:flex;justify-content:space-between;align-items:center}
.col-header h2{color:#fff;font-size:1.1em}
.count{background:rgba(255,255,255,0.2);color:#fff;padding:2px 10px;border-radius:12px;font-size:0.9em}
.items{padding:10px;flex:1;overflow-y:auto}
.card{background:#1a1d23;border-radius:8px;padding:12px;margin-bottom:10px;border-left:4px solid #63b3ed;transition:transform 0.2s}
.card:hover{transform:translateY(-2px)}
.card-title{font-weight:600;margin-bottom:8px}
.badge{color:#fff;padding:2px 8px;border-radius:4px;font-size:0.75em;font-weight:600;text-transform:uppercase}
.card-actions{display:flex;gap:5px;margin-top:8px}
.card-actions button{background:#4a5568;color:#fff;border:none;padding:5px 10px;border-radius:4px;cursor:pointer;font-size:0.85em}
.card-actions button:hover{background:#63b3ed}
.card-actions .del:hover{background:#e74c3c}
.add-form{background:#2d3748;padding:15px;border-radius:10px;margin:0 20px 20px}
.add-form input,.add-form select{background:#1a1d23;color:#e0e0e0;border:1px solid #4a5568;padding:8px 12px;border-radius:5px;margin-right:10px}
.add-form button{background:#63b3ed;color:#1a1d23;border:none;padding:8px 20px;border-radius:5px;cursor:pointer;font-weight:600}
.add-form button:hover{background:#4299e1}
</style></head><body>
<div class="header"><h1>Kanban Board</h1><span id="total"></span></div>
<div class="add-form">
<input type="text" id="new-title" placeholder="Task title...">
<select id="new-priority"><option value="low">Low</option><option value="medium" selected>Medium</option><option value="high">High</option></select>
<button onclick="addItem()">Add Task</button>
</div>
<div class="board">"##);

    render_column(&mut p, "To Do", "kb_todo", "#e74c3c");
    render_column(&mut p, "In Progress", "kb_doing", "#f39c12");
    render_column(&mut p, "Done", "kb_done", "#2ecc71");

    p = buf_write(p, r##"</div>
<script>
function addItem(){
  const t=document.getElementById('new-title').value;
  const pr=document.getElementById('new-priority').value;
  if(!t)return;
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'add',title:t,priority:pr})})
  .then(()=>location.reload());
}
function moveItem(id,dir){
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'move',id:''+id,direction:dir})})
  .then(()=>location.reload());
}
function deleteItem(id){
  if(!confirm('Delete this task?'))return;
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'delete',id:''+id})})
  .then(()=>location.reload());
}
</script></body></html>"##);
    respond(200, buf_as_str(p), "text/html");
}

fn remove_id_from_list(col_key: &str, id_str: &str) {
    let current = kv_read(col_key).unwrap_or("");
    if current.is_empty() { return; }
    let cb = current.as_bytes();
    let ib = id_str.as_bytes();
    let mut result = [0u8; 4096];
    let mut rp = 0;
    let mut start = 0;
    let mut idx = 0;
    let mut first = true;
    while idx <= cb.len() {
        if idx == cb.len() || cb[idx] == b',' {
            if idx > start {
                let segment = &cb[start..idx];
                if segment != ib {
                    if !first { result[rp] = b','; rp += 1; }
                    result[rp..rp+segment.len()].copy_from_slice(segment);
                    rp += segment.len();
                    first = false;
                }
            }
            start = idx + 1;
        }
        idx += 1;
    }
    let new_val = unsafe { core::str::from_utf8_unchecked(&result[..rp]) };
    kv_write(col_key, new_val);
}

fn append_id_to_list(col_key: &str, id_str: &str) {
    let current = kv_read(col_key).unwrap_or("");
    if current.is_empty() {
        kv_write(col_key, id_str);
    } else {
        let mut result = [0u8; 4096];
        let cb = current.as_bytes();
        result[..cb.len()].copy_from_slice(cb);
        result[cb.len()] = b',';
        let ib = id_str.as_bytes();
        result[cb.len()+1..cb.len()+1+ib.len()].copy_from_slice(ib);
        let new_val = unsafe { core::str::from_utf8_unchecked(&result[..cb.len()+1+ib.len()]) };
        kv_write(col_key, new_val);
    }
}

fn find_item_column(id_str: &str) -> Option<&'static str> {
    let cols = ["kb_todo", "kb_doing", "kb_done"];
    let mut ci = 0;
    while ci < 3 {
        let col = cols[ci];
        let list = kv_read(col).unwrap_or("");
        let lb = list.as_bytes();
        let ib = id_str.as_bytes();
        let mut start = 0;
        let mut idx = 0;
        while idx <= lb.len() {
            if idx == lb.len() || lb[idx] == b',' {
                if idx - start == ib.len() && &lb[start..idx] == ib {
                    return Some(col);
                }
                start = idx + 1;
            }
            idx += 1;
        }
        ci += 1;
    }
    None
}

#[no_mangle]
pub extern "C" fn x402_handle(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    host_log(1, "Kanban board request");

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("");

        if action == "add" {
            let title = find_json_str(body, "title").unwrap_or("Task");
            let priority = find_json_str(body, "priority").unwrap_or("medium");
            let next_id = get_item_count();
            let id_str = num_to_str(next_id);
            let (item_key, _) = make_item_key(next_id);
            // Store as "title|priority"
            let mut val = [0u8; 512];
            let tb = title.as_bytes();
            val[..tb.len()].copy_from_slice(tb);
            val[tb.len()] = b'|';
            let pb = priority.as_bytes();
            val[tb.len()+1..tb.len()+1+pb.len()].copy_from_slice(pb);
            let val_str = unsafe { core::str::from_utf8_unchecked(&val[..tb.len()+1+pb.len()]) };
            kv_write(item_key, val_str);
            append_id_to_list("kb_todo", id_str);
            kv_write("kb_next_id", num_to_str(next_id + 1));
            respond(200, r##"{"ok":true}"##, "application/json");
        } else if action == "move" {
            let id_str = find_json_str(body, "id").unwrap_or("0");
            let direction = find_json_str(body, "direction").unwrap_or("");
            if let Some(current_col) = find_item_column(id_str) {
                let target_col = match (current_col, direction) {
                    ("kb_todo", "right") => Some("kb_doing"),
                    ("kb_doing", "left") => Some("kb_todo"),
                    ("kb_doing", "right") => Some("kb_done"),
                    ("kb_done", "left") => Some("kb_doing"),
                    _ => None,
                };
                if let Some(target) = target_col {
                    remove_id_from_list(current_col, id_str);
                    append_id_to_list(target, id_str);
                }
            }
            respond(200, r##"{"ok":true}"##, "application/json");
        } else if action == "delete" {
            let id_str = find_json_str(body, "id").unwrap_or("0");
            if let Some(current_col) = find_item_column(id_str) {
                remove_id_from_list(current_col, id_str);
            }
            respond(200, r##"{"ok":true}"##, "application/json");
        } else {
            respond(400, r##"{"error":"unknown action"}"##, "application/json");
        }
        return;
    }

    render_board();
}