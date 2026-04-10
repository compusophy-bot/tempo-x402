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
        if n == 0 { self.push_str("0"); return; }
        let mut digits = [0u8; 10];
        let mut i = 0;
        while n > 0 { digits[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
        while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = digits[i]; self.pos += 1; } } }
    }

    fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) }
    }
}

#[no_mangle]
pub extern "C" fn handle(method_ptr: *const u8, method_len: i32, _path_ptr: *const u8, _path_len: i32, body_ptr: *const u8, body_len: i32) {
    let method = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(method_ptr, method_len as usize)) };
    let body = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(body_ptr, body_len as usize)) };

    if method.as_bytes()[0] == b'P' {
        if let Some(action) = find_json_str(body, "action") {
            if action.as_bytes() == b"save" {
                if let Some(color) = find_json_str(body, "color") {
                    let existing = kv_read("fav_colors").unwrap_or("");
                    let mut w = BufWriter::new();
                    if existing.len() > 0 {
                        w.push_str(existing);
                        w.push_str(",");
                    }
                    w.push_str(color);
                    kv_write("fav_colors", w.as_str());
                    respond(200, "{\"ok\":true}", "application/json");
                } else {
                    respond(400, "{\"error\":\"missing color\"}", "application/json");
                }
            } else if action.as_bytes() == b"clear" {
                kv_write("fav_colors", "");
                respond(200, "{\"ok\":true}", "application/json");
            } else {
                respond(400, "{\"error\":\"unknown action\"}", "application/json");
            }
        } else {
            respond(400, "{\"error\":\"missing action\"}", "application/json");
        }
        return;
    }

    let favs = kv_read("fav_colors").unwrap_or("");
    let mut w = BufWriter::new();
    w.push_str("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Color Picker</title><style>");
    w.push_str("*{margin:0;padding:0;box-sizing:border-box}");
    w.push_str("body{background:#111;color:#eee;font-family:'Segoe UI',sans-serif;min-height:100vh;display:flex;justify-content:center;padding:40px 20px}");
    w.push_str(".container{max-width:600px;width:100%}");
    w.push_str("h1{text-align:center;font-size:2em;margin-bottom:32px;background:linear-gradient(90deg,#f87171,#fb923c,#facc15,#4ade80,#38bdf8,#a78bfa);-webkit-background-clip:text;-webkit-text-fill-color:transparent}");
    w.push_str(".picker-area{display:flex;flex-direction:column;align-items:center;gap:24px;margin-bottom:32px}");
    w.push_str(".preview{width:200px;height:200px;border-radius:20px;border:4px solid #333;transition:background 0.2s;box-shadow:0 0 40px rgba(255,255,255,0.1)}");
    w.push_str(".controls{width:100%;max-width:400px}");
    w.push_str(".slider-group{margin-bottom:16px}");
    w.push_str(".slider-label{display:flex;justify-content:space-between;margin-bottom:6px;font-size:14px;color:#aaa}");
    w.push_str("input[type=range]{width:100%;height:8px;-webkit-appearance:none;background:#333;border-radius:4px;outline:none}");
    w.push_str("input[type=range]::-webkit-slider-thumb{-webkit-appearance:none;width:20px;height:20px;border-radius:50%;cursor:pointer}");
    w.push_str(".r-slider::-webkit-slider-thumb{background:#ef4444}");
    w.push_str(".g-slider::-webkit-slider-thumb{background:#22c55e}");
    w.push_str(".b-slider::-webkit-slider-thumb{background:#3b82f6}");
    w.push_str(".color-input{display:flex;align-items:center;gap:12px;justify-content:center;margin-bottom:16px}");
    w.push_str(".hex-display{background:#222;border:2px solid #444;border-radius:8px;padding:10px 20px;font-size:20px;font-family:monospace;color:#fff;text-align:center;min-width:120px}");
    w.push_str(".native-picker{width:60px;height:40px;border:none;border-radius:8px;cursor:pointer;background:transparent}");
    w.push_str(".btn-row{display:flex;gap:10px;justify-content:center}");
    w.push_str(".save-btn{padding:10px 24px;background:#22c55e;color:#000;border:none;border-radius:8px;font-weight:bold;cursor:pointer;font-size:14px}");
    w.push_str(".copy-btn{padding:10px 24px;background:#3b82f6;color:#fff;border:none;border-radius:8px;font-weight:bold;cursor:pointer;font-size:14px}");
    w.push_str(".clear-btn{padding:10px 24px;background:#333;color:#aaa;border:none;border-radius:8px;cursor:pointer;font-size:14px}");
    w.push_str(".favorites{margin-top:32px}");
    w.push_str(".favorites h3{color:#888;font-size:14px;text-transform:uppercase;letter-spacing:2px;margin-bottom:12px}");
    w.push_str(".fav-grid{display:flex;flex-wrap:wrap;gap:8px}");
    w.push_str(".fav-swatch{width:48px;height:48px;border-radius:10px;cursor:pointer;border:2px solid transparent;transition:all 0.2s}");
    w.push_str(".fav-swatch:hover{border-color:#fff;transform:scale(1.1)}");
    w.push_str(".rgb-display{text-align:center;color:#888;font-size:14px;font-family:monospace;margin-bottom:16px}");
    w.push_str("</style></head><body><div class='container'>");
    w.push_str("<h1>Color Picker</h1>");
    w.push_str("<div class='picker-area'>");
    w.push_str("<div class='preview' id='preview' style='background:#ff6600'></div>");
    w.push_str("<div class='color-input'><input type='color' class='native-picker' id='nativePicker' value='#ff6600' onchange='fromNative(this.value)'><div class='hex-display' id='hexDisplay'>#FF6600</div></div>");
    w.push_str("<div class='rgb-display' id='rgbDisplay'>rgb(255, 102, 0)</div>");
    w.push_str("<div class='controls'>");
    w.push_str("<div class='slider-group'><div class='slider-label'><span>Red</span><span id='rVal'>255</span></div><input type='range' class='r-slider' id='rSlider' min='0' max='255' value='255' oninput='updateColor()'></div>");
    w.push_str("<div class='slider-group'><div class='slider-label'><span>Green</span><span id='gVal'>102</span></div><input type='range' class='g-slider' id='gSlider' min='0' max='255' value='102' oninput='updateColor()'></div>");
    w.push_str("<div class='slider-group'><div class='slider-label'><span>Blue</span><span id='bVal'>0</span></div><input type='range' class='b-slider' id='bSlider' min='0' max='255' value='0' oninput='updateColor()'></div>");
    w.push_str("</div>");
    w.push_str("<div class='btn-row'><button class='save-btn' onclick='saveColor()'>Save Favorite</button><button class='copy-btn' onclick='copyHex()'>Copy Hex</button><button class='clear-btn' onclick='clearFavs()'>Clear All</button></div>");
    w.push_str("</div>");

    // Render saved favorites
    w.push_str("<div class='favorites'><h3>Saved Colors</h3><div class='fav-grid' id='favGrid'>");
    if favs.len() > 0 {
        let bytes = favs.as_bytes();
        let mut p = 0;
        while p <= bytes.len() {
            let start = p;
            while p < bytes.len() && bytes[p] != b',' { p += 1; }
            let color = unsafe { core::str::from_utf8_unchecked(&bytes[start..p]) };
            if p < bytes.len() { p += 1; }
            if color.len() > 0 {
                w.push_str("<div class='fav-swatch' style='background:");
                w.push_str(color);
                w.push_str("' onclick=\"loadColor('");
                w.push_str(color);
                w.push_str("')\" title='");
                w.push_str(color);
                w.push_str("'></div>");
            }
        }
    }
    w.push_str("</div></div></div>");

    w.push_str("<script>");
    w.push_str("const BASE=location.pathname;");
    w.push_str("function toHex(n){const h='0123456789ABCDEF';return h[n>>4]+h[n&15];}");
    w.push_str("function updateColor(){const r=+document.getElementById('rSlider').value;const g=+document.getElementById('gSlider').value;const b=+document.getElementById('bSlider').value;");
    w.push_str("document.getElementById('rVal').textContent=r;document.getElementById('gVal').textContent=g;document.getElementById('bVal').textContent=b;");
    w.push_str("const hex='#'+toHex(r)+toHex(g)+toHex(b);document.getElementById('preview').style.background=hex;document.getElementById('hexDisplay').textContent=hex;");
    w.push_str("document.getElementById('rgbDisplay').textContent='rgb('+r+', '+g+', '+b+')';document.getElementById('nativePicker').value=hex.toLowerCase();}");
    w.push_str("function fromNative(hex){const r=parseInt(hex.substr(1,2),16);const g=parseInt(hex.substr(3,2),16);const b=parseInt(hex.substr(5,2),16);");
    w.push_str("document.getElementById('rSlider').value=r;document.getElementById('gSlider').value=g;document.getElementById('bSlider').value=b;updateColor();}");
    w.push_str("function loadColor(hex){fromNative(hex);}");
    w.push_str("async function saveColor(){const hex=document.getElementById('hexDisplay').textContent;await fetch(BASE,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'save',color:hex})});location.reload();}");
    w.push_str("function copyHex(){const hex=document.getElementById('hexDisplay').textContent;navigator.clipboard.writeText(hex);document.getElementById('hexDisplay').style.color='#22c55e';setTimeout(()=>document.getElementById('hexDisplay').style.color='#fff',1000);}");
    w.push_str("async function clearFavs(){await fetch(BASE,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'clear'})});location.reload();}");
    w.push_str("</script></body></html>");

    respond(200, w.as_str(), "text/html");
}
