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

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}

const BODY: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Alex Chen — Developer Portfolio</title>
<style>
  *, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }
  :root {
    --bg: #0f172a; --surface: #1e293b; --border: #334155;
    --text: #e2e8f0; --muted: #94a3b8; --accent: #38bdf8;
    --accent2: #a78bfa; --accent3: #34d399; --radius: 12px;
  }
  body { font-family: 'Segoe UI', system-ui, sans-serif; background: var(--bg); color: var(--text); line-height: 1.6; }
  .hero {
    text-align: center; padding: 80px 20px 60px;
    background: linear-gradient(135deg, #0f172a 0%, #1e1b4b 50%, #0f172a 100%);
  }
  .avatar {
    width: 120px; height: 120px; border-radius: 50%; margin: 0 auto 24px;
    background: linear-gradient(135deg, var(--accent), var(--accent2));
    display: flex; align-items: center; justify-content: center;
    font-size: 48px; font-weight: 700; color: #fff;
  }
  .hero h1 { font-size: 2.5rem; font-weight: 800; margin-bottom: 8px; }
  .hero p { color: var(--muted); font-size: 1.15rem; max-width: 500px; margin: 0 auto; }
  .skills {
    display: flex; flex-wrap: wrap; gap: 10px; justify-content: center;
    margin-top: 28px; padding: 0 20px;
  }
  .skill {
    padding: 6px 16px; border-radius: 20px; font-size: 0.85rem; font-weight: 600;
    background: rgba(56, 189, 248, 0.12); color: var(--accent); border: 1px solid rgba(56, 189, 248, 0.25);
  }
  .skill.purple { background: rgba(167, 139, 250, 0.12); color: var(--accent2); border-color: rgba(167, 139, 250, 0.25); }
  .skill.green { background: rgba(52, 211, 153, 0.12); color: var(--accent3); border-color: rgba(52, 211, 153, 0.25); }
  .section { max-width: 1000px; margin: 0 auto; padding: 60px 20px; }
  .section h2 { font-size: 1.6rem; font-weight: 700; margin-bottom: 32px; text-align: center; }
  .projects { display: grid; grid-template-columns: repeat(auto-fill, minmax(290px, 1fr)); gap: 24px; }
  .card {
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius);
    padding: 28px; transition: transform 0.2s, box-shadow 0.2s;
  }
  .card:hover { transform: translateY(-4px); box-shadow: 0 12px 32px rgba(0,0,0,0.3); }
  .card-icon { font-size: 2rem; margin-bottom: 16px; }
  .card h3 { font-size: 1.15rem; margin-bottom: 8px; }
  .card p { color: var(--muted); font-size: 0.92rem; margin-bottom: 16px; }
  .tags { display: flex; flex-wrap: wrap; gap: 6px; }
  .tag { padding: 3px 10px; border-radius: 6px; font-size: 0.75rem; background: rgba(255,255,255,0.06); color: var(--muted); }
  .contact { text-align: center; padding: 60px 20px; border-top: 1px solid var(--border); }
  .contact a { color: var(--accent); text-decoration: none; font-weight: 600; }
  .contact a:hover { text-decoration: underline; }
</style>
</head>
<body>
  <div class="hero">
    <div class="avatar">AC</div>
    <h1>Alex Chen</h1>
    <p>Full-stack developer building fast, elegant tools for the modern web.</p>
    <div class="skills">
      <span class="skill">Rust</span>
      <span class="skill">TypeScript</span>
      <span class="skill purple">WebAssembly</span>
      <span class="skill purple">React</span>
      <span class="skill green">PostgreSQL</span>
      <span class="skill green">Docker</span>
      <span class="skill">GraphQL</span>
      <span class="skill purple">Kubernetes</span>
    </div>
  </div>
  <div class="section">
    <h2>Featured Projects</h2>
    <div class="projects">
      <div class="card">
        <div class="card-icon">&#x1F680;</div>
        <h3>Velocity Engine</h3>
        <p>High-performance WASM runtime for edge compute. Sub-millisecond cold starts with snapshot restore.</p>
        <div class="tags"><span class="tag">Rust</span><span class="tag">WASM</span><span class="tag">Edge</span></div>
      </div>
      <div class="card">
        <div class="card-icon">&#x1F4CA;</div>
        <h3>DataPulse</h3>
        <p>Real-time analytics dashboard with streaming SQL. Handles 1M events/sec on commodity hardware.</p>
        <div class="tags"><span class="tag">TypeScript</span><span class="tag">React</span><span class="tag">ClickHouse</span></div>
      </div>
      <div class="card">
        <div class="card-icon">&#x1F512;</div>
        <h3>VaultKeeper</h3>
        <p>Zero-knowledge secrets manager with hardware key support. End-to-end encrypted sharing.</p>
        <div class="tags"><span class="tag">Rust</span><span class="tag">Crypto</span><span class="tag">FIDO2</span></div>
      </div>
      <div class="card">
        <div class="card-icon">&#x1F310;</div>
        <h3>MeshSync</h3>
        <p>Distributed CRDT-based collaboration framework. Offline-first with automatic conflict resolution.</p>
        <div class="tags"><span class="tag">Rust</span><span class="tag">CRDT</span><span class="tag">P2P</span></div>
      </div>
      <div class="card">
        <div class="card-icon">&#x26A1;</div>
        <h3>Spark CLI</h3>
        <p>Universal project scaffolder with 200+ templates. Intelligent dependency resolution and caching.</p>
        <div class="tags"><span class="tag">Rust</span><span class="tag">CLI</span><span class="tag">Templates</span></div>
      </div>
      <div class="card">
        <div class="card-icon">&#x1F916;</div>
        <h3>NeuralForge</h3>
        <p>ML model serving platform with automatic batching, quantization, and A/B testing built in.</p>
        <div class="tags"><span class="tag">Python</span><span class="tag">ONNX</span><span class="tag">gRPC</span></div>
      </div>
    </div>
  </div>
  <div class="contact">
    <h2>Get in Touch</h2>
    <p style="color: var(--muted); margin-top: 12px;">
      <a href="#">GitHub</a> &middot; <a href="#">LinkedIn</a> &middot; <a href="#">alex@example.dev</a>
    </p>
  </div>
</body>
</html>"##;

#[no_mangle]
pub extern "C" fn x402_handle() {
    host_log(1, "portfolio_page: serving developer portfolio");
    respond(200, BODY, "text/html; charset=utf-8");
}
