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
    host_log(0, "coming_soon: serving");
    respond(200, "<!DOCTYPE html><html><head><meta charset='utf-8'><title>Coming Soon</title><style>*{margin:0;padding:0;box-sizing:border-box}body{background:linear-gradient(135deg,#0a0a1a,#1a0a2e);color:#e0e0e0;font-family:sans-serif;min-height:100vh;display:flex;align-items:center;justify-content:center;text-align:center}h1{font-size:48px;color:#7c4dff;margin-bottom:16px}p{font-size:18px;color:#888;max-width:500px;line-height:1.6}.badge{display:inline-block;margin-top:24px;padding:8px 20px;background:#1a1040;border:1px solid #7c4dff;border-radius:20px;color:#7c4dff;font-size:14px}</style></head><body><div><h1>Coming Soon</h1><p>We are building something amazing. Stay tuned for updates.</p><div class='badge'>Launching Q2 2024</div></div></body></html>", "text/html");
}
