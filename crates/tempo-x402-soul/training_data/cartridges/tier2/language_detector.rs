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

struct LangMatch {
    code: &'static str,
    greeting: &'static str,
    language: &'static str,
}

const LANGUAGES: &[LangMatch] = &[
    LangMatch { code: "es", greeting: "Hola, bienvenido!", language: "Spanish" },
    LangMatch { code: "fr", greeting: "Bonjour, bienvenue!", language: "French" },
    LangMatch { code: "de", greeting: "Hallo, willkommen!", language: "German" },
    LangMatch { code: "ja", greeting: "Konnichiwa!", language: "Japanese" },
    LangMatch { code: "zh", greeting: "Ni hao!", language: "Chinese" },
    LangMatch { code: "pt", greeting: "Ola, bem-vindo!", language: "Portuguese" },
    LangMatch { code: "it", greeting: "Ciao, benvenuto!", language: "Italian" },
    LangMatch { code: "ko", greeting: "Annyeonghaseyo!", language: "Korean" },
    LangMatch { code: "ru", greeting: "Privet!", language: "Russian" },
    LangMatch { code: "ar", greeting: "Marhaba!", language: "Arabic" },
    LangMatch { code: "en", greeting: "Hello, welcome!", language: "English" },
];

fn copy_to(buf: &mut [u8], offset: usize, src: &[u8]) -> usize {
    let end = if offset + src.len() > buf.len() { buf.len() } else { offset + src.len() };
    let mut i = offset;
    while i < end {
        buf[i] = src[i - offset];
        i += 1;
    }
    end
}

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };

    host_log(0, "language_detector: parsing Accept-Language header");

    let accept_lang = find_header_value(request, b"Accept-Language")
        .or_else(|| find_header_value(request, b"accept-language"))
        .unwrap_or("en");

    let lang_bytes = accept_lang.as_bytes();

    // Find the best matching language by checking if the Accept-Language starts with any code
    let mut matched = &LANGUAGES[LANGUAGES.len() - 1]; // default: English
    let mut i = 0;
    while i < LANGUAGES.len() - 1 {
        let code = LANGUAGES[i].code.as_bytes();
        if contains_substr(lang_bytes, code) {
            matched = &LANGUAGES[i];
            break;
        }
        i += 1;
    }

    host_log(0, matched.language);

    // Build response JSON into scratch buffer
    let buf = unsafe { &mut SCRATCH };
    let mut pos = 0;
    pos = copy_to(buf, pos, b"{\"detected_language\":\"");
    pos = copy_to(buf, pos, matched.language.as_bytes());
    pos = copy_to(buf, pos, b"\",\"code\":\"");
    pos = copy_to(buf, pos, matched.code.as_bytes());
    pos = copy_to(buf, pos, b"\",\"greeting\":\"");
    pos = copy_to(buf, pos, matched.greeting.as_bytes());
    pos = copy_to(buf, pos, b"\",\"accept_language\":\"");
    pos = copy_to(buf, pos, accept_lang.as_bytes());
    pos = copy_to(buf, pos, b"\"}");

    let body = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
    respond(200, body, "application/json");
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
