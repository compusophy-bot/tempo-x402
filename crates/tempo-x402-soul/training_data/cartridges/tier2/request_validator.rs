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

/// Check if a key exists anywhere in the JSON body (not just as a string value)
fn json_has_key(json: &[u8], key: &[u8]) -> bool {
    let mut i = 0;
    while i + key.len() + 2 < json.len() {
        if json[i] == b'"' {
            let start = i + 1;
            if start + key.len() < json.len()
                && &json[start..start + key.len()] == key
                && json[start + key.len()] == b'"'
            {
                // Check that what follows is a colon (i.e., this is a key, not a value)
                let mut j = start + key.len() + 1;
                while j < json.len() && json[j] == b' ' {
                    j += 1;
                }
                if j < json.len() && json[j] == b':' {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
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

// Required fields for a valid transaction request
const REQUIRED_FIELDS: &[&str] = &["recipient", "amount", "currency"];

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };

    host_log(0, "request_validator: validating POST body");

    // Check method is POST
    let method = find_json_str(request, b"method").unwrap_or("GET");
    if !contains_substr(method.as_bytes(), b"POST") {
        respond(405, r#"{"error":"method_not_allowed","message":"Only POST is accepted","allowed":["POST"]}"#, "application/json");
        return;
    }

    // Extract the body field from the request envelope
    let body_tag = b"\"body\"";
    let mut body_start = 0;
    let mut body_end = request.len();
    let mut found_body = false;
    let mut i = 0;
    while i + body_tag.len() < request.len() {
        if &request[i..i + body_tag.len()] == &body_tag[..] {
            let mut j = i + body_tag.len();
            while j < request.len() && (request[j] == b':' || request[j] == b' ') {
                j += 1;
            }
            if j < request.len() && request[j] == b'"' {
                body_start = j + 1;
                let mut k = body_start;
                while k < request.len() && request[k] != b'"' {
                    k += 1;
                }
                body_end = k;
                found_body = true;
                break;
            } else if j < request.len() && request[j] == b'{' {
                body_start = j;
                // Find matching brace
                let mut depth = 0;
                let mut k = j;
                while k < request.len() {
                    if request[k] == b'{' { depth += 1; }
                    if request[k] == b'}' {
                        depth -= 1;
                        if depth == 0 {
                            body_end = k + 1;
                            found_body = true;
                            break;
                        }
                    }
                    k += 1;
                }
                break;
            }
        }
        i += 1;
    }

    if !found_body {
        respond(400, r#"{"error":"missing_body","message":"Request body is required","required_fields":["recipient","amount","currency"]}"#, "application/json");
        return;
    }

    let body_slice = &request[body_start..body_end];

    // Check each required field
    let buf = unsafe { &mut SCRATCH };
    let mut missing_count = 0;
    let mut missing_fields: [&str; 3] = [""; 3];

    let mut f = 0;
    while f < REQUIRED_FIELDS.len() {
        if !json_has_key(body_slice, REQUIRED_FIELDS[f].as_bytes()) {
            if missing_count < 3 {
                missing_fields[missing_count] = REQUIRED_FIELDS[f];
                missing_count += 1;
            }
        }
        f += 1;
    }

    if missing_count > 0 {
        host_log(1, "request_validator: missing required fields");

        let mut pos = 0;
        pos = copy_to(buf, pos, b"{\"error\":\"validation_failed\",\"message\":\"Missing required fields\",\"missing_fields\":[");
        let mut m = 0;
        while m < missing_count {
            if m > 0 {
                pos = copy_to(buf, pos, b",");
            }
            pos = copy_to(buf, pos, b"\"");
            pos = copy_to(buf, pos, missing_fields[m].as_bytes());
            pos = copy_to(buf, pos, b"\"");
            m += 1;
        }
        pos = copy_to(buf, pos, b"],\"required_fields\":[\"recipient\",\"amount\",\"currency\"]}");

        let body = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
        respond(400, body, "application/json");
        return;
    }

    // Validate amount is present and non-empty
    let amount = find_json_str(body_slice, b"amount").unwrap_or("");
    if amount.is_empty() {
        respond(400, r#"{"error":"validation_failed","message":"Field 'amount' must not be empty"}"#, "application/json");
        return;
    }

    // Validate currency is known
    let currency = find_json_str(body_slice, b"currency").unwrap_or("");
    let valid_currency = contains_substr(currency.as_bytes(), b"pathUSD")
        || contains_substr(currency.as_bytes(), b"TEMPO")
        || contains_substr(currency.as_bytes(), b"USD");

    if !valid_currency {
        let mut pos = 0;
        pos = copy_to(buf, pos, b"{\"error\":\"invalid_currency\",\"message\":\"Unsupported currency: ");
        let cur_trunc = if currency.len() > 32 { &currency.as_bytes()[..32] } else { currency.as_bytes() };
        pos = copy_to(buf, pos, cur_trunc);
        pos = copy_to(buf, pos, b"\",\"supported\":[\"pathUSD\",\"TEMPO\",\"USD\"]}");
        let body = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
        respond(400, body, "application/json");
        return;
    }

    host_log(0, "request_validator: validation passed");
    respond(200, r#"{"status":"valid","message":"Transaction request accepted","chain_id":42431}"#, "application/json");
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
