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

fn respond(status: i32, body: &str, ct: &str) { unsafe { response(status, body.as_ptr(), body.len() as i32, ct.as_ptr(), ct.len() as i32); } }
fn host_log(level: i32, msg: &str) { unsafe { log(level, msg.as_ptr(), msg.len() as i32); } }

static mut SCRATCH: [u8; 131072] = [0u8; 131072];
#[no_mangle] pub extern "C" fn x402_alloc(size: i32) -> *mut u8 { unsafe { SCRATCH.as_mut_ptr() } }

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(0, "cookie_policy: serving");
    respond(200, "<!DOCTYPE html><html><head><meta charset='utf-8'><title>Cookie Policy</title><style>*{margin:0;padding:0;box-sizing:border-box}body{background:#111;color:#ccc;font-family:Georgia,serif;padding:40px 20px;display:flex;justify-content:center}div{max-width:700px;line-height:1.8}h1{color:#e0e0e0;margin-bottom:16px}h2{color:#aaa;margin:20px 0 8px;font-size:18px}p{margin-bottom:12px;font-size:15px}</style></head><body><div><h1>Cookie Policy</h1><h2>What Are Cookies</h2><p>Cookies are small text files stored on your device when you visit a website.</p><h2>How We Use Cookies</h2><p>We use essential cookies for authentication and session management. No tracking cookies are used.</p><h2>Your Choices</h2><p>You can disable cookies in your browser settings. Some features may not work without cookies.</p></div></body></html>", "text/html");
}
