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
    host_log(1, "progress-bars invoked");
    let body = r##"<!DOCTYPE html><html><head><meta charset="UTF-8"><title>Progress Bars</title>
<style>*{margin:0;padding:0;box-sizing:border-box}body{background:##0d1117;color:##c9d1d9;font-family:monospace;padding:32px;max-width:600px;margin:0 auto}
h1{color:##e6edf3;margin-bottom:24px}.item{margin-bottom:16px}.label{display:flex;justify-content:space-between;margin-bottom:4px;font-size:0.9rem}
.bar{height:8px;background:##21262d;border-radius:4px;overflow:hidden}
.fill{height:100%;border-radius:4px;transition:width 0.5s}
.green .fill{background:##3fb950}.blue .fill{background:##58a6ff}.purple .fill{background:##bc8cff}.yellow .fill{background:##d29922}.red .fill{background:##f85149}
</style></head><body><h1>Build Progress</h1>
<div class="item green"><div class="label"><span>Compilation</span><span>100%</span></div><div class="bar"><div class="fill" style="width:100%"></div></div></div>
<div class="item blue"><div class="label"><span>Tests</span><span>87%</span></div><div class="bar"><div class="fill" style="width:87%"></div></div></div>
<div class="item purple"><div class="label"><span>Coverage</span><span>72%</span></div><div class="bar"><div class="fill" style="width:72%"></div></div></div>
<div class="item yellow"><div class="label"><span>Lint</span><span>45%</span></div><div class="bar"><div class="fill" style="width:45%"></div></div></div>
<div class="item red"><div class="label"><span>Security Audit</span><span>23%</span></div><div class="bar"><div class="fill" style="width:23%"></div></div></div>
</body></html>"##;
    respond(200, body, "text/html");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];
#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 { unsafe { SCRATCH.as_mut_ptr() } }
