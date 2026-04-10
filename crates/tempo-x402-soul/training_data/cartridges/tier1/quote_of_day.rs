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
<title>Quote of the Day</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: 'Georgia', 'Times New Roman', serif;
    background: #fdf6e3;
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2rem;
  }
  .card {
    max-width: 560px;
    background: #fff;
    border-radius: 16px;
    box-shadow: 0 4px 24px rgba(0,0,0,0.08), 0 1px 2px rgba(0,0,0,0.04);
    padding: 3rem;
    position: relative;
    overflow: hidden;
  }
  .card::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 4px;
    background: linear-gradient(90deg, #d97706, #f59e0b, #fbbf24);
  }
  .quote-mark {
    font-size: 5rem;
    color: #fbbf24;
    line-height: 1;
    margin-bottom: -1rem;
    font-family: Georgia, serif;
  }
  .quote-text {
    font-size: 1.4rem;
    line-height: 1.8;
    color: #292524;
    font-style: italic;
    margin-bottom: 1.5rem;
  }
  .divider {
    width: 60px;
    height: 2px;
    background: #e7e5e4;
    margin-bottom: 1.5rem;
  }
  .author {
    font-size: 1rem;
    color: #78716c;
    font-style: normal;
    font-weight: 600;
  }
  .author-title {
    font-size: 0.85rem;
    color: #a8a29e;
    margin-top: 0.25rem;
  }
  .date {
    position: absolute;
    top: 1.5rem;
    right: 2rem;
    font-size: 0.75rem;
    color: #d6d3d1;
    font-family: -apple-system, sans-serif;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
</style>
</head>
<body>
<div class="card">
  <div class="date">Quote of the Day</div>
  <div class="quote-mark">&ldquo;</div>
  <p class="quote-text">The best way to predict the future is to invent it. The second best way is to write software that invents itself.</p>
  <div class="divider"></div>
  <div class="author">Alan Kay</div>
  <div class="author-title">Computer Scientist, Turing Award Laureate</div>
</div>
</body>
</html>"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(0, "quote_of_day: serving daily quote");
    respond(200, HTML, "text/html; charset=utf-8");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
