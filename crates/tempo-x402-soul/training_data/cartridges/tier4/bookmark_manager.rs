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

#[no_mangle]
pub extern "C" fn handle_request(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if method == "POST" {
        let title = find_json_str(body, "title").unwrap_or("");
        let url = find_json_str(body, "url").unwrap_or("");
        let action = find_json_str(body, "action").unwrap_or("add");

        if action == "delete" {
            let idx = find_json_str(body, "index").unwrap_or("0");
            let existing = kv_read("bookmarks").unwrap_or("");
            let mut p = 0usize;
            let mut count = 0usize;
            let eb = existing.as_bytes();
            let target: usize = parse_usize(idx);
            let mut new_pos = 0usize;
            static mut NEW_BM: [u8; 16384] = [0u8; 16384];
            while p < eb.len() {
                let mut end = p;
                while end < eb.len() && eb[end] != b'\n' { end += 1; }
                if count != target {
                    let line = &eb[p..end];
                    unsafe {
                        NEW_BM[new_pos..new_pos + line.len()].copy_from_slice(line);
                        new_pos += line.len();
                        if new_pos < NEW_BM.len() {
                            NEW_BM[new_pos] = b'\n';
                            new_pos += 1;
                        }
                    }
                }
                count += 1;
                p = end + 1;
            }
            let new_val = unsafe { core::str::from_utf8_unchecked(&NEW_BM[..new_pos]) };
            kv_write("bookmarks", new_val);
        } else if title.len() > 0 && url.len() > 0 {
            let existing = kv_read("bookmarks").unwrap_or("");
            let mut p = 0usize;
            p = buf_write(p, existing);
            p = buf_write(p, title);
            p = buf_write(p, "|");
            p = buf_write(p, url);
            p = buf_write(p, "\n");
            kv_write("bookmarks", buf_as_str(p));
        }
    }

    let bookmarks = kv_read("bookmarks").unwrap_or("");
    let mut p = 0usize;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Bookmark Manager</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#1a1a2e;color:#eee;font-family:'Segoe UI',sans-serif;min-height:100vh;display:flex;flex-direction:column;align-items:center;padding:20px}
h1{color:#e94560;margin:20px 0;font-size:2em}
.container{width:100%;max-width:600px}
.add-form{background:#16213e;padding:20px;border-radius:12px;margin-bottom:20px;display:flex;flex-direction:column;gap:10px}
.add-form input{padding:12px;border:1px solid #0f3460;border-radius:8px;background:#1a1a2e;color:#eee;font-size:1em}
.add-form input:focus{outline:none;border-color:#e94560}
.add-form button{padding:12px;background:#e94560;color:#fff;border:none;border-radius:8px;font-size:1em;cursor:pointer;font-weight:bold}
.add-form button:hover{background:#c81e45}
.bookmark{background:#16213e;padding:15px;border-radius:10px;margin-bottom:10px;display:flex;justify-content:space-between;align-items:center}
.bookmark a{color:#53d8fb;text-decoration:none;font-size:1.05em;word-break:break-all}
.bookmark a:hover{text-decoration:underline}
.bookmark .title{color:#eee;font-weight:bold;margin-bottom:4px}
.bookmark .del{background:#e94560;color:#fff;border:none;border-radius:6px;padding:6px 14px;cursor:pointer;font-size:0.9em}
.bookmark .del:hover{background:#c81e45}
.empty{text-align:center;color:#888;padding:40px;font-size:1.1em}
.count{color:#888;margin-bottom:15px;font-size:0.9em}
</style></head><body>
<h1>&#128278; Bookmark Manager</h1>
<div class="container">
<div class="add-form">
<input type="text" id="title" placeholder="Bookmark title...">
<input type="text" id="url" placeholder="https://example.com">
<button onclick="addBookmark()">Add Bookmark</button>
</div>
<div id="list">
"##);

    let bb = bookmarks.as_bytes();
    let mut idx = 0usize;
    let mut pos = 0usize;
    let mut count = 0usize;
    while pos < bb.len() {
        let mut end = pos;
        while end < bb.len() && bb[end] != b'\n' { end += 1; }
        if end > pos {
            let line = unsafe { core::str::from_utf8_unchecked(&bb[pos..end]) };
            let mut sep = 0;
            let lb = line.as_bytes();
            let mut si = 0;
            while si < lb.len() {
                if lb[si] == b'|' { sep = si; break; }
                si += 1;
            }
            if sep > 0 {
                let title = unsafe { core::str::from_utf8_unchecked(&lb[..sep]) };
                let url = unsafe { core::str::from_utf8_unchecked(&lb[sep+1..]) };
                p = buf_write(p, r##"<div class="bookmark"><div><div class="title">"##);
                p = buf_write(p, title);
                p = buf_write(p, r##"</div><a href=""##);
                p = buf_write(p, url);
                p = buf_write(p, r##"" target="_blank">"##);
                p = buf_write(p, url);
                p = buf_write(p, r##"</a></div><button class="del" onclick="delBookmark("##);
                p = write_usize(p, count);
                p = buf_write(p, r##")">Delete</button></div>"##);
                count += 1;
            }
        }
        pos = end + 1;
    }

    if count == 0 {
        p = buf_write(p, r##"<div class="empty">No bookmarks yet. Add your first one above!</div>"##);
    }

    p = buf_write(p, r##"</div></div>
<script>
function addBookmark(){
  var t=document.getElementById('title').value;
  var u=document.getElementById('url').value;
  if(!t||!u)return alert('Please fill in both fields');
  fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({title:t,url:u,action:'add'})}).then(()=>location.reload());
}
function delBookmark(i){
  fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',index:String(i)})}).then(()=>location.reload());
}
</script></body></html>"##);

    respond(200, buf_as_str(p), "text/html");
}

fn parse_usize(s: &str) -> usize {
    let mut n = 0usize;
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] >= b'0' && b[i] <= b'9' {
            n = n * 10 + (b[i] - b'0') as usize;
        }
        i += 1;
    }
    n
}

fn write_usize(pos: usize, mut n: usize) -> usize {
    if n == 0 {
        return buf_write(pos, "0");
    }
    static mut DIGITS: [u8; 20] = [0u8; 20];
    let mut i = 0;
    while n > 0 {
        unsafe { DIGITS[i] = b'0' + (n % 10) as u8; }
        n /= 10;
        i += 1;
    }
    let mut p = pos;
    while i > 0 {
        i -= 1;
        let d = unsafe { DIGITS[i] };
        let s = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(&d, 1)) };
        p = buf_write(p, s);
    }
    p
}
