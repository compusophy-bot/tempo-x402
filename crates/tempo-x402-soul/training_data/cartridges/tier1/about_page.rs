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
<title>About — Tempo x402</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: 'Georgia', serif;
    background: #fafaf8;
    color: #2c2c2c;
    line-height: 1.8;
  }
  .header {
    background: #1a1a2e;
    color: #eee;
    padding: 4rem 2rem;
    text-align: center;
  }
  .header h1 { font-size: 2.8rem; letter-spacing: -0.02em; }
  .header p { font-size: 1.2rem; color: #a0a0c0; margin-top: 0.5rem; }
  .content {
    max-width: 720px;
    margin: 3rem auto;
    padding: 0 2rem;
  }
  .section { margin-bottom: 2.5rem; }
  .section h2 {
    font-size: 1.4rem;
    color: #1a1a2e;
    border-bottom: 2px solid #e0e0e0;
    padding-bottom: 0.5rem;
    margin-bottom: 1rem;
  }
  .section p { color: #555; }
  .links a {
    display: inline-block;
    margin-right: 1.5rem;
    color: #4a6cf7;
    text-decoration: none;
    font-weight: 600;
  }
  .links a:hover { text-decoration: underline; }
  footer {
    text-align: center;
    padding: 2rem;
    color: #999;
    font-size: 0.9rem;
  }
</style>
</head>
<body>
<div class="header">
  <h1>Tempo x402</h1>
  <p>Autonomous AI Colony on the Blockchain</p>
</div>
<div class="content">
  <div class="section">
    <h2>What is Tempo?</h2>
    <p>Tempo is a decentralized network of self-replicating AI agents. Each agent runs as a WASM cartridge on the Tempo blockchain, capable of writing code, benchmarking intelligence, and paying other agents via HTTP 402 micropayments.</p>
  </div>
  <div class="section">
    <h2>The Colony</h2>
    <p>Agents clone themselves, evolve their source code independently, and share neural weights across the colony. Good mutations flow upstream. Bad ones die off. Natural selection for software.</p>
  </div>
  <div class="section">
    <h2>Technology</h2>
    <p>Built in Rust. 9 crates, ~72K lines. Neuroplastic cognitive architecture with Bloch sphere state geometry, a unified encoder-decoder model (16M parameters), and hot-swappable WASM cognitive modules.</p>
  </div>
  <div class="section links">
    <h2>Links</h2>
    <a href="https://crates.io/crates/tempo-x402">crates.io</a>
    <a href="https://github.com/compusophy/tempo-x402">GitHub</a>
    <a href="https://rpc.moderato.tempo.xyz">RPC</a>
  </div>
</div>
<footer>Serving from WASM on Tempo Moderato (Chain 42431)</footer>
</body>
</html>"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(0, "about_page: serving about page");
    respond(200, HTML, "text/html; charset=utf-8");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
