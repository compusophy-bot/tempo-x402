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
<title>Color Palette — Catppuccin Mocha</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: #1e1e2e;
    color: #cdd6f4;
    padding: 3rem 2rem;
  }
  h1 {
    text-align: center;
    font-size: 2rem;
    margin-bottom: 0.5rem;
  }
  .subtitle {
    text-align: center;
    color: #6c7086;
    margin-bottom: 3rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 1rem;
    max-width: 900px;
    margin: 0 auto;
  }
  .swatch {
    border-radius: 12px;
    overflow: hidden;
    background: #181825;
    border: 1px solid #313244;
    transition: transform 0.15s;
  }
  .swatch:hover { transform: translateY(-2px); }
  .color-block {
    height: 100px;
    width: 100%;
  }
  .swatch-info {
    padding: 0.75rem;
  }
  .swatch-name {
    font-weight: 600;
    font-size: 0.85rem;
    margin-bottom: 0.25rem;
  }
  .swatch-hex {
    font-family: 'SF Mono', 'Fira Code', monospace;
    font-size: 0.8rem;
    color: #6c7086;
  }
</style>
</head>
<body>
<h1>Catppuccin Mocha</h1>
<p class="subtitle">A soothing pastel theme for the high-spirited</p>
<div class="grid">
  <div class="swatch">
    <div class="color-block" style="background:#f38ba8;"></div>
    <div class="swatch-info"><div class="swatch-name">Red</div><div class="swatch-hex">#f38ba8</div></div>
  </div>
  <div class="swatch">
    <div class="color-block" style="background:#fab387;"></div>
    <div class="swatch-info"><div class="swatch-name">Peach</div><div class="swatch-hex">#fab387</div></div>
  </div>
  <div class="swatch">
    <div class="color-block" style="background:#f9e2af;"></div>
    <div class="swatch-info"><div class="swatch-name">Yellow</div><div class="swatch-hex">#f9e2af</div></div>
  </div>
  <div class="swatch">
    <div class="color-block" style="background:#a6e3a1;"></div>
    <div class="swatch-info"><div class="swatch-name">Green</div><div class="swatch-hex">#a6e3a1</div></div>
  </div>
  <div class="swatch">
    <div class="color-block" style="background:#94e2d5;"></div>
    <div class="swatch-info"><div class="swatch-name">Teal</div><div class="swatch-hex">#94e2d5</div></div>
  </div>
  <div class="swatch">
    <div class="color-block" style="background:#89dceb;"></div>
    <div class="swatch-info"><div class="swatch-name">Sky</div><div class="swatch-hex">#89dceb</div></div>
  </div>
  <div class="swatch">
    <div class="color-block" style="background:#74c7ec;"></div>
    <div class="swatch-info"><div class="swatch-name">Sapphire</div><div class="swatch-hex">#74c7ec</div></div>
  </div>
  <div class="swatch">
    <div class="color-block" style="background:#89b4fa;"></div>
    <div class="swatch-info"><div class="swatch-name">Blue</div><div class="swatch-hex">#89b4fa</div></div>
  </div>
  <div class="swatch">
    <div class="color-block" style="background:#b4befe;"></div>
    <div class="swatch-info"><div class="swatch-name">Lavender</div><div class="swatch-hex">#b4befe</div></div>
  </div>
  <div class="swatch">
    <div class="color-block" style="background:#cba6f7;"></div>
    <div class="swatch-info"><div class="swatch-name">Mauve</div><div class="swatch-hex">#cba6f7</div></div>
  </div>
  <div class="swatch">
    <div class="color-block" style="background:#f5c2e7;"></div>
    <div class="swatch-info"><div class="swatch-name">Pink</div><div class="swatch-hex">#f5c2e7</div></div>
  </div>
  <div class="swatch">
    <div class="color-block" style="background:#eba0ac;"></div>
    <div class="swatch-info"><div class="swatch-name">Maroon</div><div class="swatch-hex">#eba0ac</div></div>
  </div>
</div>
</body>
</html>"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(0, "color_palette: serving palette grid");
    respond(200, HTML, "text/html; charset=utf-8");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
