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

fn find_json_str<'a>(json: &'a [u8], key: &[u8]) -> Option<&'a str> {
    let mut i = 0;
    while i + key.len() + 3 < json.len() {
        if json[i] == b'"' {
            let start = i + 1;
            if start + key.len() < json.len()
                && &json[start..start + key.len()] == key
                && json[start + key.len()] == b'"'
            {
                let mut j = start + key.len() + 1;
                while j < json.len() && (json[j] == b':' || json[j] == b' ') { j += 1; }
                if j < json.len() && json[j] == b'"' {
                    let val_start = j + 1;
                    let mut val_end = val_start;
                    while val_end < json.len() && json[val_end] != b'"' { val_end += 1; }
                    return core::str::from_utf8(&json[val_start..val_end]).ok();
                }
            }
        }
        i += 1;
    }
    None
}

fn bytes_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] { return false; }
        i += 1;
    }
    true
}

fn parse_u64(s: &str) -> u64 {
    let bytes = s.as_bytes();
    let mut result: u64 = 0;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] >= b'0' && bytes[i] <= b'9' {
            result = result * 10 + (bytes[i] - b'0') as u64;
        }
        i += 1;
    }
    result
}

fn write_u64(buf: &mut [u8], val: u64) -> usize {
    if val == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 20];
    let mut n = val;
    let mut len = 0;
    while n > 0 {
        tmp[len] = b'0' + (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    let mut i = 0;
    while i < len {
        buf[i] = tmp[len - 1 - i];
        i += 1;
    }
    len
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

fn append(pos: usize, data: &[u8]) -> usize {
    unsafe {
        let mut i = 0;
        while i < data.len() && pos + i < SCRATCH.len() {
            SCRATCH[pos + i] = data[i];
            i += 1;
        }
        pos + i
    }
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };
    let method = find_json_str(request, b"method").unwrap_or("GET");

    host_log(0, "click_counter: handling request");

    if bytes_equal(method.as_bytes(), b"POST") {
        // Increment counter
        let count = match kv_read("clicks") {
            Some(s) => parse_u64(s),
            None => 0,
        };
        let new_count = count + 1;
        let mut num_buf = [0u8; 20];
        let num_len = write_u64(&mut num_buf, new_count);
        let count_str = unsafe { core::str::from_utf8(&num_buf[..num_len]).unwrap_or("0") };
        kv_write("clicks", count_str);

        let mut pos = 0;
        pos = append(pos, b"{\"clicks\":");
        pos = append(pos, &num_buf[..num_len]);
        pos = append(pos, b"}");
        unsafe {
            let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
            respond(200, resp, "application/json");
        }
        return;
    }

    // GET: render interactive HTML page
    let count = match kv_read("clicks") {
        Some(s) => parse_u64(s),
        None => 0,
    };
    let mut num_buf = [0u8; 20];
    let num_len = write_u64(&mut num_buf, count);

    let mut pos = 0;
    pos = append(pos, b"<!DOCTYPE html><html><head><title>Click Counter</title>");
    pos = append(pos, b"<style>");
    pos = append(pos, b"*{margin:0;padding:0;box-sizing:border-box}");
    pos = append(pos, b"body{font-family:sans-serif;display:flex;justify-content:center;align-items:center;min-height:100vh;background:#0d1117;color:#c9d1d9}");
    pos = append(pos, b".container{text-align:center}");
    pos = append(pos, b".count{font-size:96px;font-weight:bold;color:#58a6ff;margin:20px 0;transition:transform 0.1s}");
    pos = append(pos, b".count.bump{transform:scale(1.2)}");
    pos = append(pos, b"button{font-size:24px;padding:16px 48px;background:#238636;color:#fff;border:none;border-radius:8px;cursor:pointer}");
    pos = append(pos, b"button:hover{background:#2ea043}");
    pos = append(pos, b"button:active{transform:scale(0.95)}");
    pos = append(pos, b"</style></head><body>");
    pos = append(pos, b"<div class=\"container\">");
    pos = append(pos, b"<h1>Click Counter</h1>");
    pos = append(pos, b"<div class=\"count\" id=\"count\">");
    pos = append(pos, &num_buf[..num_len]);
    pos = append(pos, b"</div>");
    pos = append(pos, b"<button onclick=\"doClick()\">Click Me!</button>");
    pos = append(pos, b"</div>");
    pos = append(pos, b"<script>");
    pos = append(pos, b"function doClick(){");
    pos = append(pos, b"fetch(window.location.pathname,{method:'POST'})");
    pos = append(pos, b".then(r=>r.json())");
    pos = append(pos, b".then(d=>{const el=document.getElementById('count');el.textContent=d.clicks;el.classList.add('bump');setTimeout(()=>el.classList.remove('bump'),100)})");
    pos = append(pos, b".catch(e=>console.error(e));}");
    pos = append(pos, b"</script>");
    pos = append(pos, b"</body></html>");

    unsafe {
        let body = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("error");
        respond(200, body, "text/html");
    }
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
