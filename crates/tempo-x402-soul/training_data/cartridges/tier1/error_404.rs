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

const HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>404 — Not Found</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: #0d1117;
    color: #c9d1d9;
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
  }
  .code {
    font-size: 8rem;
    font-weight: 800;
    color: #f85149;
    line-height: 1;
    text-shadow: 0 0 40px rgba(248,81,73,0.3);
  }
  .message {
    font-size: 1.5rem;
    color: #8b949e;
    margin: 1rem 0 2rem;
  }
  .hint {
    font-size: 0.95rem;
    color: #484f58;
    max-width: 400px;
    line-height: 1.6;
  }
  .home-link {
    display: inline-block;
    margin-top: 2rem;
    padding: 0.75rem 2rem;
    background: #238636;
    color: #fff;
    text-decoration: none;
    border-radius: 6px;
    font-weight: 600;
    transition: background 0.2s;
  }
  .home-link:hover { background: #2ea043; }
  .glitch {
    position: relative;
  }
  .glitch::before, .glitch::after {
    content: '404';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
  }
  .glitch::before {
    color: #58a6ff;
    clip-path: inset(0 0 60% 0);
    transform: translate(-2px, -2px);
  }
  .glitch::after {
    color: #f0883e;
    clip-path: inset(60% 0 0 0);
    transform: translate(2px, 2px);
  }
</style>
</head>
<body>
  <div class="code glitch">404</div>
  <p class="message">This page could not be found.</p>
  <p class="hint">The cartridge you requested does not exist or has been unloaded. Check the URL and try again, or browse available cartridges from the studio.</p>
  <a class="home-link" href="/">Back to Home</a>
</body>
</html>"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(1, "error_404: resource not found");
    respond(404, HTML, "text/html; charset=utf-8");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
