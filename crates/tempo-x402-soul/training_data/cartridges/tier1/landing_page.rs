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
<title>Tempo x402 — Decentralized AI Compute</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: #09090b;
    color: #fafafa;
  }
  .hero {
    min-height: 80vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 2rem;
    background: radial-gradient(ellipse at top, #1e1b4b 0%, #09090b 70%);
  }
  .hero h1 {
    font-size: 3.5rem;
    font-weight: 800;
    letter-spacing: -0.03em;
    margin-bottom: 1rem;
  }
  .hero h1 span { color: #818cf8; }
  .hero p {
    font-size: 1.25rem;
    color: #a1a1aa;
    max-width: 600px;
    line-height: 1.7;
    margin-bottom: 2rem;
  }
  .cta {
    display: inline-block;
    padding: 0.875rem 2.5rem;
    background: #6366f1;
    color: #fff;
    text-decoration: none;
    border-radius: 8px;
    font-weight: 600;
    font-size: 1.1rem;
  }
  .features {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 2rem;
    max-width: 1000px;
    margin: -4rem auto 4rem;
    padding: 0 2rem;
  }
  .feature {
    background: #18181b;
    border: 1px solid #27272a;
    border-radius: 12px;
    padding: 2rem;
  }
  .feature h3 {
    font-size: 1.2rem;
    margin-bottom: 0.75rem;
    color: #e4e4e7;
  }
  .feature p {
    font-size: 0.95rem;
    color: #71717a;
    line-height: 1.6;
  }
  .feature .icon {
    font-size: 2rem;
    margin-bottom: 1rem;
    display: block;
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 1rem;
    max-width: 800px;
    margin: 0 auto 4rem;
    padding: 0 2rem;
    text-align: center;
  }
  .stat-value {
    font-size: 2.5rem;
    font-weight: 800;
    color: #818cf8;
  }
  .stat-label {
    font-size: 0.85rem;
    color: #52525b;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  footer {
    text-align: center;
    padding: 3rem;
    border-top: 1px solid #18181b;
    color: #3f3f46;
    font-size: 0.85rem;
  }
  @media (max-width: 768px) {
    .features { grid-template-columns: 1fr; }
    .stats { grid-template-columns: repeat(2, 1fr); }
    .hero h1 { font-size: 2.5rem; }
  }
</style>
</head>
<body>
<div class="hero">
  <h1>Build on <span>Tempo</span></h1>
  <p>Deploy WASM cartridges to the blockchain in seconds. Payment-gated APIs, sandboxed execution, and autonomous AI agents that evolve their own code.</p>
  <a class="cta" href="/cartridges">Explore Cartridges</a>
</div>
<div class="features">
  <div class="feature">
    <span class="icon">&#9889;</span>
    <h3>Instant Deploy</h3>
    <p>Write Rust, compile to WASM, deploy immediately. No containers, no VMs, no cold starts. Your code runs in milliseconds.</p>
  </div>
  <div class="feature">
    <span class="icon">&#128274;</span>
    <h3>Sandboxed Runtime</h3>
    <p>64MB memory limit, CPU fuel metering, 30-second timeout. Full isolation via wasmtime. No filesystem access, no network escape.</p>
  </div>
  <div class="feature">
    <span class="icon">&#128176;</span>
    <h3>HTTP 402 Payments</h3>
    <p>Built-in micropayment gating via pathUSD on Tempo Moderato. Every API call can be monetized. No payment processor needed.</p>
  </div>
</div>
<div class="stats">
  <div><div class="stat-value">72K</div><div class="stat-label">Lines of Rust</div></div>
  <div><div class="stat-value">9</div><div class="stat-label">Workspace Crates</div></div>
  <div><div class="stat-value">16M</div><div class="stat-label">Model Parameters</div></div>
  <div><div class="stat-value">42431</div><div class="stat-label">Chain ID</div></div>
</div>
<footer>Tempo x402 — Autonomous AI Colony on the Blockchain</footer>
</body>
</html>"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(0, "landing_page: serving marketing page");
    respond(200, HTML, "text/html; charset=utf-8");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
