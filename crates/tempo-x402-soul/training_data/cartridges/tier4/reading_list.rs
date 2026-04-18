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

fn write_usize(pos: usize, mut n: usize) -> usize {
    if n == 0 { return buf_write(pos, "0"); }
    static mut DIGITS: [u8; 20] = [0u8; 20];
    let mut i = 0;
    while n > 0 { unsafe { DIGITS[i] = b'0' + (n % 10) as u8; } n /= 10; i += 1; }
    let mut p = pos;
    while i > 0 { i -= 1; let d = unsafe { DIGITS[i] }; let s = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(&d, 1)) }; p = buf_write(p, s); }
    p
}

fn parse_usize(s: &str) -> usize {
    let mut n = 0usize;
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() { if b[i] >= b'0' && b[i] <= b'9' { n = n * 10 + (b[i] - b'0') as usize; } i += 1; }
    n
}

#[no_mangle]
pub extern "C" fn handle_request(method_ptr: *const u8, method_len: i32, path_ptr: *const u8, path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if method == "POST" {
        let action = find_json_str(body, "action").unwrap_or("add");
        let title = find_json_str(body, "title").unwrap_or("");
        let author = find_json_str(body, "author").unwrap_or("");
        let idx_str = find_json_str(body, "index").unwrap_or("0");

        if action == "add" && title.len() > 0 {
            let existing = kv_read("books").unwrap_or("");
            let mut bp = 0usize;
            bp = buf_write(bp, existing);
            bp = buf_write(bp, title);
            bp = buf_write(bp, "|");
            bp = buf_write(bp, author);
            bp = buf_write(bp, "|unread\n");
            kv_write("books", buf_as_str(bp));
        } else if action == "toggle" {
            let target = parse_usize(idx_str);
            let existing = kv_read("books").unwrap_or("");
            let eb = existing.as_bytes();
            static mut UPD: [u8; 16384] = [0u8; 16384];
            let mut np = 0usize;
            let mut epos = 0;
            let mut count = 0usize;
            while epos < eb.len() {
                let mut eend = epos;
                while eend < eb.len() && eb[eend] != b'\n' { eend += 1; }
                if eend > epos {
                    let line = &eb[epos..eend];
                    if count == target {
                        let mut seps: [usize; 2] = [0; 2];
                        let mut sc = 0;
                        let mut si = 0;
                        while si < line.len() && sc < 2 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
                        if sc >= 2 {
                            let status = unsafe { core::str::from_utf8_unchecked(&line[seps[1]+1..]) };
                            let new_status = if status == "read" { "unread" } else { "read" };
                            unsafe {
                                UPD[np..np+seps[1]+1].copy_from_slice(&line[..seps[1]+1]);
                                np += seps[1] + 1;
                            }
                            let sb = new_status.as_bytes();
                            unsafe { UPD[np..np+sb.len()].copy_from_slice(sb); np += sb.len(); UPD[np] = b'\n'; np += 1; }
                        }
                    } else {
                        unsafe { UPD[np..np+line.len()].copy_from_slice(line); np += line.len(); UPD[np] = b'\n'; np += 1; }
                    }
                    count += 1;
                }
                epos = eend + 1;
            }
            kv_write("books", unsafe { core::str::from_utf8_unchecked(&UPD[..np]) });
        } else if action == "delete" {
            let target = parse_usize(idx_str);
            let existing = kv_read("books").unwrap_or("");
            let eb = existing.as_bytes();
            static mut DEL: [u8; 16384] = [0u8; 16384];
            let mut np = 0usize;
            let mut epos = 0;
            let mut count = 0usize;
            while epos < eb.len() {
                let mut eend = epos;
                while eend < eb.len() && eb[eend] != b'\n' { eend += 1; }
                if eend > epos {
                    if count != target {
                        let line = &eb[epos..eend];
                        unsafe { DEL[np..np+line.len()].copy_from_slice(line); np += line.len(); DEL[np] = b'\n'; np += 1; }
                    }
                    count += 1;
                }
                epos = eend + 1;
            }
            kv_write("books", unsafe { core::str::from_utf8_unchecked(&DEL[..np]) });
        }
    }

    let books = kv_read("books").unwrap_or("");

    let mut p = 0usize;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Reading List</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#faf4ed;color:#575279;font-family:Georgia,'Times New Roman',serif;min-height:100vh;padding:20px;display:flex;flex-direction:column;align-items:center}
h1{color:#907aa9;margin:20px 0;font-size:2.2em}
.container{width:100%;max-width:600px}
.stats{display:flex;gap:15px;margin-bottom:20px}
.stat-card{flex:1;background:#fffaf3;border-radius:12px;padding:15px;text-align:center;border:1px solid #f2e9e1;box-shadow:0 2px 8px rgba(0,0,0,0.05)}
.stat-card .num{font-size:1.8em;font-weight:bold;color:#907aa9}
.stat-card .lbl{color:#9893a5;font-size:0.85em}
.add-form{background:#fffaf3;border-radius:12px;padding:20px;margin-bottom:20px;border:1px solid #f2e9e1;box-shadow:0 2px 8px rgba(0,0,0,0.05)}
.add-form h2{color:#907aa9;margin-bottom:12px;font-size:1.1em}
.form-row{display:flex;gap:10px}
.form-row input{flex:1;padding:12px;border:1px solid #f2e9e1;border-radius:8px;background:#faf4ed;color:#575279;font-family:inherit;font-size:1em}
.form-row input:focus{outline:none;border-color:#907aa9}
.add-btn{padding:12px 20px;background:#907aa9;color:#fff;border:none;border-radius:8px;cursor:pointer;font-weight:bold;font-family:inherit}
.add-btn:hover{background:#7c6897}
.filter-row{display:flex;gap:8px;margin-bottom:15px}
.filter-btn{padding:8px 18px;border:1px solid #f2e9e1;border-radius:20px;background:#fffaf3;color:#575279;cursor:pointer;font-family:inherit;font-size:0.9em}
.filter-btn.active{background:#907aa9;color:#fff;border-color:#907aa9}
.book{background:#fffaf3;border:1px solid #f2e9e1;border-radius:12px;padding:18px;margin-bottom:10px;display:flex;align-items:center;gap:15px;box-shadow:0 2px 8px rgba(0,0,0,0.03);transition:transform 0.2s}
.book:hover{transform:translateX(4px)}
.book.read{opacity:0.7}
.book .icon{font-size:2em}
.book .info{flex:1}
.book .title{font-size:1.15em;font-weight:bold;color:#575279}
.book .author{color:#9893a5;font-size:0.9em;margin-top:2px}
.book .status{padding:4px 12px;border-radius:20px;font-size:0.8em;font-weight:bold}
.status-unread{background:#f2e9e1;color:#907aa9}
.status-read{background:#d7e6d0;color:#56949f}
.book-actions{display:flex;gap:6px}
.book-actions button{padding:6px 12px;border:none;border-radius:6px;cursor:pointer;font-size:0.85em;font-family:inherit}
.btn-toggle{background:#d7e6d0;color:#56949f}
.btn-delete{background:#f2e9e1;color:#b4637a}
.empty{text-align:center;color:#9893a5;padding:40px;font-size:1.1em;font-style:italic}
</style></head><body>
<h1>&#128214; Reading List</h1>
<div class="container">
"##);

    // Count stats
    let bb = books.as_bytes();
    let mut bpos = 0;
    let mut total = 0usize;
    let mut read_count = 0usize;
    while bpos < bb.len() {
        let mut bend = bpos;
        while bend < bb.len() && bb[bend] != b'\n' { bend += 1; }
        if bend > bpos {
            let line = &bb[bpos..bend];
            let mut seps: [usize; 2] = [0; 2];
            let mut sc = 0;
            let mut si = 0;
            while si < line.len() && sc < 2 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
            if sc >= 2 {
                total += 1;
                let status = unsafe { core::str::from_utf8_unchecked(&line[seps[1]+1..]) };
                if status == "read" { read_count += 1; }
            }
        }
        bpos = bend + 1;
    }
    let unread_count = total - read_count;

    p = buf_write(p, r##"<div class="stats"><div class="stat-card"><div class="num">"##);
    p = write_usize(p, total);
    p = buf_write(p, r##"</div><div class="lbl">Total Books</div></div><div class="stat-card"><div class="num">"##);
    p = write_usize(p, read_count);
    p = buf_write(p, r##"</div><div class="lbl">Read</div></div><div class="stat-card"><div class="num">"##);
    p = write_usize(p, unread_count);
    p = buf_write(p, r##"</div><div class="lbl">To Read</div></div></div>"##);

    p = buf_write(p, r##"<div class="add-form"><h2>Add a Book</h2><div class="form-row">
<input type="text" id="title" placeholder="Book title">
<input type="text" id="author" placeholder="Author">
<button class="add-btn" onclick="addBook()">Add</button></div></div>
<div class="filter-row">
<button class="filter-btn active" onclick="filter('all',this)">All</button>
<button class="filter-btn" onclick="filter('unread',this)">To Read</button>
<button class="filter-btn" onclick="filter('read',this)">Read</button>
</div><div id="bookList">"##);

    // Render books
    bpos = 0;
    let mut idx = 0usize;
    if total == 0 {
        p = buf_write(p, r##"<div class="empty">Your reading list is empty. Add a book to get started!</div>"##);
    }
    while bpos < bb.len() {
        let mut bend = bpos;
        while bend < bb.len() && bb[bend] != b'\n' { bend += 1; }
        if bend > bpos {
            let line = &bb[bpos..bend];
            let mut seps: [usize; 2] = [0; 2];
            let mut sc = 0;
            let mut si = 0;
            while si < line.len() && sc < 2 { if line[si] == b'|' { seps[sc] = si; sc += 1; } si += 1; }
            if sc >= 2 {
                let btitle = unsafe { core::str::from_utf8_unchecked(&line[..seps[0]]) };
                let bauthor = unsafe { core::str::from_utf8_unchecked(&line[seps[0]+1..seps[1]]) };
                let bstatus = unsafe { core::str::from_utf8_unchecked(&line[seps[1]+1..]) };
                let is_read = bstatus == "read";

                p = buf_write(p, r##"<div class="book "##);
                if is_read { p = buf_write(p, "read"); }
                p = buf_write(p, r##"" data-status=""##);
                p = buf_write(p, bstatus);
                p = buf_write(p, r##""><div class="icon">"##);
                if is_read { p = buf_write(p, "&#9989;"); } else { p = buf_write(p, "&#128213;"); }
                p = buf_write(p, r##"</div><div class="info"><div class="title">"##);
                p = buf_write(p, btitle);
                p = buf_write(p, r##"</div><div class="author">"##);
                p = buf_write(p, bauthor);
                p = buf_write(p, r##"</div></div><span class="status "##);
                if is_read { p = buf_write(p, "status-read"); } else { p = buf_write(p, "status-unread"); }
                p = buf_write(p, r##"">"##);
                if is_read { p = buf_write(p, "Read"); } else { p = buf_write(p, "To Read"); }
                p = buf_write(p, r##"</span><div class="book-actions"><button class="btn-toggle" onclick="toggleBook("##);
                p = write_usize(p, idx);
                p = buf_write(p, r##")">"##);
                if is_read { p = buf_write(p, "Unread"); } else { p = buf_write(p, "Done"); }
                p = buf_write(p, r##"</button><button class="btn-delete" onclick="delBook("##);
                p = write_usize(p, idx);
                p = buf_write(p, r##")">Delete</button></div></div>"##);
                idx += 1;
            }
        }
        bpos = bend + 1;
    }

    p = buf_write(p, r##"</div></div>
<script>
function addBook(){var t=document.getElementById('title').value;var a=document.getElementById('author').value;if(!t)return alert('Enter a book title');fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add',title:t,author:a||'Unknown'})}).then(function(){location.reload()})}
function toggleBook(i){fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'toggle',index:String(i)})}).then(function(){location.reload()})}
function delBook(i){if(!confirm('Remove this book?'))return;fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',index:String(i)})}).then(function(){location.reload()})}
function filter(f,btn){document.querySelectorAll('.filter-btn').forEach(function(b){b.classList.remove('active')});btn.classList.add('active');document.querySelectorAll('.book').forEach(function(b){if(f==='all')b.style.display='';else b.style.display=b.getAttribute('data-status')===f?'':'none'})}
</script></body></html>"##);

    respond(200, buf_as_str(p), "text/html");
}
