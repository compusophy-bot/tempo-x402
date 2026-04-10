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
                while j < json.len() && (json[j] == b':' || json[j] == b' ') {
                    j += 1;
                }
                if j < json.len() && json[j] == b'"' {
                    let val_start = j + 1;
                    let mut val_end = val_start;
                    while val_end < json.len() && json[val_end] != b'"' {
                        val_end += 1;
                    }
                    return core::str::from_utf8(&json[val_start..val_end]).ok();
                }
            }
        }
        i += 1;
    }
    None
}

fn find_header_value<'a>(json: &'a [u8], header_key: &[u8]) -> Option<&'a str> {
    let headers_tag = b"\"headers\"";
    let mut i = 0;
    while i + headers_tag.len() < json.len() {
        if &json[i..i + headers_tag.len()] == &headers_tag[..] {
            let region_start = i + headers_tag.len();
            let region = &json[region_start..];
            return find_json_str(region, header_key);
        }
        i += 1;
    }
    None
}

fn contains_substr(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    let mut i = 0;
    while i + needle.len() <= haystack.len() {
        if &haystack[i..i + needle.len()] == needle {
            return true;
        }
        i += 1;
    }
    false
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

fn copy_to(buf: &mut [u8], offset: usize, src: &[u8]) -> usize {
    let end = if offset + src.len() > buf.len() { buf.len() } else { offset + src.len() };
    let mut i = offset;
    while i < end {
        buf[i] = src[i - offset];
        i += 1;
    }
    end
}

/// Decode a single %XX hex escape
fn hex_val(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

/// URL-decode a byte slice into a buffer, returning the number of bytes written
fn url_decode(src: &[u8], dst: &mut [u8]) -> usize {
    let mut si = 0;
    let mut di = 0;
    while si < src.len() && di < dst.len() {
        if src[si] == b'+' {
            dst[di] = b' ';
            si += 1;
            di += 1;
        } else if src[si] == b'%' && si + 2 < src.len() {
            dst[di] = (hex_val(src[si + 1]) << 4) | hex_val(src[si + 2]);
            si += 3;
            di += 1;
        } else {
            dst[di] = src[si];
            si += 1;
            di += 1;
        }
    }
    di
}

/// Represents a parsed form field
struct FormField {
    key_start: usize,
    key_len: usize,
    val_start: usize,
    val_len: usize,
}

const MAX_FIELDS: usize = 32;

/// Parse URL-encoded form body into fields. Returns count of fields parsed.
/// Decoded keys/values are stored starting at decode_buf offset in SCRATCH.
fn parse_form_fields(body: &[u8], fields: &mut [FormField; MAX_FIELDS], decode_area: &mut [u8]) -> usize {
    let mut count = 0;
    let mut pos = 0; // position in body
    let mut decode_pos = 0; // position in decode_area

    while pos < body.len() && count < MAX_FIELDS {
        // Find key end (= or & or end)
        let key_start_raw = pos;
        while pos < body.len() && body[pos] != b'=' && body[pos] != b'&' {
            pos += 1;
        }
        let key_end_raw = pos;

        if key_start_raw == key_end_raw {
            // Empty key, skip
            if pos < body.len() { pos += 1; }
            continue;
        }

        // Decode key
        let key_decoded_start = decode_pos;
        let key_decoded_len = url_decode(&body[key_start_raw..key_end_raw], &mut decode_area[decode_pos..]);
        decode_pos += key_decoded_len;

        // Skip '='
        let mut val_decoded_start = decode_pos;
        let mut val_decoded_len = 0;
        if pos < body.len() && body[pos] == b'=' {
            pos += 1;
            // Find value end (& or end)
            let val_start_raw = pos;
            while pos < body.len() && body[pos] != b'&' {
                pos += 1;
            }
            let val_end_raw = pos;
            // Decode value
            val_decoded_start = decode_pos;
            val_decoded_len = url_decode(&body[val_start_raw..val_end_raw], &mut decode_area[decode_pos..]);
            decode_pos += val_decoded_len;
        }

        fields[count] = FormField {
            key_start: key_decoded_start,
            key_len: key_decoded_len,
            val_start: val_decoded_start,
            val_len: val_decoded_len,
        };
        count += 1;

        // Skip '&'
        if pos < body.len() && body[pos] == b'&' {
            pos += 1;
        }
    }

    count
}

fn write_u32(buf: &mut [u8], offset: usize, mut val: u32) -> usize {
    if val == 0 { buf[offset] = b'0'; return offset + 1; }
    let mut digits = [0u8; 10];
    let mut n = 0;
    while val > 0 { digits[n] = b'0' + (val % 10) as u8; val /= 10; n += 1; }
    let mut pos = offset;
    let mut i = n;
    while i > 0 { i -= 1; buf[pos] = digits[i]; pos += 1; }
    pos
}

/// Escape a byte slice for JSON string output (handle quotes and backslashes)
fn json_escape_to(buf: &mut [u8], offset: usize, src: &[u8]) -> usize {
    let mut pos = offset;
    let mut i = 0;
    while i < src.len() && pos + 2 < buf.len() {
        match src[i] {
            b'"' => { buf[pos] = b'\\'; buf[pos + 1] = b'"'; pos += 2; }
            b'\\' => { buf[pos] = b'\\'; buf[pos + 1] = b'\\'; pos += 2; }
            b'\n' => { buf[pos] = b'\\'; buf[pos + 1] = b'n'; pos += 2; }
            b'\r' => { buf[pos] = b'\\'; buf[pos + 1] = b'r'; pos += 2; }
            c => { buf[pos] = c; pos += 1; }
        }
        i += 1;
    }
    pos
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };

    host_log(0, "form_parser: parsing URL-encoded form data");

    // Must be POST
    let method = find_json_str(request, b"method").unwrap_or("GET");
    if !contains_substr(method.as_bytes(), b"POST") {
        respond(405, r#"{"error":"method_not_allowed","message":"POST required for form submission","allowed":["POST"]}"#, "application/json");
        return;
    }

    // Check content type
    let ct = find_header_value(request, b"Content-Type")
        .or_else(|| find_header_value(request, b"content-type"))
        .unwrap_or("");
    if !contains_substr(ct.as_bytes(), b"form-urlencoded") && !ct.is_empty() {
        // Allow missing content-type but reject wrong ones
        if !ct.is_empty() && !contains_substr(ct.as_bytes(), b"form") {
            respond(415, r#"{"error":"unsupported_media_type","message":"Expected application/x-www-form-urlencoded"}"#, "application/json");
            return;
        }
    }

    // Extract body as string (the form data)
    let body_str = find_json_str(request, b"body").unwrap_or("");
    if body_str.is_empty() {
        respond(400, r#"{"error":"empty_body","message":"Form body is empty","fields":{},"field_count":0}"#, "application/json");
        return;
    }

    let body_bytes = body_str.as_bytes();

    // Use second half of SCRATCH for decoded data
    let mut fields = [FormField { key_start: 0, key_len: 0, val_start: 0, val_len: 0 }; MAX_FIELDS];

    // We need a separate decode area; use a static buffer
    static mut DECODE_BUF: [u8; 32768] = [0u8; 32768];
    let decode_area = unsafe { &mut DECODE_BUF };

    let field_count = parse_form_fields(body_bytes, &mut fields, decode_area);

    // Build JSON response
    let buf = unsafe { &mut SCRATCH };
    let mut pos = 0;

    pos = copy_to(buf, pos, b"{\"field_count\":");
    pos = write_u32(buf, pos, field_count as u32);
    pos = copy_to(buf, pos, b",\"fields\":{");

    let mut f = 0;
    while f < field_count {
        if f > 0 {
            pos = copy_to(buf, pos, b",");
        }
        let field = &fields[f];
        let key = unsafe { &DECODE_BUF[field.key_start..field.key_start + field.key_len] };
        let val = unsafe { &DECODE_BUF[field.val_start..field.val_start + field.val_len] };

        pos = copy_to(buf, pos, b"\"");
        pos = json_escape_to(buf, pos, key);
        pos = copy_to(buf, pos, b"\":\"");
        pos = json_escape_to(buf, pos, val);
        pos = copy_to(buf, pos, b"\"");

        f += 1;
    }

    pos = copy_to(buf, pos, b"},\"raw_length\":");
    pos = write_u32(buf, pos, body_bytes.len() as u32);
    pos = copy_to(buf, pos, b",\"content_type\":\"application/x-www-form-urlencoded\"}");

    host_log(0, "form_parser: parsing complete");
    let body = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
    respond(200, body, "application/json");
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
