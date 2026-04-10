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

fn respond(status: i32, body: &str, ct: &str) { unsafe { response(status, body.as_ptr(), body.len() as i32, ct.as_ptr(), ct.len() as i32); } }
fn host_log(level: i32, msg: &str) { unsafe { log(level, msg.as_ptr(), msg.len() as i32); } }
fn kv_read(key: &str) -> Option<&'static str> { unsafe { let r = kv_get(key.as_ptr(), key.len() as i32); if r < 0 { return None; } let p = (r >> 32) as *const u8; let l = (r & 0xFFFFFFFF) as usize; core::str::from_utf8(core::slice::from_raw_parts(p, l)).ok() } }
fn kv_write(key: &str, val: &str) { unsafe { kv_set(key.as_ptr(), key.len() as i32, val.as_ptr(), val.len() as i32); } }

fn find_json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let kb = key.as_bytes(); let jb = json.as_bytes(); let mut i = 0;
    while i + kb.len() + 3 < jb.len() {
        if jb[i] == b'"' { let s = i + 1;
            if s + kb.len() < jb.len() && &jb[s..s + kb.len()] == kb && jb[s + kb.len()] == b'"' {
                let mut j = s + kb.len() + 1; while j < jb.len() && (jb[j] == b':' || jb[j] == b' ') { j += 1; }
                if j < jb.len() && jb[j] == b'"' { let vs = j + 1; let mut ve = vs; while ve < jb.len() && jb[ve] != b'"' { ve += 1; } return core::str::from_utf8(&jb[vs..ve]).ok(); }
            }
        } i += 1;
    } None
}

static mut BUF: [u8; 65536] = [0u8; 65536];
struct W { pos: usize }
impl W {
    fn new() -> Self { Self { pos: 0 } }
    fn s(&mut self, s: &str) { let b = s.as_bytes(); unsafe { let e = (self.pos + b.len()).min(BUF.len()); BUF[self.pos..e].copy_from_slice(&b[..e - self.pos]); self.pos = e; } }
    fn n(&mut self, mut n: u32) { if n == 0 { self.s("0"); return; } let mut d = [0u8; 10]; let mut i = 0; while n > 0 { d[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; } while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = d[i]; self.pos += 1; } } } }
    fn out(&self) -> &str { unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) } }
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];
#[no_mangle] pub extern "C" fn x402_alloc(size: i32) -> *mut u8 { unsafe { SCRATCH.as_mut_ptr() } }

