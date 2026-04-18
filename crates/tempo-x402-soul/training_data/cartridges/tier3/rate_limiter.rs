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

const MAX_REQUESTS_PER_WINDOW: u64 = 10;

/// Rate limiter uses a sliding window approach stored in KV.
/// Keys: "rl_window" = current window identifier (minute bucket),
///       "rl_count" = requests in current window.
/// Since we don't have a clock, we use payment_info as an entropy source
/// to derive a pseudo-minute bucket. In practice, the window resets
/// when the bucket changes. We use the request sequence number as window.
/// Simplified: just track total requests and a window counter.
/// Every 10 requests, the window resets.
#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };

    host_log(0, "rate_limiter: checking rate limit");

    // Get current window count
    let window_count = match kv_read("rl_count") {
        Some(s) => parse_u64(s),
        None => 0,
    };

    // Get total requests ever (used as window rotation trigger)
    let total = match kv_read("rl_total") {
        Some(s) => parse_u64(s),
        None => 0,
    };
    let new_total = total + 1;

    // Every 60 requests, reset the window (simulates time-based window)
    let current_window = new_total / 60;
    let stored_window = match kv_read("rl_window") {
        Some(s) => parse_u64(s),
        None => 0,
    };

    let effective_count = if current_window != stored_window {
        // New window, reset
        let mut wb = [0u8; 20];
        let wl = write_u64(&mut wb, current_window);
        let ws = unsafe { core::str::from_utf8(&wb[..wl]).unwrap_or("0") };
        kv_write("rl_window", ws);
        kv_write("rl_count", "1");
        1u64
    } else {
        let new_count = window_count + 1;
        let mut nb = [0u8; 20];
        let nl = write_u64(&mut nb, new_count);
        let ns = unsafe { core::str::from_utf8(&nb[..nl]).unwrap_or("0") };
        kv_write("rl_count", ns);
        new_count
    };

    // Update total
    let mut tb = [0u8; 20];
    let tl = write_u64(&mut tb, new_total);
    let ts = unsafe { core::str::from_utf8(&tb[..tl]).unwrap_or("0") };
    kv_write("rl_total", ts);

    let remaining = if effective_count > MAX_REQUESTS_PER_WINDOW {
        0u64
    } else {
        MAX_REQUESTS_PER_WINDOW - effective_count
    };

    if effective_count > MAX_REQUESTS_PER_WINDOW {
        host_log(1, "rate_limiter: rate limit exceeded");
        let mut pos = 0;
        pos = append(pos, b"{\"error\":\"rate limit exceeded\",\"limit\":");
        let mut lb = [0u8; 20];
        let ll = write_u64(&mut lb, MAX_REQUESTS_PER_WINDOW);
        pos = append(pos, &lb[..ll]);
        pos = append(pos, b",\"remaining\":0,\"retry_after\":\"next window\"}");
        unsafe {
            let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
            respond(429, resp, "application/json");
        }
        return;
    }

    // Request allowed
    let mut pos = 0;
    pos = append(pos, b"{\"allowed\":true,\"limit\":");
    let mut lb = [0u8; 20];
    let ll = write_u64(&mut lb, MAX_REQUESTS_PER_WINDOW);
    pos = append(pos, &lb[..ll]);
    pos = append(pos, b",\"remaining\":");
    let mut rb = [0u8; 20];
    let rl = write_u64(&mut rb, remaining);
    pos = append(pos, &rb[..rl]);
    pos = append(pos, b",\"current\":");
    let mut cb = [0u8; 20];
    let cl = write_u64(&mut cb, effective_count);
    pos = append(pos, &cb[..cl]);
    pos = append(pos, b",\"total_requests\":");
    pos = append(pos, &tb[..tl]);
    pos = append(pos, b"}");

    unsafe {
        let resp = core::str::from_utf8(&SCRATCH[..pos]).unwrap_or("{}");
        respond(200, resp, "application/json");
    }
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
