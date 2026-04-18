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
    while i + kb.len() + 3 < jb.len() { if jb[i] == b'"' { let s = i + 1; if s + kb.len() < jb.len() && &jb[s..s + kb.len()] == kb && jb[s + kb.len()] == b'"' { let mut j = s + kb.len() + 1; while j < jb.len() && (jb[j] == b':' || jb[j] == b' ') { j += 1; } if j < jb.len() && jb[j] == b'"' { let vs = j + 1; let mut ve = vs; while ve < jb.len() && jb[ve] != b'"' { ve += 1; } return core::str::from_utf8(&jb[vs..ve]).ok(); } } } i += 1; } None
}
static mut BUF: [u8; 65536] = [0u8; 65536];
struct W { pos: usize }
impl W {
    fn new() -> Self { Self { pos: 0 } }
    fn s(&mut self, s: &str) { let b = s.as_bytes(); unsafe { let e = (self.pos + b.len()).min(BUF.len()); BUF[self.pos..e].copy_from_slice(&b[..e - self.pos]); self.pos = e; } }
    fn n(&mut self, mut n: u32) { if n == 0 { self.s("0"); return; } let mut d = [0u8; 10]; let mut i = 0; while n > 0 { d[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; } while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = d[i]; self.pos += 1; } } } }
    fn hex(&mut self, n: u8) {
        let hi = n >> 4; let lo = n & 0xF;
        let h = if hi < 10 { b'0' + hi } else { b'a' + hi - 10 };
        let l = if lo < 10 { b'0' + lo } else { b'a' + lo - 10 };
        unsafe { if self.pos + 1 < BUF.len() { BUF[self.pos] = h; BUF[self.pos + 1] = l; self.pos += 2; } }
    }
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
    host_log(0, "color_palette_gen: handling request");

    if method == "POST" {
        if let Some(action) = find_json_str(body, "action") {
            if action == "save" {
                if let Some(palette) = find_json_str(body, "palette") {
                    let name = find_json_str(body, "name").unwrap_or("Unnamed");
                    let existing = kv_read("pal_saved").unwrap_or("");
                    let mut w = W::new();
                    if !existing.is_empty() { w.s(existing); w.s("\n"); }
                    w.s(name); w.s("|"); w.s(palette);
                    kv_write("pal_saved", w.out());
                    respond(200, r#"{"ok":true}"#, "application/json");
                } else { respond(400, r#"{"error":"missing palette"}"#, "application/json"); }
            } else if action == "generate" {
                let counter = kv_read("pal_counter").map(|s| parse_u32(s)).unwrap_or(0);
                let mut seed = counter.wrapping_mul(1103515245).wrapping_add(12345);
                let mut colors = [[0u8; 3]; 5];
                let mut i = 0;
                while i < 5 {
                    seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                    colors[i][0] = ((seed >> 16) & 0xFF) as u8;
                    seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                    colors[i][1] = ((seed >> 16) & 0xFF) as u8;
                    seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                    colors[i][2] = ((seed >> 16) & 0xFF) as u8;
                    i += 1;
                }
                let mut w = W::new();
                i = 0;
                while i < 5 {
                    if i > 0 { w.s(","); }
                    w.s("#"); w.hex(colors[i][0]); w.hex(colors[i][1]); w.hex(colors[i][2]);
                    i += 1;
                }
                kv_write("pal_current", w.out());
                let mut cw = W::new(); cw.n(counter + 1); kv_write("pal_counter", cw.out());
                let mut rw = W::new();
                rw.s(r#"{"palette":""#); rw.s(w.out()); rw.s(r#""}"#);
                respond(200, rw.out(), "application/json");
            } else { respond(400, r#"{"error":"unknown"}"#, "application/json"); }
        } else { respond(400, r#"{"error":"missing action"}"#, "application/json"); }
        return;
    }

    let current = kv_read("pal_current").unwrap_or("#ff6b6b,#ffd93d,#6bcb77,#4d96ff,#9b59b6");
    let saved = kv_read("pal_saved").unwrap_or("");

    let mut w = W::new();
    w.s("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Color Palette Generator</title><style>");
    w.s("*{margin:0;padding:0;box-sizing:border-box}body{background:#111;color:#e0e0e0;font-family:'Segoe UI',sans-serif;padding:30px 20px;display:flex;justify-content:center}");
    w.s(".c{max-width:600px;width:100%;text-align:center}h1{color:#fff;margin-bottom:24px}");
    w.s(".palette{display:flex;gap:0;border-radius:16px;overflow:hidden;margin-bottom:20px;height:200px}");
    w.s(".swatch{flex:1;cursor:pointer;position:relative;transition:flex 0.3s}.swatch:hover{flex:1.5}");
    w.s(".swatch .label{position:absolute;bottom:10px;left:50%;transform:translateX(-50%);background:rgba(0,0,0,0.7);padding:4px 10px;border-radius:4px;font-size:12px;font-family:monospace;white-space:nowrap;opacity:0;transition:opacity 0.2s}");
    w.s(".swatch:hover .label{opacity:1}");
    w.s(".btns{display:flex;gap:10px;justify-content:center;margin-bottom:20px}");
    w.s("button{padding:12px 24px;border:none;border-radius:8px;cursor:pointer;font-size:15px;font-weight:600}");
    w.s(".gen{background:#fff;color:#000}.save{background:#333;color:#fff}");
    w.s(".save-form{display:flex;gap:8px;justify-content:center;margin-bottom:20px}");
    w.s(".save-form input{padding:10px;background:#222;border:1px solid #444;color:#e0e0e0;border-radius:6px;font-size:14px}");
    w.s(".saved-section{text-align:left;margin-top:24px}.saved-section h3{color:#888;font-size:13px;text-transform:uppercase;margin-bottom:12px}");
    w.s(".saved-pal{display:flex;align-items:center;gap:12px;background:#1a1a1a;padding:10px;border-radius:8px;margin-bottom:6px}");
    w.s(".saved-pal .name{flex:1;font-size:14px}.mini-swatch{display:flex;gap:2px;border-radius:4px;overflow:hidden}");
    w.s(".mini-swatch div{width:24px;height:24px}");
    w.s("</style></head><body><div class='c'><h1>Color Palette Generator</h1>");
    w.s("<div class='palette' id='pal'>");

    // Render current palette
    let cb = current.as_bytes();
    let mut p = 0;
    while p <= cb.len() {
        let ss = p;
        while p < cb.len() && cb[p] != b',' { p += 1; }
        let color = unsafe { core::str::from_utf8_unchecked(&cb[ss..p]) };
        p += 1;
        if !color.is_empty() {
            w.s("<div class='swatch' style='background:"); w.s(color);
            w.s("' onclick=\"navigator.clipboard.writeText('"); w.s(color);
            w.s("')\"><span class='label'>"); w.s(color); w.s("</span></div>");
        }
    }

    w.s("</div><div class='btns'><button class='gen' onclick='generate()'>Generate New</button></div>");
    w.s("<div class='save-form'><input id='name' placeholder='Palette name'><button class='save' onclick='savePal()'>Save</button></div>");

    // Saved palettes
    if !saved.is_empty() {
        w.s("<div class='saved-section'><h3>Saved Palettes</h3>");
        let sb = saved.as_bytes();
        let mut sp = 0;
        while sp < sb.len() {
            let ls = sp;
            while sp < sb.len() && sb[sp] != b'\n' { sp += 1; }
            let line = unsafe { core::str::from_utf8_unchecked(&sb[ls..sp]) };
            if sp < sb.len() { sp += 1; }
            let lb = line.as_bytes();
            let mut sep = 0; while sep < lb.len() && lb[sep] != b'|' { sep += 1; }
            let name = &line[..sep];
            let colors = if sep + 1 < lb.len() { &line[sep + 1..] } else { "" };
            w.s("<div class='saved-pal'><span class='name'>"); w.s(name);
            w.s("</span><div class='mini-swatch'>");
            let ccb = colors.as_bytes();
            let mut cp = 0;
            while cp <= ccb.len() {
                let cs = cp;
                while cp < ccb.len() && ccb[cp] != b',' { cp += 1; }
                let c = unsafe { core::str::from_utf8_unchecked(&ccb[cs..cp]) };
                cp += 1;
                if !c.is_empty() { w.s("<div style='background:"); w.s(c); w.s("'></div>"); }
            }
            w.s("</div></div>");
        }
        w.s("</div>");
    }

    w.s("</div><script>const B=location.pathname;const cur='"); w.s(current); w.s("';");
    w.s("async function generate(){await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'generate'})});location.reload();}");
    w.s("async function savePal(){const n=document.getElementById('name').value.trim()||'Untitled';await fetch(B,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'save',name:n,palette:cur})});location.reload();}");
    w.s("</script></body></html>");
    respond(200, w.out(), "text/html");
}