fn parse_u32(s: &str) -> u32 { let mut n: u32 = 0; for &b in s.as_bytes() { if b >= b'0' && b <= b'9' { n = n * 10 + (b - b'0') as u32; } } n }

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize)) };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(0, "music_playlist: handling request");

    if method == "POST" {
        if let Some(action) = find_json_str(body, "action") {
            if action == "add" {
                let title = find_json_str(body, "title").unwrap_or("");
                let artist = find_json_str(body, "artist").unwrap_or("Unknown");
                let genre = find_json_str(body, "genre").unwrap_or("other");
                if !title.is_empty() {
                    let existing = kv_read("playlist").unwrap_or("");
                    let mut w = W::new();
                    if !existing.is_empty() { w.s(existing); w.s("\n"); }
                    w.s(title); w.s("|"); w.s(artist); w.s("|"); w.s(genre); w.s("|0");
                    kv_write("playlist", w.out());
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"title required"}"#, "application/json"); }
            } else if action == "like" {
                if let Some(idx_s) = find_json_str(body, "index") {
                    let idx = parse_u32(idx_s);
                    let existing = kv_read("playlist").unwrap_or("");
                    let mut w = W::new();
                    let eb = existing.as_bytes();
                    let mut p = 0; let mut line_num: u32 = 0;
                    while p < eb.len() {
                        let ls = p;
                        while p < eb.len() && eb[p] != b'\n' { p += 1; }
                        let line = unsafe { core::str::from_utf8_unchecked(&eb[ls..p]) };
                        if p < eb.len() { p += 1; }
                        if w.pos > 0 { w.s("\n"); }
                        if line_num == idx {
                            // Toggle like: find last | and flip 0/1
                            let lb = line.as_bytes();
                            let mut last_pipe = 0;
                            let mut pi = 0;
                            while pi < lb.len() { if lb[pi] == b'|' { last_pipe = pi; } pi += 1; }
                            w.s(&line[..last_pipe + 1]);
                            if last_pipe + 1 < lb.len() && lb[last_pipe + 1] == b'1' { w.s("0"); } else { w.s("1"); }
                        } else {
                            w.s(line);
                        }
                        line_num += 1;
                    }
                    kv_write("playlist", w.out());
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"missing index"}"#, "application/json"); }
            } else if action == "remove" {
                if let Some(idx_s) = find_json_str(body, "index") {
                    let idx = parse_u32(idx_s);
                    let existing = kv_read("playlist").unwrap_or("");
                    let mut w = W::new();
                    let eb = existing.as_bytes();
                    let mut p = 0; let mut line_num: u32 = 0;
                    while p < eb.len() {
                        let ls = p;
                        while p < eb.len() && eb[p] != b'\n' { p += 1; }
                        let line = unsafe { core::str::from_utf8_unchecked(&eb[ls..p]) };
                        if p < eb.len() { p += 1; }
                        if line_num != idx && !line.is_empty() {
                            if w.pos > 0 { w.s("\n"); }
                            w.s(line);
                        }
                        line_num += 1;
                    }
                    kv_write("playlist", w.out());
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"missing index"}"#, "application/json"); }
            } else { respond(400, r#"{"error":"unknown action"}"#, "application/json"); }
        } else { respond(400, r#"{"error":"missing action"}"#, "application/json"); }
        return;
    }

    // GET — render playlist
    let playlist = kv_read("playlist").unwrap_or("");
    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Music Playlist</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#0a0a0a;color:#e0e0e0;font-family:'Segoe UI',sans-serif;padding:30px 20px;display:flex;justify-content:center}");
    w.s(".c{max-width:650px;width:100%}h1{text-align:center;color:#1db954;margin-bottom:20px}");
    w.s(".form{display:flex;gap:8px;margin-bottom:20px;flex-wrap:wrap}");
    w.s(".form input,.form select{padding:10px;background:#181818;border:1px solid #333;color:#e0e0e0;border-radius:20px;font-size:14px}");
    w.s(".form input{flex:1;min-width:120px}");
    w.s("button{padding:10px 20px;background:#1db954;color:#000;border:none;border-radius:20px;cursor:pointer;font-size:14px;font-weight:600}");
    w.s(".track{display:flex;align-items:center;gap:14px;padding:12px 16px;background:#181818;border-radius:8px;margin-bottom:4px;transition:background 0.2s}");
    w.s(".track:hover{background:#282828}");
    w.s(".num{color:#666;font-size:14px;width:24px;text-align:right}.title{font-size:15px;font-weight:500}.artist{font-size:13px;color:#888}");
    w.s(".meta{flex:1}.genre{font-size:11px;color:#1db954;background:#0a2a14;padding:2px 8px;border-radius:10px}");
    w.s(".like-btn{background:none;border:none;cursor:pointer;font-size:20px;padding:4px 8px}.liked{color:#1db954}.not-liked{color:#555}");
    w.s(".rm{background:#333;color:#aaa;border:none;border-radius:50%;width:28px;height:28px;cursor:pointer;font-size:14px}");
    w.s(".stats{text-align:center;color:#666;font-size:13px;margin-top:16px}");
    w.s("</style></head><body><div class='c'><h1>My Playlist</h1>");
    w.s("<div class='form'><input id='title' placeholder='Song title'><input id='artist' placeholder='Artist'><select id='genre'><option>pop</option><option>rock</option><option>hip-hop</option><option>electronic</option><option>jazz</option><option>classical</option><option>other</option></select>");
    w.s("<button onclick='addSong()'>Add Song</button></div>");
    w.s("<div id='tracks'>");

    let mut total: u32 = 0;
    let mut liked: u32 = 0;
    if !playlist.is_empty() {
        let pb = playlist.as_bytes();
        let mut p = 0; let mut idx: u32 = 0;
        while p < pb.len() {
            let ls = p;
            while p < pb.len() && pb[p] != b'\n' { p += 1; }
            let line = unsafe { core::str::from_utf8_unchecked(&pb[ls..p]) };
            if p < pb.len() { p += 1; }
            if line.is_empty() { continue; }
            // Parse: title|artist|genre|liked
            let lb = line.as_bytes();
            let mut pipes = [0usize; 3]; let mut pi = 0; let mut li = 0;
            while li < lb.len() && pi < 3 { if lb[li] == b'|' { pipes[pi] = li; pi += 1; } li += 1; }
            if pi >= 3 {
                let title = &line[..pipes[0]];
                let artist = &line[pipes[0] + 1..pipes[1]];
                let genre = &line[pipes[1] + 1..pipes[2]];
                let is_liked = pipes[2] + 1 < lb.len() && lb[pipes[2] + 1] == b'1';
                total += 1;
                if is_liked { liked += 1; }

                w.s("<div class='track'><span class='num'>");
                w.n(idx + 1);
                w.s("</span><div class='meta'><div class='title'>");
                w.s(title);
                w.s("</div><div class='artist'>");
                w.s(artist);
                w.s(" <span class='genre'>");
                w.s(genre);
                w.s("</span></div></div><button class='like-btn ");
                if is_liked { w.s("liked"); } else { w.s("not-liked"); }
                w.s("' onclick='like("); w.n(idx); w.s(")'>");
                if is_liked { w.s("&#9829;"); } else { w.s("&#9825;"); }
                w.s("</button><button class='rm' onclick='rm("); w.n(idx); w.s(")'>x</button></div>");
            }
            idx += 1;
        }
    }

    w.s("</div><div class='stats'>");
    w.n(total); w.s(" songs | "); w.n(liked); w.s(" liked</div></div>");
    w.s("<script>const B=location.pathname;");
    w.s("async function addSong(){const t=document.getElementById('title').value.trim();if(!t)return;const a=document.getElementById('artist').value.trim()||'Unknown';const g=document.getElementById('genre').value;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add',title:t,artist:a,genre:g})});location.reload();}");
    w.s("async function like(i){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'like',index:String(i)})});location.reload();}");
    w.s("async function rm(i){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'remove',index:String(i)})});location.reload();}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
