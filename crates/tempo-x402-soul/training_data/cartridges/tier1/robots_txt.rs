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

const ROBOTS: &str = "# robots.txt — Tempo x402 Cartridge\n\
# https://www.robotstxt.org/\n\
\n\
User-agent: *\n\
Allow: /\n\
Allow: /c/\n\
Allow: /cartridges\n\
Disallow: /api/internal/\n\
Disallow: /soul/\n\
Disallow: /metrics\n\
Disallow: /admin/\n\
\n\
# Crawl-delay for polite bots\n\
Crawl-delay: 10\n\
\n\
# Sitemaps\n\
Sitemap: https://borg-0-production.up.railway.app/sitemap.xml\n\
\n\
# AI training opt-out\n\
User-agent: GPTBot\n\
Disallow: /\n\
\n\
User-agent: CCBot\n\
Disallow: /\n\
\n\
User-agent: Google-Extended\n\
Disallow: /\n";

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(0, "robots_txt: serving robots.txt");
    respond(200, ROBOTS, "text/plain");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
