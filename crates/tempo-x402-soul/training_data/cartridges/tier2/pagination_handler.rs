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

/// Parse a query parameter value from a path like /items?page=2&limit=10
fn parse_query_param(path: &[u8], param: &[u8]) -> Option<u32> {
    // Find '?'
    let mut qmark = 0;
    while qmark < path.len() && path[qmark] != b'?' {
        qmark += 1;
    }
    if qmark >= path.len() {
        return None;
    }
    let query = &path[qmark + 1..];

    // Search for param= in the query string
    let mut i = 0;
    while i + param.len() + 1 < query.len() {
        // Check we're at start of a param (beginning or after &)
        let at_start = i == 0 || query[i - 1] == b'&';
        if at_start && &query[i..i + param.len()] == param && query[i + param.len()] == b'=' {
            let val_start = i + param.len() + 1;
            let mut val_end = val_start;
            while val_end < query.len() && query[val_end] != b'&' && query[val_end] != b'#' {
                val_end += 1;
            }
            // Parse integer
            let mut num: u32 = 0;
            let mut k = val_start;
            while k < val_end {
                if query[k] >= b'0' && query[k] <= b'9' {
                    num = num * 10 + (query[k] - b'0') as u32;
                }
                k += 1;
            }
            return Some(num);
        }
        i += 1;
    }
    None
}

fn write_u32(buf: &mut [u8], offset: usize, mut val: u32) -> usize {
    if val == 0 {
        buf[offset] = b'0';
        return offset + 1;
    }
    let mut digits = [0u8; 10];
    let mut count = 0;
    while val > 0 {
        digits[count] = b'0' + (val % 10) as u8;
        val /= 10;
        count += 1;
    }
    let mut pos = offset;
    let mut i = count;
    while i > 0 {
        i -= 1;
        buf[pos] = digits[i];
        pos += 1;
    }
    pos
}

// Simulated dataset: 47 items total
const TOTAL_ITEMS: u32 = 47;
const MAX_LIMIT: u32 = 50;

struct Item {
    id: u32,
    name: &'static str,
}

const SAMPLE_ITEMS: &[Item] = &[
    Item { id: 1, name: "alpha" },
    Item { id: 2, name: "beta" },
    Item { id: 3, name: "gamma" },
    Item { id: 4, name: "delta" },
    Item { id: 5, name: "epsilon" },
    Item { id: 6, name: "zeta" },
    Item { id: 7, name: "eta" },
    Item { id: 8, name: "theta" },
    Item { id: 9, name: "iota" },
    Item { id: 10, name: "kappa" },
];

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    let request = unsafe { core::slice::from_raw_parts(request_ptr, request_len as usize) };

    host_log(0, "pagination_handler: parsing page and limit params");

    let path = find_json_str(request, b"path").unwrap_or("/items?page=1&limit=10");
    let path_bytes = path.as_bytes();

    let page = parse_query_param(path_bytes, b"page").unwrap_or(1);
    let mut limit = parse_query_param(path_bytes, b"limit").unwrap_or(10);

    // Clamp values
    let page = if page == 0 { 1 } else { page };
    if limit == 0 { limit = 10; }
    if limit > MAX_LIMIT { limit = MAX_LIMIT; }

    let offset = (page - 1) * limit;
    let total_pages = (TOTAL_ITEMS + limit - 1) / limit;

    if offset >= TOTAL_ITEMS {
        respond(200, r#"{"data":[],"pagination":{"page":0,"limit":0,"total":47,"total_pages":0,"has_next":false,"has_prev":true}}"#, "application/json");
        return;
    }

    let remaining = TOTAL_ITEMS - offset;
    let count = if remaining < limit { remaining } else { limit };
    let has_next = offset + count < TOTAL_ITEMS;
    let has_prev = page > 1;

    let buf = unsafe { &mut SCRATCH };
    let mut pos = 0;

    // Build response
    pos = copy_to(buf, pos, b"{\"data\":[");

    let mut item_idx = 0;
    while item_idx < count {
        if item_idx > 0 {
            pos = copy_to(buf, pos, b",");
        }
        // Use sample items cyclically
        let sample = &SAMPLE_ITEMS[(item_idx as usize) % SAMPLE_ITEMS.len()];
        let actual_id = offset + item_idx + 1;

        pos = copy_to(buf, pos, b"{\"id\":");
        pos = write_u32(buf, pos, actual_id);
        pos = copy_to(buf, pos, b",\"name\":\"");
        pos = copy_to(buf, pos, sample.name.as_bytes());
        pos = copy_to(buf, pos, b"-");
        pos = write_u32(buf, pos, actual_id);
        pos = copy_to(buf, pos, b"\"}");
        item_idx += 1;
    }

    pos = copy_to(buf, pos, b"],\"pagination\":{\"page\":");
    pos = write_u32(buf, pos, page);
    pos = copy_to(buf, pos, b",\"limit\":");
    pos = write_u32(buf, pos, limit);
    pos = copy_to(buf, pos, b",\"total\":");
    pos = write_u32(buf, pos, TOTAL_ITEMS);
    pos = copy_to(buf, pos, b",\"total_pages\":");
    pos = write_u32(buf, pos, total_pages);
    pos = copy_to(buf, pos, b",\"has_next\":");
    pos = copy_to(buf, pos, if has_next { b"true" } else { b"false" });
    pos = copy_to(buf, pos, b",\"has_prev\":");
    pos = copy_to(buf, pos, if has_prev { b"true" } else { b"false" });

    if has_next {
        pos = copy_to(buf, pos, b",\"next\":\"/items?page=");
        pos = write_u32(buf, pos, page + 1);
        pos = copy_to(buf, pos, b"&limit=");
        pos = write_u32(buf, pos, limit);
        pos = copy_to(buf, pos, b"\"");
    }
    if has_prev {
        pos = copy_to(buf, pos, b",\"prev\":\"/items?page=");
        pos = write_u32(buf, pos, page - 1);
        pos = copy_to(buf, pos, b"&limit=");
        pos = write_u32(buf, pos, limit);
        pos = copy_to(buf, pos, b"\"");
    }

    pos = copy_to(buf, pos, b"}}");

    host_log(0, "pagination_handler: response built");
    let body = unsafe { core::str::from_utf8_unchecked(&SCRATCH[..pos]) };
    respond(200, body, "application/json");
}

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
