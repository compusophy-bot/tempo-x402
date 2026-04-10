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
<title>Project Timeline</title>
<style>
  *, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: 'Segoe UI', system-ui, sans-serif; background: #fafafa; color: #1e293b; padding: 60px 20px; }
  .container { max-width: 720px; margin: 0 auto; }
  .header { text-align: center; margin-bottom: 56px; }
  .header h1 { font-size: 2rem; font-weight: 800; margin-bottom: 8px; }
  .header p { color: #64748b; font-size: 1rem; }
  .timeline { position: relative; padding-left: 44px; }
  .timeline::before {
    content: ''; position: absolute; left: 15px; top: 8px; bottom: 8px;
    width: 2px; background: linear-gradient(180deg, #3b82f6, #8b5cf6, #ec4899);
    border-radius: 1px;
  }
  .event { position: relative; margin-bottom: 40px; }
  .event:last-child { margin-bottom: 0; }
  .dot {
    position: absolute; left: -44px; top: 4px;
    width: 30px; height: 30px; border-radius: 50%;
    display: flex; align-items: center; justify-content: center;
    font-size: 0.85rem; font-weight: 700; color: #fff;
    box-shadow: 0 2px 8px rgba(0,0,0,0.15);
  }
  .dot.blue { background: #3b82f6; }
  .dot.purple { background: #8b5cf6; }
  .dot.pink { background: #ec4899; }
  .dot.green { background: #22c55e; }
  .dot.amber { background: #f59e0b; }
  .dot.current {
    animation: ring 2s ease-in-out infinite;
  }
  @keyframes ring {
    0%, 100% { box-shadow: 0 0 0 0 rgba(59,130,246,0.4); }
    50% { box-shadow: 0 0 0 8px rgba(59,130,246,0); }
  }
  .card {
    background: #fff; border-radius: 14px; padding: 24px;
    box-shadow: 0 2px 12px rgba(0,0,0,0.04);
    border: 1px solid #f1f5f9;
    transition: transform 0.2s, box-shadow 0.2s;
  }
  .card:hover { transform: translateX(4px); box-shadow: 0 4px 20px rgba(0,0,0,0.08); }
  .card .date {
    font-size: 0.78rem; font-weight: 700; text-transform: uppercase;
    letter-spacing: 1px; margin-bottom: 8px;
  }
  .date.blue { color: #3b82f6; }
  .date.purple { color: #8b5cf6; }
  .date.pink { color: #ec4899; }
  .date.green { color: #22c55e; }
  .date.amber { color: #f59e0b; }
  .card h3 { font-size: 1.1rem; font-weight: 700; margin-bottom: 8px; }
  .card p { font-size: 0.9rem; color: #64748b; line-height: 1.6; }
  .card .tags { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 12px; }
  .card .tag {
    font-size: 0.72rem; padding: 3px 10px; border-radius: 6px;
    background: #f1f5f9; color: #475569; font-weight: 600;
  }
  .status-badge {
    display: inline-block; font-size: 0.72rem; font-weight: 700;
    padding: 2px 8px; border-radius: 4px; margin-left: 8px;
    vertical-align: middle;
  }
  .status-badge.done { background: #dcfce7; color: #16a34a; }
  .status-badge.active { background: #dbeafe; color: #2563eb; }
  .status-badge.upcoming { background: #f1f5f9; color: #64748b; }
</style>
</head>
<body>
  <div class="container">
    <div class="header">
      <h1>Project Timeline</h1>
      <p>Key milestones and deliverables for Project Phoenix</p>
    </div>
    <div class="timeline">
      <div class="event">
        <div class="dot green">&#x2713;</div>
        <div class="card">
          <div class="date green">Jan 15, 2026</div>
          <h3>Project Kickoff <span class="status-badge done">DONE</span></h3>
          <p>Initial team assembly, stakeholder alignment, and scope definition. Established communication channels and sprint cadence.</p>
          <div class="tags"><span class="tag">Planning</span><span class="tag">Team</span></div>
        </div>
      </div>
      <div class="event">
        <div class="dot green">&#x2713;</div>
        <div class="card">
          <div class="date green">Feb 8, 2026</div>
          <h3>Architecture Review <span class="status-badge done">DONE</span></h3>
          <p>Finalized system architecture with microservices approach. Selected Rust for core engine, TypeScript for client SDK. Approved by security team.</p>
          <div class="tags"><span class="tag">Architecture</span><span class="tag">Security</span><span class="tag">Review</span></div>
        </div>
      </div>
      <div class="event">
        <div class="dot green">&#x2713;</div>
        <div class="card">
          <div class="date green">Mar 1, 2026</div>
          <h3>Core Engine v1.0 <span class="status-badge done">DONE</span></h3>
          <p>Shipped first version of the processing engine with 2M events/sec throughput. Includes hot-reload, graceful shutdown, and telemetry.</p>
          <div class="tags"><span class="tag">Engineering</span><span class="tag">Rust</span><span class="tag">Performance</span></div>
        </div>
      </div>
      <div class="event">
        <div class="dot blue current">&#x25B6;</div>
        <div class="card">
          <div class="date blue">Apr 10, 2026</div>
          <h3>Beta Launch <span class="status-badge active">IN PROGRESS</span></h3>
          <p>Rolling out to 500 beta users. Collecting feedback on API ergonomics, dashboard UX, and onboarding flow. Load testing at 10x capacity.</p>
          <div class="tags"><span class="tag">Beta</span><span class="tag">UX</span><span class="tag">Testing</span></div>
        </div>
      </div>
      <div class="event">
        <div class="dot purple">5</div>
        <div class="card">
          <div class="date purple">May 20, 2026</div>
          <h3>Public API GA <span class="status-badge upcoming">UPCOMING</span></h3>
          <p>General availability of the public API with SDKs for Python, TypeScript, Go, and Rust. Rate limiting, billing, and usage dashboards ready.</p>
          <div class="tags"><span class="tag">API</span><span class="tag">SDK</span><span class="tag">Launch</span></div>
        </div>
      </div>
      <div class="event">
        <div class="dot pink">6</div>
        <div class="card">
          <div class="date pink">Jul 1, 2026</div>
          <h3>Enterprise Features <span class="status-badge upcoming">UPCOMING</span></h3>
          <p>SSO/SAML, audit logging, custom roles, SOC 2 compliance, dedicated infrastructure, and SLA guarantees for enterprise customers.</p>
          <div class="tags"><span class="tag">Enterprise</span><span class="tag">Security</span><span class="tag">Compliance</span></div>
        </div>
      </div>
      <div class="event">
        <div class="dot amber">7</div>
        <div class="card">
          <div class="date amber">Sep 15, 2026</div>
          <h3>v2.0 — ML Pipeline <span class="status-badge upcoming">UPCOMING</span></h3>
          <p>Integrated machine learning pipeline with real-time inference, model versioning, A/B testing, and automated retraining on drift detection.</p>
          <div class="tags"><span class="tag">ML</span><span class="tag">v2.0</span><span class="tag">AI</span></div>
        </div>
      </div>
    </div>
  </div>
</body>
</html>"##;

#[no_mangle]
pub extern "C" fn x402_handle() {
    host_log(1, "timeline_page: serving project timeline");
    respond(200, BODY, "text/html; charset=utf-8");
}
