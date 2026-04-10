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
    host_log(1, "keyboard-shortcuts invoked");
    let body = r##"<!DOCTYPE html><html><head><meta charset="UTF-8"><title>Keyboard Shortcuts</title>
<style>*{margin:0;padding:0;box-sizing:border-box}body{background:##0d1117;color:##c9d1d9;font-family:-apple-system,sans-serif;padding:32px;max-width:700px;margin:0 auto}
h1{color:##e6edf3;margin-bottom:24px;font-size:1.8rem}
.group{margin-bottom:24px}.group-title{font-size:0.8rem;text-transform:uppercase;letter-spacing:0.1em;color:##8b949e;margin-bottom:8px;padding-bottom:4px;border-bottom:1px solid ##21262d}
.shortcut{display:flex;justify-content:space-between;align-items:center;padding:8px 0;border-bottom:1px solid ##161b22}
.desc{color:##c9d1d9}.keys{display:flex;gap:4px}
kbd{background:##21262d;border:1px solid ##30363d;border-radius:4px;padding:2px 8px;font-family:monospace;font-size:0.85rem;color:##e6edf3;box-shadow:0 1px 0 ##0d1117}
</style></head><body><h1>Keyboard Shortcuts</h1>
<div class="group"><div class="group-title">Navigation</div>
<div class="shortcut"><span class="desc">Go to dashboard</span><div class="keys"><kbd>G</kbd><kbd>D</kbd></div></div>
<div class="shortcut"><span class="desc">Go to settings</span><div class="keys"><kbd>G</kbd><kbd>S</kbd></div></div>
<div class="shortcut"><span class="desc">Search</span><div class="keys"><kbd>Ctrl</kbd><kbd>K</kbd></div></div>
</div>
<div class="group"><div class="group-title">Actions</div>
<div class="shortcut"><span class="desc">New item</span><div class="keys"><kbd>N</kbd></div></div>
<div class="shortcut"><span class="desc">Save</span><div class="keys"><kbd>Ctrl</kbd><kbd>S</kbd></div></div>
<div class="shortcut"><span class="desc">Delete</span><div class="keys"><kbd>Del</kbd></div></div>
<div class="shortcut"><span class="desc">Undo</span><div class="keys"><kbd>Ctrl</kbd><kbd>Z</kbd></div></div>
</div></body></html>"##;
    respond(200, body, "text/html");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];
#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 { unsafe { SCRATCH.as_mut_ptr() } }
