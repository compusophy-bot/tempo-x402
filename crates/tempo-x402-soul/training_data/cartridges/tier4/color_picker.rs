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
        let action = find_json_str(body, "action").unwrap_or("save");
        let color = find_json_str(body, "color").unwrap_or("");
        let idx_str = find_json_str(body, "index").unwrap_or("0");

        if action == "save" && color.len() > 0 {
            let existing = kv_read("fav_colors").unwrap_or("");
            let mut bp = 0usize;
            bp = buf_write(bp, existing);
            bp = buf_write(bp, color);
            bp = buf_write(bp, "\n");
            kv_write("fav_colors", buf_as_str(bp));
        } else if action == "delete" {
            let target = parse_usize(idx_str);
            let existing = kv_read("fav_colors").unwrap_or("");
            let eb = existing.as_bytes();
            static mut DEL: [u8; 4096] = [0u8; 4096];
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
            kv_write("fav_colors", unsafe { core::str::from_utf8_unchecked(&DEL[..np]) });
        }
    }

    let favorites = kv_read("fav_colors").unwrap_or("");

    let mut p = 0usize;
    p = buf_write(p, r##"<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Color Picker</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:#18181b;color:#e4e4e7;font-family:'Segoe UI',sans-serif;min-height:100vh;padding:20px;display:flex;flex-direction:column;align-items:center}
h1{margin:20px 0;font-size:2em}
.container{width:100%;max-width:550px}
.picker-card{background:#27272a;border-radius:20px;padding:30px;margin-bottom:20px;text-align:center;border:1px solid #3f3f46}
.preview{width:200px;height:200px;border-radius:50%;margin:0 auto 20px;border:4px solid #3f3f46;transition:background 0.3s}
.color-input{width:80px;height:50px;border:none;border-radius:10px;cursor:pointer;background:transparent}
.hex-display{font-size:2em;font-family:monospace;font-weight:bold;margin:15px 0;letter-spacing:2px}
.rgb-display{color:#a1a1aa;margin-bottom:15px}
.sliders{margin:20px 0}
.slider-row{display:flex;align-items:center;gap:10px;margin-bottom:10px}
.slider-row label{width:20px;font-weight:bold;font-size:1.1em}
.slider-row label.r{color:#ef4444}
.slider-row label.g{color:#22c55e}
.slider-row label.b{color:#3b82f6}
.slider-row input[type=range]{flex:1;height:8px;-webkit-appearance:none;background:#3f3f46;border-radius:4px;outline:none}
.slider-row input[type=range]::-webkit-slider-thumb{-webkit-appearance:none;width:20px;height:20px;border-radius:50%;cursor:pointer}
.slider-row .val{width:40px;text-align:right;font-family:monospace}
.btn-row{display:flex;gap:10px;justify-content:center}
.btn{padding:12px 24px;border:none;border-radius:10px;cursor:pointer;font-weight:bold;font-size:1em}
.btn-save{background:#8b5cf6;color:#fff}
.btn-save:hover{background:#7c3aed}
.btn-copy{background:#3f3f46;color:#e4e4e7}
.btn-copy:hover{background:#52525b}
.favorites{background:#27272a;border-radius:20px;padding:20px;border:1px solid #3f3f46}
.favorites h2{margin-bottom:15px;color:#a78bfa}
.fav-grid{display:flex;flex-wrap:wrap;gap:10px}
.fav-item{position:relative;width:60px;height:60px;border-radius:12px;cursor:pointer;border:2px solid #3f3f46;transition:transform 0.2s}
.fav-item:hover{transform:scale(1.1)}
.fav-item .fav-hex{position:absolute;bottom:-20px;left:50%;transform:translateX(-50%);font-size:0.65em;color:#a1a1aa;white-space:nowrap;font-family:monospace}
.fav-item .fav-del{position:absolute;top:-6px;right:-6px;width:18px;height:18px;border-radius:50%;background:#ef4444;color:#fff;border:none;font-size:0.7em;cursor:pointer;display:none;line-height:18px;text-align:center}
.fav-item:hover .fav-del{display:block}
.empty-fav{color:#71717a;padding:20px;text-align:center;width:100%}
.presets{display:flex;flex-wrap:wrap;gap:8px;margin:15px 0;justify-content:center}
.preset{width:36px;height:36px;border-radius:8px;cursor:pointer;border:2px solid transparent;transition:border-color 0.2s}
.preset:hover{border-color:#fff}
</style></head><body>
<h1>&#127912; Color Picker</h1>
<div class="container">
<div class="picker-card">
<div class="preview" id="preview" style="background:#8b5cf6"></div>
<div class="hex-display" id="hexDisplay">#8B5CF6</div>
<div class="rgb-display" id="rgbDisplay">rgb(139, 92, 246)</div>
<input type="color" class="color-input" id="colorInput" value="#8b5cf6" onchange="fromPicker(this.value)">
<div class="sliders">
<div class="slider-row"><label class="r">R</label><input type="range" min="0" max="255" value="139" id="rSlider" oninput="fromSliders()"><span class="val" id="rVal">139</span></div>
<div class="slider-row"><label class="g">G</label><input type="range" min="0" max="255" value="92" id="gSlider" oninput="fromSliders()"><span class="val" id="gVal">92</span></div>
<div class="slider-row"><label class="b">B</label><input type="range" min="0" max="255" value="246" id="bSlider" oninput="fromSliders()"><span class="val" id="bVal">246</span></div>
</div>
<div class="presets">
<div class="preset" style="background:#ef4444" onclick="fromPicker('#ef4444')"></div>
<div class="preset" style="background:#f97316" onclick="fromPicker('#f97316')"></div>
<div class="preset" style="background:#eab308" onclick="fromPicker('#eab308')"></div>
<div class="preset" style="background:#22c55e" onclick="fromPicker('#22c55e')"></div>
<div class="preset" style="background:#06b6d4" onclick="fromPicker('#06b6d4')"></div>
<div class="preset" style="background:#3b82f6" onclick="fromPicker('#3b82f6')"></div>
<div class="preset" style="background:#8b5cf6" onclick="fromPicker('#8b5cf6')"></div>
<div class="preset" style="background:#ec4899" onclick="fromPicker('#ec4899')"></div>
<div class="preset" style="background:#ffffff" onclick="fromPicker('#ffffff')"></div>
<div class="preset" style="background:#000000;border-color:#3f3f46" onclick="fromPicker('#000000')"></div>
</div>
<div class="btn-row">
<button class="btn btn-save" onclick="saveColor()">Save to Favorites</button>
<button class="btn btn-copy" onclick="copyHex()">Copy Hex</button>
</div>
</div>
<div class="favorites"><h2>&#11088; Saved Colors</h2><div class="fav-grid" id="favGrid">"##);

    // Render favorites
    let fb = favorites.as_bytes();
    let mut fpos = 0;
    let mut fidx = 0usize;
    let mut has_favs = false;

    while fpos < fb.len() {
        let mut fend = fpos;
        while fend < fb.len() && fb[fend] != b'\n' { fend += 1; }
        if fend > fpos {
            has_favs = true;
            let color = unsafe { core::str::from_utf8_unchecked(&fb[fpos..fend]) };
            p = buf_write(p, r##"<div class="fav-item" style="background:"##);
            p = buf_write(p, color);
            p = buf_write(p, r##"" onclick="fromPicker('"##);
            p = buf_write(p, color);
            p = buf_write(p, r##"')"><span class="fav-hex">"##);
            p = buf_write(p, color);
            p = buf_write(p, r##"</span><button class="fav-del" onclick="event.stopPropagation();delFav("##);
            p = write_usize(p, fidx);
            p = buf_write(p, r##")">x</button></div>"##);
            fidx += 1;
        }
        fpos = fend + 1;
    }

    if !has_favs {
        p = buf_write(p, r##"<div class="empty-fav">No saved colors yet</div>"##);
    }

    p = buf_write(p, r##"</div></div></div>
<script>
var curHex='#8b5cf6';
function hexToRgb(h){var r=parseInt(h.substr(1,2),16),g=parseInt(h.substr(3,2),16),b=parseInt(h.substr(5,2),16);return{r:r,g:g,b:b}}
function rgbToHex(r,g,b){return'#'+((1<<24)+(r<<16)+(g<<8)+b).toString(16).slice(1)}
function updateUI(hex){curHex=hex;var c=hexToRgb(hex);document.getElementById('preview').style.background=hex;document.getElementById('hexDisplay').textContent=hex.toUpperCase();document.getElementById('rgbDisplay').textContent='rgb('+c.r+', '+c.g+', '+c.b+')';document.getElementById('rSlider').value=c.r;document.getElementById('gSlider').value=c.g;document.getElementById('bSlider').value=c.b;document.getElementById('rVal').textContent=c.r;document.getElementById('gVal').textContent=c.g;document.getElementById('bVal').textContent=c.b;document.getElementById('colorInput').value=hex}
function fromPicker(v){updateUI(v)}
function fromSliders(){var r=+document.getElementById('rSlider').value;var g=+document.getElementById('gSlider').value;var b=+document.getElementById('bSlider').value;updateUI(rgbToHex(r,g,b))}
function saveColor(){fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'save',color:curHex})}).then(function(){location.reload()})}
function delFav(i){fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({action:'delete',index:String(i)})}).then(function(){location.reload()})}
function copyHex(){navigator.clipboard.writeText(curHex).then(function(){document.getElementById('hexDisplay').textContent='Copied!';setTimeout(function(){document.getElementById('hexDisplay').textContent=curHex.toUpperCase()},1000)})}
</script></body></html>"##);

    respond(200, buf_as_str(p), "text/html");
}
