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

static mut BUF: [u8; 32768] = [0u8; 32768];

fn buf_write(pos: usize, s: &str) -> usize {
    let b = s.as_bytes();
    let end = (pos + b.len()).min(unsafe { BUF.len() });
    unsafe { BUF[pos..end].copy_from_slice(&b[..end - pos]); }
    end
}

fn buf_as_str(len: usize) -> &'static str {
    unsafe { core::str::from_utf8_unchecked(&BUF[..len]) }
}

fn parse_u32(s: &str) -> u32 {
    let mut n: u32 = 0;
    for b in s.as_bytes() {
        if *b >= b'0' && *b <= b'9' {
            n = n.wrapping_mul(10).wrapping_add((*b - b'0') as u32);
        }
    }
    n
}

fn u32_to_str(mut n: u32, buf: &mut [u8; 10]) -> &str {
    if n == 0 { buf[0] = b'0'; return unsafe { core::str::from_utf8_unchecked(&buf[..1]) }; }
    let mut i = 10;
    while n > 0 { i -= 1; buf[i] = b'0' + (n % 10) as u8; n /= 10; }
    unsafe { core::str::from_utf8_unchecked(&buf[i..]) }
}

/// Simple deterministic pseudo-random: uses total visit count as seed.
/// Returns 0 (variant A) or 1 (variant B).
fn pick_variant(total: u32) -> u32 {
    // Simple hash: multiply by a prime, take high bit
    let hash = total.wrapping_mul(2654435761);
    hash >> 31
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe {
        let bytes = core::slice::from_raw_parts(request_ptr, request_len as usize);
        core::str::from_utf8(bytes).unwrap_or("{}")
    };

    let method = find_json_str(request, "method").unwrap_or("GET");
    let path = find_json_str(request, "path").unwrap_or("/");

    host_log(1, "ab_test: handling request");

    // POST /convert/{variant} — record a conversion for A or B
    if method == "POST" && path.len() > 9 {
        let pb = path.as_bytes();
        if pb.len() >= 10 && pb[0] == b'/' && pb[1] == b'c' && pb[2] == b'o' && pb[3] == b'n'
            && pb[4] == b'v' && pb[5] == b'e' && pb[6] == b'r' && pb[7] == b't' && pb[8] == b'/'
        {
            let variant = &path[9..];
            if variant == "A" || variant == "a" {
                let cur = parse_u32(kv_read("conv_a").unwrap_or("0")) + 1;
                let mut tmp = [0u8; 10];
                kv_write("conv_a", u32_to_str(cur, &mut tmp));
                let mut p = 0;
                p = buf_write(p, "{\"variant\":\"A\",\"conversions\":");
                let mut tb = [0u8; 10];
                p = buf_write(p, u32_to_str(cur, &mut tb));
                p = buf_write(p, "}");
                respond(200, buf_as_str(p), "application/json");
                return;
            } else if variant == "B" || variant == "b" {
                let cur = parse_u32(kv_read("conv_b").unwrap_or("0")) + 1;
                let mut tmp = [0u8; 10];
                kv_write("conv_b", u32_to_str(cur, &mut tmp));
                let mut p = 0;
                p = buf_write(p, "{\"variant\":\"B\",\"conversions\":");
                let mut tb = [0u8; 10];
                p = buf_write(p, u32_to_str(cur, &mut tb));
                p = buf_write(p, "}");
                respond(200, buf_as_str(p), "application/json");
                return;
            }
            respond(400, "{\"error\":\"variant must be A or B\"}", "application/json");
            return;
        }
    }

    // GET /stats — show A/B test statistics
    if path == "/stats" {
        let views_a = parse_u32(kv_read("views_a").unwrap_or("0"));
        let views_b = parse_u32(kv_read("views_b").unwrap_or("0"));
        let conv_a = parse_u32(kv_read("conv_a").unwrap_or("0"));
        let conv_b = parse_u32(kv_read("conv_b").unwrap_or("0"));

        let mut p = 0;
        p = buf_write(p, "{\"variant_a\":{\"views\":");
        let mut t1 = [0u8; 10];
        p = buf_write(p, u32_to_str(views_a, &mut t1));
        p = buf_write(p, ",\"conversions\":");
        let mut t2 = [0u8; 10];
        p = buf_write(p, u32_to_str(conv_a, &mut t2));
        p = buf_write(p, "},\"variant_b\":{\"views\":");
        let mut t3 = [0u8; 10];
        p = buf_write(p, u32_to_str(views_b, &mut t3));
        p = buf_write(p, ",\"conversions\":");
        let mut t4 = [0u8; 10];
        p = buf_write(p, u32_to_str(conv_b, &mut t4));
        p = buf_write(p, "}}");
        respond(200, buf_as_str(p), "application/json");
        return;
    }

    // GET / — assign a variant and show it
    let total = parse_u32(kv_read("total_visits").unwrap_or("0"));
    let new_total = total + 1;
    let mut tmp = [0u8; 10];
    kv_write("total_visits", u32_to_str(new_total, &mut tmp));

    let variant = pick_variant(new_total);
    if variant == 0 {
        let views = parse_u32(kv_read("views_a").unwrap_or("0")) + 1;
        let mut tmp2 = [0u8; 10];
        kv_write("views_a", u32_to_str(views, &mut tmp2));

        let mut p = 0;
        p = buf_write(p, "<html><head><title>A/B Test</title>");
        p = buf_write(p, "<style>body{font-family:sans-serif;text-align:center;margin-top:80px;background:#1a1a2e;color:#eee}");
        p = buf_write(p, ".variant{font-size:64px;color:#e94560;margin:20px}button{padding:16px 32px;font-size:20px;background:#e94560;color:#fff;border:none;border-radius:8px;cursor:pointer}</style></head>");
        p = buf_write(p, "<body><h1>Variant A</h1><div class=\"variant\">A</div>");
        p = buf_write(p, "<p>You are seeing variant A (red theme)</p>");
        p = buf_write(p, "<button onclick=\"fetch('/convert/A',{method:'POST'}).then(()=>alert('Converted!'))\">Convert</button>");
        p = buf_write(p, "</body></html>");
        respond(200, buf_as_str(p), "text/html");
    } else {
        let views = parse_u32(kv_read("views_b").unwrap_or("0")) + 1;
        let mut tmp2 = [0u8; 10];
        kv_write("views_b", u32_to_str(views, &mut tmp2));

        let mut p = 0;
        p = buf_write(p, "<html><head><title>A/B Test</title>");
        p = buf_write(p, "<style>body{font-family:sans-serif;text-align:center;margin-top:80px;background:#0f0f23;color:#eee}");
        p = buf_write(p, ".variant{font-size:64px;color:#00cc96;margin:20px}button{padding:16px 32px;font-size:20px;background:#00cc96;color:#000;border:none;border-radius:8px;cursor:pointer}</style></head>");
        p = buf_write(p, "<body><h1>Variant B</h1><div class=\"variant\">B</div>");
        p = buf_write(p, "<p>You are seeing variant B (green theme)</p>");
        p = buf_write(p, "<button onclick=\"fetch('/convert/B',{method:'POST'}).then(()=>alert('Converted!'))\">Convert</button>");
        p = buf_write(p, "</body></html>");
        respond(200, buf_as_str(p), "text/html");
    }
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
