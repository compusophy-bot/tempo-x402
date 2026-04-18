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
    host_log(0, "visit_counter: handling request");

    let count = match kv_read("visit_count") {
        Some(s) => parse_u64(s),
        None => 0,
    };
    let new_count = count + 1;

    let mut num_buf = [0u8; 20];
    let num_len = write_u64(&mut num_buf, new_count);

    unsafe {
        let count_str = core::str::from_utf8(&num_buf[..num_len]).unwrap_or("0");
        kv_write("visit_count", count_str);
    }

    let mut pos = 0;
    pos = append(pos, b"<html><head><title>Visit Counter</title>");
    pos = append(pos, b"<style>body{font-family:sans-serif;text-align:center;margin-top:100px;background:#1a1a2e;color:#eee}");
    pos = append(pos, b".count{font-size:72px;color:#e94560;margin:20px}</style></head>");
    pos = append(pos, b"<body><h1>Page Visits</h1><div class=\"count\">");
    pos = append(pos, &num_buf[..num_len]);
    pos = append(pos, b"</div><p>This page has been visited ");
    pos = append(pos, &num_buf[..num_len]);
    pos = append(pos, b" time(s).</p></body></html>");

    unsafe {
        let body = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("error");
        respond(200, body, "text/html");
    }
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
