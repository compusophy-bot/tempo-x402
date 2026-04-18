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
<title>Changelog — Tempo x402</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: #fff;
    color: #24292f;
    line-height: 1.6;
  }
  .container {
    max-width: 720px;
    margin: 0 auto;
    padding: 3rem 2rem;
  }
  h1 {
    font-size: 2rem;
    margin-bottom: 0.5rem;
  }
  .lead {
    color: #656d76;
    margin-bottom: 3rem;
    font-size: 1.05rem;
  }
  .release {
    position: relative;
    padding-left: 2rem;
    margin-bottom: 2.5rem;
    border-left: 2px solid #d0d7de;
  }
  .release::before {
    content: '';
    position: absolute;
    left: -7px;
    top: 4px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #fff;
    border: 2px solid #0969da;
  }
  .release.latest::before {
    background: #0969da;
  }
  .version {
    font-size: 1.3rem;
    font-weight: 700;
    color: #0969da;
  }
  .date {
    font-size: 0.85rem;
    color: #656d76;
    margin-bottom: 0.75rem;
  }
  .tag {
    display: inline-block;
    padding: 0.1rem 0.5rem;
    border-radius: 12px;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    margin-right: 0.25rem;
    vertical-align: middle;
  }
  .tag-feat { background: #dafbe1; color: #116329; }
  .tag-fix { background: #fff8c5; color: #6a5300; }
  .tag-break { background: #ffebe9; color: #82071e; }
  ul {
    list-style: none;
    margin-top: 0.5rem;
  }
  li {
    padding: 0.3rem 0;
    color: #424a53;
    font-size: 0.95rem;
  }
  li::before {
    content: '\2022';
    color: #d0d7de;
    margin-right: 0.5rem;
  }
</style>
</head>
<body>
<div class="container">
  <h1>Changelog</h1>
  <p class="lead">All notable changes to the Tempo x402 platform.</p>

  <div class="release latest">
    <div class="version">v9.1.7 <span class="tag tag-feat">feature</span></div>
    <div class="date">April 10, 2026</div>
    <ul>
      <li>Workers compute their own ELO from benchmark results</li>
      <li>Self-assessment loop reduces dependency on queen evaluation</li>
      <li>ELO delta tracked per-commit for regression detection</li>
    </ul>
  </div>

  <div class="release">
    <div class="version">v9.1.0 <span class="tag tag-feat">feature</span></div>
    <div class="date">April 8, 2026</div>
    <ul>
      <li>Cartridge system overhaul: hot-reload on recompile</li>
      <li>KV persistence across cartridge executions</li>
      <li>Four cartridge types: Backend, Interactive, Frontend, Cognitive</li>
      <li>CognitiveOrchestrator wired into ThinkingLoop</li>
      <li>Soul tool limits raised: Observe 10, Code 25, Review 10</li>
    </ul>
  </div>

  <div class="release">
    <div class="version">v9.0.0 <span class="tag tag-break">breaking</span></div>
    <div class="date">April 8, 2026</div>
    <ul>
      <li>Neuroplastic fluid cognitive architecture</li>
      <li>Bloch sphere state geometry for agent cognition</li>
      <li>Unified encoder-decoder model (16M parameters)</li>
      <li>201 benchmark problems across 5 tiers</li>
    </ul>
  </div>

  <div class="release">
    <div class="version">v7.0.0 <span class="tag tag-feat">feature</span></div>
    <div class="date">April 4, 2026</div>
    <ul>
      <li>Studio rebuild with live cartridge browser</li>
      <li>Rayon parallelization for benchmark execution</li>
      <li>Sled database migration completed</li>
    </ul>
  </div>

  <div class="release">
    <div class="version">v6.8.0 <span class="tag tag-fix">fix</span></div>
    <div class="date">April 2, 2026</div>
    <ul>
      <li>Benchmark as core learning signal</li>
      <li>Five learning pipeline fixes</li>
      <li>Codegen backprop through embeddings and FFN</li>
    </ul>
  </div>
</div>
</body>
</html>"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(0, "changelog: serving version history");
    respond(200, HTML, "text/html; charset=utf-8");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
