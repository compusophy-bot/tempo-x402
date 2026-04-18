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

fn respond(status: i32, body: &str, ct: &str) {
    unsafe { response(status, body.as_ptr(), body.len() as i32, ct.as_ptr(), ct.len() as i32); }
}
fn host_log(level: i32, msg: &str) { unsafe { log(level, msg.as_ptr(), msg.len() as i32); } }

fn kv_read(key: &str) -> Option<&'static str> {
    unsafe {
        let r = kv_get(key.as_ptr(), key.len() as i32);
        if r < 0 { return None; }
        let ptr = (r >> 32) as *const u8;
        let len = (r & 0xFFFFFFFF) as usize;
        core::str::from_utf8(core::slice::from_raw_parts(ptr, len)).ok()
    }
}
fn kv_write(key: &str, val: &str) {
    unsafe { kv_set(key.as_ptr(), key.len() as i32, val.as_ptr(), val.len() as i32); }
}

static mut BUF: [u8; 65536] = [0u8; 65536];
static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 { unsafe { SCRATCH.as_mut_ptr() } }

fn find_json_str<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let kb = key.as_bytes(); let jb = json.as_bytes();
    let mut i = 0;
    while i + kb.len() + 3 < jb.len() {
        if jb[i] == b'"' {
            let s = i + 1;
            if s + kb.len() < jb.len() && &jb[s..s + kb.len()] == kb && jb[s + kb.len()] == b'"' {
                let mut j = s + kb.len() + 1;
                while j < jb.len() && (jb[j] == b':' || jb[j] == b' ') { j += 1; }
                if j < jb.len() && jb[j] == b'"' {
                    let vs = j + 1; let mut ve = vs;
                    while ve < jb.len() && jb[ve] != b'"' { ve += 1; }
                    return core::str::from_utf8(&jb[vs..ve]).ok();
                }
            }
        }
        i += 1;
    }
    None
}

fn parse_u32(s: &str) -> u32 {
    let mut n: u32 = 0;
    for &b in s.as_bytes() { if b >= b'0' && b <= b'9' { n = n * 10 + (b - b'0') as u32; } }
    n
}

