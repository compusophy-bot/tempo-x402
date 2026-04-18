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
        if n == 0 {
            self.push_str("0");
            return;
        }
        let mut digits = [0u8; 10];
        let mut i = 0;
        while n > 0 {
            digits[i] = b'0' + (n % 10) as u8;
            n /= 10;
            i += 1;
        }
        while i > 0 {
            i -= 1;
            unsafe {
                if self.pos < BUF.len() {
                    BUF[self.pos] = digits[i];
                    self.pos += 1;
                }
            }
        }
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
        // POST: save last expression to history
        if let Some(expr) = find_json_str(body, "expr") {
            if let Some(result) = find_json_str(body, "result") {
                let existing = kv_read("calc_history").unwrap_or("");
                let mut w = BufWriter::new();
                w.push_str(expr);
                w.push_str(" = ");
                w.push_str(result);
                w.push_str("\n");
                if existing.len() > 0 {
                    // Keep last 10 entries
                    let mut count: u32 = 0;
                    let bytes = existing.as_bytes();
                    let mut p = 0;
                    while p < bytes.len() {
                        if bytes[p] == b'\n' { count += 1; }
                        p += 1;
                    }
                    if count < 10 {
                        w.push_str(existing);
                    } else {
                        // Skip first line
                        let mut skip = 0;
                        while skip < bytes.len() && bytes[skip] != b'\n' { skip += 1; }
                        if skip < bytes.len() { skip += 1; }
                        if skip < bytes.len() {
                            w.push_str(unsafe { core::str::from_utf8_unchecked(&bytes[skip..]) });
                        }
                    }
                }
                kv_write("calc_history", w.as_str());
            }
        }
        respond(200, "{\"ok\":true}", "application/json");
        return;
    }

    let history = kv_read("calc_history").unwrap_or("");
    let mut w = BufWriter::new();
    w.push_str("<!DOCTYPE html><html><head><meta charset='utf-8'><meta name='viewport' content='width=device-width,initial-scale=1'><title>Calculator</title><style>");
    w.push_str("*{margin:0;padding:0;box-sizing:border-box}");
    w.push_str("body{background:#0d1117;color:#e6edf3;font-family:'SF Mono','Courier New',monospace;min-height:100vh;display:flex;justify-content:center;align-items:center;padding:20px}");
    w.push_str(".calc{background:#161b22;border-radius:16px;padding:24px;width:340px;box-shadow:0 8px 32px rgba(0,0,0,0.4)}");
    w.push_str(".display{background:#0d1117;border:1px solid #30363d;border-radius:12px;padding:20px;margin-bottom:16px;text-align:right;min-height:80px;display:flex;flex-direction:column;justify-content:flex-end}");
    w.push_str(".expr{color:#8b949e;font-size:14px;min-height:20px}");
    w.push_str(".result{color:#e6edf3;font-size:32px;font-weight:bold}");
    w.push_str(".buttons{display:grid;grid-template-columns:repeat(4,1fr);gap:8px}");
    w.push_str(".btn{padding:18px;border:none;border-radius:10px;font-size:18px;font-weight:600;cursor:pointer;transition:all 0.15s}");
    w.push_str(".btn-num{background:#21262d;color:#e6edf3}.btn-num:hover{background:#30363d}");
    w.push_str(".btn-op{background:#1f6feb;color:#fff}.btn-op:hover{background:#388bfd}");
    w.push_str(".btn-eq{background:#238636;color:#fff}.btn-eq:hover{background:#2ea043}");
    w.push_str(".btn-clear{background:#da3633;color:#fff}.btn-clear:hover{background:#f85149}");
    w.push_str(".history{margin-top:16px;background:#0d1117;border:1px solid #30363d;border-radius:12px;padding:12px;max-height:150px;overflow-y:auto}");
    w.push_str(".history h3{color:#8b949e;font-size:12px;margin-bottom:8px}");
    w.push_str(".hist-item{color:#7d8590;font-size:13px;padding:3px 0}");
    w.push_str("</style></head><body><div class='calc'>");
    w.push_str("<div class='display'><div class='expr' id='expr'></div><div class='result' id='result'>0</div></div>");
    w.push_str("<div class='buttons'>");
    w.push_str("<button class='btn btn-clear' onclick='clearAll()'>AC</button>");
    w.push_str("<button class='btn btn-num' onclick='backspace()'>&#9003;</button>");
    w.push_str("<button class='btn btn-op' onclick='addOp(\"%\")'>&percnt;</button>");
    w.push_str("<button class='btn btn-op' onclick='addOp(\"/\")'>&divide;</button>");
    w.push_str("<button class='btn btn-num' onclick='addNum(\"7\")'>7</button>");
    w.push_str("<button class='btn btn-num' onclick='addNum(\"8\")'>8</button>");
    w.push_str("<button class='btn btn-num' onclick='addNum(\"9\")'>9</button>");
    w.push_str("<button class='btn btn-op' onclick='addOp(\"*\")'>&times;</button>");
    w.push_str("<button class='btn btn-num' onclick='addNum(\"4\")'>4</button>");
    w.push_str("<button class='btn btn-num' onclick='addNum(\"5\")'>5</button>");
    w.push_str("<button class='btn btn-num' onclick='addNum(\"6\")'>6</button>");
    w.push_str("<button class='btn btn-op' onclick='addOp(\"-\")'>&minus;</button>");
    w.push_str("<button class='btn btn-num' onclick='addNum(\"1\")'>1</button>");
    w.push_str("<button class='btn btn-num' onclick='addNum(\"2\")'>2</button>");
    w.push_str("<button class='btn btn-num' onclick='addNum(\"3\")'>3</button>");
    w.push_str("<button class='btn btn-op' onclick='addOp(\"+\")'>+</button>");
    w.push_str("<button class='btn btn-num' onclick='addNum(\"0\")' style='grid-column:span 2'>0</button>");
    w.push_str("<button class='btn btn-num' onclick='addDot()'>.</button>");
    w.push_str("<button class='btn btn-eq' onclick='calculate()'>=</button>");
    w.push_str("</div>");

    // History section
    w.push_str("<div class='history'><h3>HISTORY</h3>");
    if history.len() > 0 {
        let bytes = history.as_bytes();
        let mut pos = 0;
        while pos < bytes.len() {
            let start = pos;
            while pos < bytes.len() && bytes[pos] != b'\n' { pos += 1; }
            let line = unsafe { core::str::from_utf8_unchecked(&bytes[start..pos]) };
            if pos < bytes.len() { pos += 1; }
            if line.len() > 0 {
                w.push_str("<div class='hist-item'>");
                w.push_str(line);
                w.push_str("</div>");
            }
        }
    } else {
        w.push_str("<div class='hist-item'>No calculations yet</div>");
    }
    w.push_str("</div></div>");

    w.push_str("<script>");
    w.push_str("let expression='';let lastResult='';");
    w.push_str("function addNum(n){expression+=n;document.getElementById('expr').textContent=expression;}");
    w.push_str("function addOp(o){if(expression.length>0){expression+=' '+o+' ';document.getElementById('expr').textContent=expression;}}");
    w.push_str("function addDot(){if(!expression.includes('.')||expression.includes(' ')){expression+='.';document.getElementById('expr').textContent=expression;}}");
    w.push_str("function clearAll(){expression='';document.getElementById('expr').textContent='';document.getElementById('result').textContent='0';}");
    w.push_str("function backspace(){expression=expression.trimEnd().slice(0,-1).trimEnd();document.getElementById('expr').textContent=expression;}");
    w.push_str("async function calculate(){if(!expression)return;try{let r=Function('return '+expression)();document.getElementById('result').textContent=r;await fetch(location.pathname,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({expr:expression,result:String(r)})});lastResult=String(r);expression='';}catch(e){document.getElementById('result').textContent='Error';}}");
    w.push_str("document.addEventListener('keydown',function(e){if(e.key>='0'&&e.key<='9')addNum(e.key);else if('+-*/%'.includes(e.key))addOp(e.key);else if(e.key==='Enter')calculate();else if(e.key==='Escape')clearAll();else if(e.key==='Backspace')backspace();else if(e.key==='.')addDot();});");
    w.push_str("</script></body></html>");

    respond(200, w.as_str(), "text/html");
}
