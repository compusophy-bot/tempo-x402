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

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(1, "redirect-page invoked");
    let body = r##"<!DOCTYPE html><html><head><meta charset="UTF-8">
<meta http-equiv="refresh" content="3;url=/">
<title>Redirecting...</title>
<style>*{margin:0;padding:0;box-sizing:border-box}
body{background:##0d1117;color:##c9d1d9;font-family:monospace;display:flex;justify-content:center;align-items:center;min-height:100vh}
.card{text-align:center;padding:40px;border:1px solid ##30363d;border-radius:12px;background:##161b22}
h2{color:##58a6ff;margin-bottom:12px}p{color:##8b949e}
.spinner{width:40px;height:40px;border:3px solid ##30363d;border-top-color:##58a6ff;border-radius:50%;animation:spin 1s linear infinite;margin:20px auto}
@keyframes spin{to{transform:rotate(360deg)}}
</style></head><body><div class="card"><div class="spinner"></div><h2>Redirecting</h2><p>You will be redirected in 3 seconds...</p></div></body></html>"##;
    respond(200, body, "text/html");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];
#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 { unsafe { SCRATCH.as_mut_ptr() } }
