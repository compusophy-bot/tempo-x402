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

fn get_page_count() -> u32 {
    match kv_read("wiki_count") {
        Some(s) => parse_u32(s),
        None => 0,
    }
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

fn render_home() {
    let count = get_page_count();
    let mut p = 0;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Wiki Engine</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:Georgia,serif;background:#f5f0e8;color:#333;max-width:900px;margin:0 auto;padding:20px}
h1{color:#2c5530;border-bottom:3px solid #2c5530;padding-bottom:10px;margin-bottom:20px}
h2{color:#3a7040;margin:15px 0 10px}
.nav{background:#2c5530;padding:12px 20px;border-radius:8px;margin-bottom:20px}
.nav a{color:#fff;text-decoration:none;margin-right:20px;font-weight:bold}
.nav a:hover{text-decoration:underline}
.page-list{list-style:none}
.page-list li{padding:10px 15px;border-bottom:1px solid #ddd}
.page-list li:hover{background:#e8e0d0}
.page-list a{color:#2c5530;text-decoration:none;font-size:1.1em}
.btn{background:#2c5530;color:#fff;border:none;padding:10px 20px;border-radius:5px;cursor:pointer;font-size:1em}
.btn:hover{background:#3a7040}
input[type=text],textarea{width:100%;padding:10px;border:2px solid #ccc;border-radius:5px;font-size:1em;margin:5px 0 15px}
textarea{height:300px;font-family:monospace}
.recent{background:#fff;padding:15px;border-radius:8px;margin-top:20px;border:1px solid #ddd}
.edit-count{color:#888;font-size:0.9em;margin-left:10px}
</style></head><body>
<div class="nav"><a href="?">Home</a> <a href="?action=new">New Page</a> <a href="?action=recent">Recent Changes</a></div>
<h1>Wiki Engine</h1>
<p>A collaborative wiki with "##);
    p = buf_write(p, num_to_str(count));
    p = buf_write(p, r##" pages.</p>
<h2>All Pages</h2><ul class="page-list">"##);

    let mut i: u32 = 0;
    while i < count && i < 50 {
        let key_p = buf_write(0, ""); // dummy
        let idx_str = num_to_str(i);
        let mut kbuf = [0u8; 64];
        let prefix = b"wiki_title_";
        let idx_b = idx_str.as_bytes();
        let klen = prefix.len() + idx_b.len();
        kbuf[..prefix.len()].copy_from_slice(prefix);
        kbuf[prefix.len()..klen].copy_from_slice(idx_b);
        let key = unsafe { core::str::from_utf8_unchecked(&kbuf[..klen]) };
        if let Some(title) = kv_read(key) {
            p = buf_write(p, r##"<li><a href="?page="##);
            p = buf_write(p, idx_str);
            p = buf_write(p, r##"">"##);
            p = buf_write(p, title);
            p = buf_write(p, r##"</a></li>"##);
        }
        i += 1;
    }
    p = buf_write(p, r##"</ul>
<div class="recent"><h2>Recent Changes</h2>"##);
    if let Some(log) = kv_read("wiki_changelog") {
        p = buf_write(p, "<pre>");
        p = buf_write(p, log);
        p = buf_write(p, "</pre>");
    } else {
        p = buf_write(p, "<p>No changes yet.</p>");
    }
    p = buf_write(p, "</div></body></html>");
    respond(200, buf_as_str(p), "text/html");
}

fn render_new_page_form() {
    let page = r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>New Page - Wiki</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:Georgia,serif;background:#f5f0e8;color:#333;max-width:900px;margin:0 auto;padding:20px}
h1{color:#2c5530;margin-bottom:20px}
.nav{background:#2c5530;padding:12px 20px;border-radius:8px;margin-bottom:20px}
.nav a{color:#fff;text-decoration:none;margin-right:20px;font-weight:bold}
.btn{background:#2c5530;color:#fff;border:none;padding:10px 20px;border-radius:5px;cursor:pointer;font-size:1em}
.btn:hover{background:#3a7040}
input[type=text],textarea{width:100%;padding:10px;border:2px solid #ccc;border-radius:5px;font-size:1em;margin:5px 0 15px}
textarea{height:300px;font-family:monospace}
</style></head><body>
<div class="nav"><a href="?">Home</a> <a href="?action=new">New Page</a></div>
<h1>Create New Page</h1>
<form method="POST">
<label>Title:</label>
<input type="text" name="title" placeholder="Page title" required>
<label>Content (use [[Page Title]] for links):</label>
<textarea name="content" placeholder="Write your wiki content here..."></textarea>
<button class="btn" type="submit">Create Page</button>
</form>
<script>
document.querySelector('form').onsubmit=function(e){
  e.preventDefault();
  const t=document.querySelector('[name=title]').value;
  const c=document.querySelector('[name=content]').value;
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'create',title:t,content:c})})
  .then(r=>r.text()).then(()=>location.href='?');
};
</script></body></html>"##;
    respond(200, page, "text/html");
}

fn render_view_page(idx: u32) {
    let idx_str = num_to_str(idx);
    let mut title_key = [0u8; 64];
    let mut content_key = [0u8; 64];
    let tp = b"wiki_title_";
    let cp = b"wiki_content_";
    let ib = idx_str.as_bytes();
    title_key[..tp.len()].copy_from_slice(tp);
    title_key[tp.len()..tp.len()+ib.len()].copy_from_slice(ib);
    content_key[..cp.len()].copy_from_slice(cp);
    content_key[cp.len()..cp.len()+ib.len()].copy_from_slice(ib);
    let tk = unsafe { core::str::from_utf8_unchecked(&title_key[..tp.len()+ib.len()]) };
    let ck = unsafe { core::str::from_utf8_unchecked(&content_key[..cp.len()+ib.len()]) };
    let title = kv_read(tk).unwrap_or("Untitled");
    let content = kv_read(ck).unwrap_or("");

    let mut p = 0;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>"##);
    p = buf_write(p, title);
    p = buf_write(p, r##" - Wiki</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:Georgia,serif;background:#f5f0e8;color:#333;max-width:900px;margin:0 auto;padding:20px}
h1{color:#2c5530;border-bottom:3px solid #2c5530;padding-bottom:10px;margin-bottom:20px}
.nav{background:#2c5530;padding:12px 20px;border-radius:8px;margin-bottom:20px}
.nav a{color:#fff;text-decoration:none;margin-right:20px;font-weight:bold}
.content{background:#fff;padding:20px;border-radius:8px;border:1px solid #ddd;line-height:1.8;white-space:pre-wrap}
.btn{background:#2c5530;color:#fff;border:none;padding:10px 20px;border-radius:5px;cursor:pointer;font-size:1em;text-decoration:none;display:inline-block;margin-top:15px}
.btn:hover{background:#3a7040}
</style></head><body>
<div class="nav"><a href="?">Home</a> <a href="?action=new">New Page</a></div>
<h1>"##);
    p = buf_write(p, title);
    p = buf_write(p, r##"</h1><div class="content">"##);
    p = buf_write(p, content);
    p = buf_write(p, r##"</div>
<a class="btn" href="?action=edit&page="##);
    p = buf_write(p, idx_str);
    p = buf_write(p, r##"">Edit Page</a></body></html>"##);
    respond(200, buf_as_str(p), "text/html");
}

fn render_edit_page(idx: u32) {
    let idx_str = num_to_str(idx);
    let mut title_key = [0u8; 64];
    let mut content_key = [0u8; 64];
    let tp = b"wiki_title_";
    let cp = b"wiki_content_";
    let ib = idx_str.as_bytes();
    title_key[..tp.len()].copy_from_slice(tp);
    title_key[tp.len()..tp.len()+ib.len()].copy_from_slice(ib);
    content_key[..cp.len()].copy_from_slice(cp);
    content_key[cp.len()..cp.len()+ib.len()].copy_from_slice(ib);
    let tk = unsafe { core::str::from_utf8_unchecked(&title_key[..tp.len()+ib.len()]) };
    let ck = unsafe { core::str::from_utf8_unchecked(&content_key[..cp.len()+ib.len()]) };
    let title = kv_read(tk).unwrap_or("Untitled");
    let content = kv_read(ck).unwrap_or("");

    let mut p = 0;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Edit: "##);
    p = buf_write(p, title);
    p = buf_write(p, r##" - Wiki</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{font-family:Georgia,serif;background:#f5f0e8;color:#333;max-width:900px;margin:0 auto;padding:20px}
h1{color:#2c5530;margin-bottom:20px}
.nav{background:#2c5530;padding:12px 20px;border-radius:8px;margin-bottom:20px}
.nav a{color:#fff;text-decoration:none;margin-right:20px;font-weight:bold}
.btn{background:#2c5530;color:#fff;border:none;padding:10px 20px;border-radius:5px;cursor:pointer;font-size:1em}
input[type=text],textarea{width:100%;padding:10px;border:2px solid #ccc;border-radius:5px;font-size:1em;margin:5px 0 15px}
textarea{height:300px;font-family:monospace}
</style></head><body>
<div class="nav"><a href="?">Home</a> <a href="?page="##);
    p = buf_write(p, idx_str);
    p = buf_write(p, r##"">Back to Page</a></div>
<h1>Editing: "##);
    p = buf_write(p, title);
    p = buf_write(p, r##"</h1>
<form method="POST">
<label>Title:</label>
<input type="text" name="title" value=""##);
    p = buf_write(p, title);
    p = buf_write(p, r##"">
<label>Content:</label>
<textarea name="content">"##);
    p = buf_write(p, content);
    p = buf_write(p, r##"</textarea>
<button class="btn" type="submit">Save Changes</button>
</form>
<script>
document.querySelector('form').onsubmit=function(e){
  e.preventDefault();
  const t=document.querySelector('[name=title]').value;
  const c=document.querySelector('[name=content]').value;
  fetch('',{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({action:'edit',page:'"##);
    p = buf_write(p, idx_str);
    p = buf_write(p, r##"',title:t,content:c})})
  .then(r=>r.text()).then(()=>location.href='?page="##);
    p = buf_write(p, idx_str);
    p = buf_write(p, r##"');
};
</script></body></html>"##);
    respond(200, buf_as_str(p), "text/html");
}

fn append_changelog(entry: &str) {
    let existing = kv_read("wiki_changelog").unwrap_or("");
    let mut p = 0;
    p = buf_write(p, entry);
    p = buf_write(p, "\n");
    p = buf_write(p, existing);
    let truncated = if p > 2000 { 2000 } else { p };
    kv_write("wiki_changelog", buf_as_str(truncated));
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

#[no_mangle]
pub extern "C" fn x402_handle(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let path = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(path_ptr, path_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    host_log(1, "Wiki engine request");

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("");
        let title = find_json_str(body, "title").unwrap_or("Untitled");
        let content = find_json_str(body, "content").unwrap_or("");

        if action == "create" {
            let count = get_page_count();
            let idx_str = num_to_str(count);
            let ib = idx_str.as_bytes();
            let mut title_key = [0u8; 64];
            let mut content_key = [0u8; 64];
            let tp = b"wiki_title_";
            let cp = b"wiki_content_";
            title_key[..tp.len()].copy_from_slice(tp);
            title_key[tp.len()..tp.len()+ib.len()].copy_from_slice(ib);
            content_key[..cp.len()].copy_from_slice(cp);
            content_key[cp.len()..cp.len()+ib.len()].copy_from_slice(ib);
            let tk = unsafe { core::str::from_utf8_unchecked(&title_key[..tp.len()+ib.len()]) };
            let ck = unsafe { core::str::from_utf8_unchecked(&content_key[..cp.len()+ib.len()]) };
            kv_write(tk, title);
            kv_write(ck, content);
            let new_count = count + 1;
            kv_write("wiki_count", num_to_str(new_count));
            let mut lp = 0;
            lp = buf_write(lp, "Created: ");
            lp = buf_write(lp, title);
            append_changelog(buf_as_str(lp));
            respond(200, r##"{"ok":true}"##, "application/json");
        } else if action == "edit" {
            let page_str = find_json_str(body, "page").unwrap_or("0");
            let idx = parse_u32(page_str);
            let idx_s = num_to_str(idx);
            let ib = idx_s.as_bytes();
            let mut title_key = [0u8; 64];
            let mut content_key = [0u8; 64];
            let tp = b"wiki_title_";
            let cp = b"wiki_content_";
            title_key[..tp.len()].copy_from_slice(tp);
            title_key[tp.len()..tp.len()+ib.len()].copy_from_slice(ib);
            content_key[..cp.len()].copy_from_slice(cp);
            content_key[cp.len()..cp.len()+ib.len()].copy_from_slice(ib);
            let tk = unsafe { core::str::from_utf8_unchecked(&title_key[..tp.len()+ib.len()]) };
            let ck = unsafe { core::str::from_utf8_unchecked(&content_key[..cp.len()+ib.len()]) };
            kv_write(tk, title);
            kv_write(ck, content);
            let mut lp = 0;
            lp = buf_write(lp, "Edited: ");
            lp = buf_write(lp, title);
            append_changelog(buf_as_str(lp));
            respond(200, r##"{"ok":true}"##, "application/json");
        } else {
            respond(400, r##"{"error":"unknown action"}"##, "application/json");
        }
        return;
    }

    // GET routing
    let query = if let Some(qi) = path.as_bytes().iter().position(|&b| b == b'?') {
        &path[qi + 1..]
    } else {
        ""
    };

    if let Some(page_str) = find_query_param(query, "page") {
        let idx = parse_u32(page_str);
        if let Some(act) = find_query_param(query, "action") {
            if act == "edit" {
                render_edit_page(idx);
                return;
            }
        }
        render_view_page(idx);
    } else if let Some(action) = find_query_param(query, "action") {
        if action == "new" {
            render_new_page_form();
        } else {
            render_home();
        }
    } else {
        render_home();
    }
}