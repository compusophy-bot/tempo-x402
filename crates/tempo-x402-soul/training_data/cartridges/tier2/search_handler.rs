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

/// Extract query parameter value from a path like /search?q=term&page=1
fn extract_query_param<'a>(path: &'a [u8], param: &[u8]) -> Option<&'a str> {
    // Find '?'
    let mut qmark = 0;
    while qmark < path.len() && path[qmark] != b'?' {
        qmark += 1;
    }
    if qmark >= path.len() {
        return None;
    }
    let query = &path[qmark + 1..];

    // Search for param= in query string
    let mut i = 0;
    while i + param.len() + 1 <= query.len() {
        // Check if at start of query or after &
        let at_start = i == 0 || query[i - 1] == b'&';
        if at_start && &query[i..i + param.len()] == param && i + param.len() < query.len() && query[i + param.len()] == b'=' {
            let val_start = i + param.len() + 1;
            let mut val_end = val_start;
            while val_end < query.len() && query[val_end] != b'&' {
                val_end += 1;
            }
            return core::str::from_utf8(&query[val_start..val_end]).ok();
        }
        i += 1;
    }
    None
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

fn copy_to_scratch(offset: usize, src: &[u8]) -> usize {
    unsafe {
        let mut i = 0;
        while i < src.len() && offset + i < SCRATCH.len() {
            SCRATCH[offset + i] = src[i];
            i += 1;
        }
        offset + i
    }
}

fn contains_ci(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    let mut i = 0;
    while i + needle.len() <= haystack.len() {
        let mut matched = true;
        let mut j = 0;
        while j < needle.len() {
            let a = if haystack[i + j] >= b'A' && haystack[i + j] <= b'Z' {
                haystack[i + j] + 32
            } else {
                haystack[i + j]
            };
            let b = if needle[j] >= b'A' && needle[j] <= b'Z' {
                needle[j] + 32
            } else {
                needle[j]
            };
            if a != b {
                matched = false;
                break;
            }
            j += 1;
        }
        if matched {
            return true;
        }
        i += 1;
    }
    false
}

// Simulated search database
struct SearchEntry {
    title: &'static str,
    keywords: &'static str,
}

const ENTRIES: &[SearchEntry] = &[
    SearchEntry { title: "Getting Started with WASM", keywords: "wasm webassembly rust tutorial" },
    SearchEntry { title: "Tempo Blockchain Guide", keywords: "tempo blockchain chain crypto" },
    SearchEntry { title: "Cartridge Development", keywords: "cartridge wasm development rust code" },
    SearchEntry { title: "Payment Integration", keywords: "payment x402 http money" },
    SearchEntry { title: "KV Storage Tutorial", keywords: "kv storage database persist data" },
    SearchEntry { title: "Deploy Your First App", keywords: "deploy app application hosting" },
    SearchEntry { title: "Rust No-Std Patterns", keywords: "rust no_std embedded wasm patterns" },
    SearchEntry { title: "API Design Best Practices", keywords: "api rest json design patterns" },
];

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };
    let path = find_json_str(request, b"path").unwrap_or("/search");
    let path_bytes = path.as_bytes();

    host_log(0, "search_handler: processing search query");

    let query = extract_query_param(path_bytes, b"q");

    match query {
        Some(term) if !term.is_empty() => {
            let term_bytes = term.as_bytes();

            // Build HTML search results page
            let mut pos = 0;
            pos = copy_to_scratch(pos, b"<!DOCTYPE html>\n<html>\n<head><title>Search: ");
            pos = copy_to_scratch(pos, term_bytes);
            pos = copy_to_scratch(pos, b"</title></head>\n<body>\n<h1>Search Results</h1>\n<p>Query: <strong>");
            pos = copy_to_scratch(pos, term_bytes);
            pos = copy_to_scratch(pos, b"</strong></p>\n<ul>\n");

            let mut result_count = 0u8;
            let mut i = 0;
            while i < ENTRIES.len() {
                if contains_ci(ENTRIES[i].keywords.as_bytes(), term_bytes)
                    || contains_ci(ENTRIES[i].title.as_bytes(), term_bytes)
                {
                    pos = copy_to_scratch(pos, b"<li><a href=\"#\">");
                    pos = copy_to_scratch(pos, ENTRIES[i].title.as_bytes());
                    pos = copy_to_scratch(pos, b"</a></li>\n");
                    result_count += 1;
                }
                i += 1;
            }

            if result_count == 0 {
                pos = copy_to_scratch(pos, b"<li>No results found</li>\n");
            }

            pos = copy_to_scratch(pos, b"</ul>\n</body>\n</html>");

            let result = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
            respond(200, result, "text/html");
        }
        _ => {
            respond(400, "<!DOCTYPE html>\n<html><body><h1>Search</h1><p>Missing query parameter. Use ?q=term</p></body></html>", "text/html");
        }
    }
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
