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

fn write_key(buf: &mut [u8], prefix: &[u8], num: u32) -> usize {
    let mut pos = 0;
    for &b in prefix { buf[pos] = b; pos += 1; }
    if num == 0 { buf[pos] = b'0'; return pos + 1; }
    let mut d = [0u8; 10]; let mut di = 0; let mut n = num;
    while n > 0 { d[di] = b'0' + (n % 10) as u8; n /= 10; di += 1; }
    while di > 0 { di -= 1; buf[pos] = d[di]; pos += 1; }
    pos
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize)) };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(0, "address_book: handling request");

    if method == "POST" {
        if let Some(action) = find_json_str(body, "action") {
            if action == "add" {
                let name = find_json_str(body, "name").unwrap_or("");
                let email = find_json_str(body, "email").unwrap_or("");
                let phone = find_json_str(body, "phone").unwrap_or("");
                let group = find_json_str(body, "group").unwrap_or("general");
                if !name.is_empty() {
                    let count = kv_read("ab_count").map(|s| parse_u32(s)).unwrap_or(0);
                    let mut w = W::new();
                    w.s(name); w.s("|"); w.s(email); w.s("|"); w.s(phone); w.s("|"); w.s(group);
                    let mut kb = [0u8; 16];
                    let kl = write_key(&mut kb, b"ab_", count);
                    let key = unsafe { core::str::from_utf8_unchecked(&kb[..kl]) };
                    kv_write(key, w.out());
                    let mut cw = W::new(); cw.n(count + 1);
                    kv_write("ab_count", cw.out());
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"name required"}"#, "application/json"); }
            } else if action == "delete" {
                if let Some(idx_s) = find_json_str(body, "index") {
                    let idx = parse_u32(idx_s);
                    let mut kb = [0u8; 16];
                    let kl = write_key(&mut kb, b"ab_", idx);
                    let key = unsafe { core::str::from_utf8_unchecked(&kb[..kl]) };
                    kv_write(key, "");
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"missing index"}"#, "application/json"); }
            } else { respond(400, r#"{"error":"unknown action"}"#, "application/json"); }
        } else { respond(400, r#"{"error":"missing action"}"#, "application/json"); }
        return;
    }

    // GET — render address book
    let count = kv_read("ab_count").map(|s| parse_u32(s)).unwrap_or(0);
    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Address Book</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#0f0f1a;color:#d4d4d4;font-family:'Segoe UI',sans-serif;padding:30px 20px;display:flex;justify-content:center}");
    w.s(".c{max-width:700px;width:100%}h1{text-align:center;color:#64b5f6;margin-bottom:20px}");
    w.s(".form{background:#1a1a2e;padding:16px;border-radius:10px;margin-bottom:20px;display:grid;grid-template-columns:1fr 1fr;gap:8px}");
    w.s(".form input,.form select{padding:10px;background:#111;border:1px solid #333;color:#d4d4d4;border-radius:6px;font-size:14px}");
    w.s(".form .full{grid-column:1/-1}");
    w.s("button{padding:10px 18px;background:#1976d2;color:#fff;border:none;border-radius:6px;cursor:pointer;font-size:14px}");
    w.s(".card{background:#1a1a2e;padding:16px;border-radius:10px;margin-bottom:8px;display:flex;gap:16px;align-items:center}");
    w.s(".avatar{width:48px;height:48px;border-radius:50%;background:#1976d2;display:flex;align-items:center;justify-content:center;font-size:20px;font-weight:bold;color:#fff;flex-shrink:0}");
    w.s(".info{flex:1}.info .name{font-size:16px;font-weight:600;color:#e0e0e0}.info .detail{font-size:13px;color:#888;margin-top:2px}");
    w.s(".group-badge{padding:2px 8px;border-radius:10px;font-size:11px;background:#1a3a5a;color:#64b5f6}");
    w.s(".del{background:#c62828;padding:6px 12px;font-size:12px;border-radius:4px}");
    w.s(".empty{text-align:center;color:#555;padding:40px;font-size:16px}");
    w.s("</style></head><body><div class='c'><h1>Address Book</h1>");
    w.s("<div class='form'><input id='name' placeholder='Name'><input id='email' placeholder='Email'><input id='phone' placeholder='Phone'><select id='group'><option>general</option><option>family</option><option>work</option><option>friends</option></select>");
    w.s("<button class='full' onclick='addContact()'>Add Contact</button></div>");
    w.s("<div id='contacts'>");

    if count == 0 {
        w.s("<div class='empty'>No contacts yet. Add one above!</div>");
    } else {
        let mut i: u32 = 0;
        while i < count {
            let mut kb = [0u8; 16];
            let kl = write_key(&mut kb, b"ab_", i);
            let key = unsafe { core::str::from_utf8_unchecked(&kb[..kl]) };
            if let Some(data) = kv_read(key) {
                if !data.is_empty() {
                    let db = data.as_bytes();
                    let mut parts = [0usize; 4];
                    let mut pi = 0; let mut di = 0;
                    while di < db.len() && pi < 3 { if db[di] == b'|' { parts[pi] = di; pi += 1; } di += 1; }
                    parts[3] = db.len();
                    let name = &data[..parts[0]];
                    let email = if parts[0] + 1 < parts[1] { &data[parts[0] + 1..parts[1]] } else { "" };
                    let phone = if parts[1] + 1 < parts[2] { &data[parts[1] + 1..parts[2]] } else { "" };
                    let group = if parts[2] + 1 < db.len() { &data[parts[2] + 1..] } else { "general" };

                    let initial = if !name.is_empty() { &name[..1] } else { "?" };
                    w.s("<div class='card'><div class='avatar'>");
                    w.s(initial);
                    w.s("</div><div class='info'><div class='name'>");
                    w.s(name);
                    w.s(" <span class='group-badge'>");
                    w.s(group);
                    w.s("</span></div><div class='detail'>");
                    if !email.is_empty() { w.s(email); w.s(" | "); }
                    w.s(phone);
                    w.s("</div></div><button class='del' onclick='del(");
                    w.n(i);
                    w.s(")'>Delete</button></div>");
                }
            }
            i += 1;
        }
    }

    w.s("</div></div><script>const B=location.pathname;");
    w.s("async function addContact(){const n=document.getElementById('name').value.trim();if(!n)return;const e=document.getElementById('email').value.trim();const p=document.getElementById('phone').value.trim();const g=document.getElementById('group').value;await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'add',name:n,email:e,phone:p,group:g})});location.reload();}");
    w.s("async function del(i){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',index:String(i)})});location.reload();}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
