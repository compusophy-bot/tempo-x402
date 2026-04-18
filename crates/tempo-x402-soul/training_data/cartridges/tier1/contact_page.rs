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
    host_log(0, "contact_page: serving");
    respond(200, "<!DOCTYPE html><html><head><meta charset='utf-8'><title>Contact</title><style>*{margin:0;padding:0;box-sizing:border-box}body{background:#1a1a2e;color:#eee;font-family:sans-serif;display:flex;justify-content:center;padding:40px 20px}.c{max-width:500px;width:100%}h1{color:#e94560;margin-bottom:20px}label{display:block;font-size:13px;color:#888;margin:12px 0 4px}input,textarea{width:100%;padding:12px;background:#0f3460;border:1px solid #16213e;color:#eee;border-radius:6px;font-size:14px}textarea{height:120px;resize:vertical}button{margin-top:16px;padding:12px;background:#e94560;color:#fff;border:none;border-radius:6px;cursor:pointer;font-size:14px;width:100%}.info{margin-top:24px;padding:16px;background:#16213e;border-radius:8px;font-size:14px;line-height:1.6;color:#aaa}</style></head><body><div class='c'><h1>Contact Us</h1><form onsubmit='return false'><label>Name</label><input placeholder='Your name'><label>Email</label><input type='email' placeholder='you@example.com'><label>Message</label><textarea placeholder='How can we help?'></textarea><button>Send</button></form><div class='info'>Email: hello@example.com<br>Phone: +1 555-123-4567</div></div></body></html>", "text/html");
}