struct BufWriter { pos: usize }
impl BufWriter {
    fn new() -> Self { Self { pos: 0 } }
    fn push(&mut self, s: &str) {
        let b = s.as_bytes();
        unsafe { let e = (self.pos + b.len()).min(BUF.len()); BUF[self.pos..e].copy_from_slice(&b[..e - self.pos]); self.pos = e; }
    }
    fn push_num(&mut self, mut n: u32) {
        if n == 0 { self.push("0"); return; }
        let mut d = [0u8; 10]; let mut i = 0;
        while n > 0 { d[i] = b'0' + (n % 10) as u8; n /= 10; i += 1; }
        while i > 0 { i -= 1; unsafe { if self.pos < BUF.len() { BUF[self.pos] = d[i]; self.pos += 1; } } }
    }
    fn as_str(&self) -> &str { unsafe { core::str::from_utf8_unchecked(&BUF[..self.pos]) } }
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(request_ptr, request_len as usize)) };
    let method = find_json_str(request, "method").unwrap_or("GET");
    let body = find_json_str(request, "body").unwrap_or("");

    host_log(0, "unit_converter: handling request");

    // POST — save a conversion to history
    if method == "POST" {
        if let Some(entry) = find_json_str(body, "conversion") {
            let existing = kv_read("conv_history").unwrap_or("");
            let mut w = BufWriter::new();
            w.push(entry);
            if !existing.is_empty() { w.push("\n"); w.push(existing); }
            // Keep last 20 lines
            let mut lines = 0;
            let bytes = w.as_str().as_bytes();
            let mut trunc = bytes.len();
            let mut p = 0;
            while p < bytes.len() {
                if bytes[p] == b'\n' { lines += 1; if lines >= 20 { trunc = p; break; } }
                p += 1;
            }
            let trimmed = unsafe { core::str::from_utf8_unchecked(&bytes[..trunc]) };
            kv_write("conv_history", trimmed);
            respond(200, r#"{"ok":true}"#, "application/json");
        } else {
            respond(400, r#"{"error":"missing conversion"}"#, "application/json");
        }
        return;
    }

    // GET — full converter UI (all conversion done client-side)
    let history = kv_read("conv_history").unwrap_or("");
    let mut w = BufWriter::new();
    w.push("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Unit Converter</title><style>");
    w.push("*{margin:0;padding:0;box-sizing:border-box}body{background:#0f0f23;color:#ccc;font-family:'Segoe UI',sans-serif;padding:30px 20px;display:flex;justify-content:center}");
    w.push(".c{max-width:550px;width:100%}h1{text-align:center;color:#00cc7a;margin-bottom:24px}");
    w.push(".tabs{display:flex;gap:4px;margin-bottom:20px;flex-wrap:wrap}.tab{padding:8px 16px;background:#1a1a3e;border:1px solid #333;border-radius:6px;cursor:pointer;color:#888;font-size:13px}.tab.active{background:#0a2a1a;border-color:#00cc7a;color:#00cc7a}");
    w.push(".panel{background:#1a1a2e;padding:20px;border-radius:12px;margin-bottom:16px}");
    w.push(".row{display:flex;gap:10px;align-items:center;margin-bottom:12px}");
    w.push("input[type=number]{flex:1;padding:12px;background:#111;border:1px solid #333;color:#e0e0e0;border-radius:6px;font-size:18px;text-align:center}");
    w.push("select{padding:10px;background:#111;border:1px solid #333;color:#ccc;border-radius:6px;font-size:14px}");
    w.push(".result{text-align:center;font-size:28px;color:#00cc7a;padding:16px;background:#0a2a1a;border-radius:8px;margin-top:10px}");
    w.push(".swap{background:none;border:none;color:#00cc7a;font-size:24px;cursor:pointer;padding:8px}");
    w.push("button.save{margin-top:10px;padding:8px 16px;background:#00cc7a;color:#000;border:none;border-radius:6px;cursor:pointer;font-size:13px}");
    w.push(".history{background:#1a1a2e;padding:16px;border-radius:12px}.history h3{color:#888;font-size:13px;margin-bottom:10px;text-transform:uppercase}");
    w.push(".hist-item{padding:6px 0;border-bottom:1px solid #222;font-size:13px;color:#666}");
    w.push("</style></head><body><div class='c'><h1>Unit Converter</h1>");
    w.push("<div class='tabs'><div class='tab active' onclick='showTab(0)'>Length</div><div class='tab' onclick='showTab(1)'>Weight</div><div class='tab' onclick='showTab(2)'>Temperature</div><div class='tab' onclick='showTab(3)'>Volume</div></div>");
    w.push("<div class='panel'><div class='row'><input type='number' id='val' value='1' oninput='convert()'><button class='swap' onclick='swapUnits()'>&#8646;</button><input type='number' id='res' readonly></div>");
    w.push("<div class='row'><select id='from' onchange='convert()'></select><select id='to' onchange='convert()'></select></div>");
    w.push("<button class='save' onclick='saveConv()'>Save to History</button></div>");

    // History
    w.push("<div class='history'><h3>Recent Conversions</h3>");
    if !history.is_empty() {
        let hb = history.as_bytes();
        let mut p = 0;
        while p < hb.len() {
            let ls = p;
            while p < hb.len() && hb[p] != b'\n' { p += 1; }
            let line = unsafe { core::str::from_utf8_unchecked(&hb[ls..p]) };
            if p < hb.len() { p += 1; }
            if !line.is_empty() {
                w.push("<div class='hist-item'>");
                w.push(line);
                w.push("</div>");
            }
        }
    } else {
        w.push("<div class='hist-item'>No conversions yet</div>");
    }
    w.push("</div></div>");

    w.push("<script>");
    w.push("const units={length:['m','km','mi','ft','in','cm','mm','yd'],weight:['kg','g','lb','oz','mg','ton'],temperature:['C','F','K'],volume:['L','mL','gal','qt','pt','cup','fl_oz']};");
    w.push("const factors={m:1,km:1000,mi:1609.344,ft:0.3048,in:0.0254,cm:0.01,mm:0.001,yd:0.9144,kg:1,g:0.001,lb:0.453592,oz:0.0283495,mg:0.000001,ton:907.185,L:1,mL:0.001,gal:3.78541,qt:0.946353,pt:0.473176,cup:0.236588,fl_oz:0.0295735};");
    w.push("const tabs=['length','weight','temperature','volume'];let curTab=0;");
    w.push("function showTab(i){curTab=i;document.querySelectorAll('.tab').forEach((t,j)=>t.classList.toggle('active',j===i));populateUnits();convert();}");
    w.push("function populateUnits(){const u=units[tabs[curTab]];const f=document.getElementById('from');const t=document.getElementById('to');f.innerHTML='';t.innerHTML='';u.forEach((v,i)=>{f.add(new Option(v,v));t.add(new Option(v,v));});if(u.length>1)t.selectedIndex=1;convert();}");
    w.push("function convert(){const v=parseFloat(document.getElementById('val').value)||0;const f=document.getElementById('from').value;const t=document.getElementById('to').value;let r;if(tabs[curTab]==='temperature'){r=convertTemp(v,f,t);}else{r=v*factors[f]/factors[t];}document.getElementById('res').value=Math.round(r*1e6)/1e6;}");
    w.push("function convertTemp(v,f,t){let c;if(f==='C')c=v;else if(f==='F')c=(v-32)*5/9;else c=v-273.15;if(t==='C')return c;if(t==='F')return c*9/5+32;return c+273.15;}");
    w.push("function swapUnits(){const f=document.getElementById('from');const t=document.getElementById('to');const tmp=f.value;f.value=t.value;t.value=tmp;convert();}");
    w.push("async function saveConv(){const v=document.getElementById('val').value;const f=document.getElementById('from').value;const r=document.getElementById('res').value;const t=document.getElementById('to').value;const s=v+' '+f+' = '+r+' '+t;await fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({conversion:s})});location.reload();}");
    w.push("populateUnits();");
    w.push("</script></body></html>");
    respond(200, w.as_str(), "text/html");
}
