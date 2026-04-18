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
<title>Metrics Display</title>
<style>
  *, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }
  :root { --bg: #0f172a; --surface: #1e293b; --border: #334155; --text: #e2e8f0; --muted: #94a3b8; }
  body { font-family: 'Segoe UI', system-ui, sans-serif; background: var(--bg); color: var(--text); padding: 40px 20px; }
  .container { max-width: 1100px; margin: 0 auto; }
  h1 { font-size: 1.5rem; font-weight: 700; margin-bottom: 8px; }
  .subtitle { color: var(--muted); font-size: 0.9rem; margin-bottom: 36px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 20px; margin-bottom: 32px; }
  .metric-card {
    background: var(--surface); border: 1px solid var(--border); border-radius: 16px;
    padding: 28px; position: relative; overflow: hidden;
  }
  .metric-card::before {
    content: ''; position: absolute; top: 0; left: 0; right: 0; height: 3px;
  }
  .metric-card.blue::before { background: linear-gradient(90deg, #3b82f6, #60a5fa); }
  .metric-card.green::before { background: linear-gradient(90deg, #22c55e, #4ade80); }
  .metric-card.purple::before { background: linear-gradient(90deg, #8b5cf6, #a78bfa); }
  .metric-card.amber::before { background: linear-gradient(90deg, #f59e0b, #fbbf24); }
  .metric-card.rose::before { background: linear-gradient(90deg, #f43f5e, #fb7185); }
  .metric-card.cyan::before { background: linear-gradient(90deg, #06b6d4, #22d3ee); }
  .metric-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px; }
  .metric-label { font-size: 0.82rem; color: var(--muted); font-weight: 600; text-transform: uppercase; letter-spacing: 1px; }
  .metric-icon { width: 36px; height: 36px; border-radius: 10px; display: flex; align-items: center; justify-content: center; font-size: 1.1rem; }
  .blue .metric-icon { background: rgba(59,130,246,0.15); }
  .green .metric-icon { background: rgba(34,197,94,0.15); }
  .purple .metric-icon { background: rgba(139,92,246,0.15); }
  .amber .metric-icon { background: rgba(245,158,11,0.15); }
  .rose .metric-icon { background: rgba(244,63,94,0.15); }
  .cyan .metric-icon { background: rgba(6,182,212,0.15); }
  .metric-value { font-size: 2.5rem; font-weight: 800; line-height: 1; margin-bottom: 8px; font-variant-numeric: tabular-nums; }
  .trend { display: inline-flex; align-items: center; gap: 4px; font-size: 0.82rem; font-weight: 600; padding: 3px 8px; border-radius: 6px; }
  .trend.up { background: rgba(34,197,94,0.12); color: #4ade80; }
  .trend.down { background: rgba(244,63,94,0.12); color: #fb7185; }
  .trend.flat { background: rgba(148,163,184,0.12); color: #94a3b8; }
  .sparkline { display: flex; align-items: flex-end; gap: 3px; height: 40px; margin-top: 16px; }
  .spark-bar { flex: 1; border-radius: 2px; min-width: 4px; transition: height 0.3s; }
  .blue .spark-bar { background: rgba(59,130,246,0.4); }
  .green .spark-bar { background: rgba(34,197,94,0.4); }
  .purple .spark-bar { background: rgba(139,92,246,0.4); }
  .amber .spark-bar { background: rgba(245,158,11,0.4); }
  .rose .spark-bar { background: rgba(244,63,94,0.4); }
  .cyan .spark-bar { background: rgba(6,182,212,0.4); }
  .spark-bar:last-child { opacity: 1; }
  .period { text-align: right; font-size: 0.78rem; color: #475569; margin-top: 4px; }
</style>
</head>
<body>
  <div class="container">
    <h1>Platform Metrics</h1>
    <p class="subtitle">Real-time overview for the last 30 days</p>
    <div class="grid">
      <div class="metric-card blue">
        <div class="metric-header">
          <span class="metric-label">Total Revenue</span>
          <div class="metric-icon">&#x1F4B0;</div>
        </div>
        <div class="metric-value">$847K</div>
        <span class="trend up">&#x2191; 12.5%</span>
        <div class="sparkline">
          <div class="spark-bar" style="height:45%"></div><div class="spark-bar" style="height:55%"></div>
          <div class="spark-bar" style="height:40%"></div><div class="spark-bar" style="height:65%"></div>
          <div class="spark-bar" style="height:50%"></div><div class="spark-bar" style="height:70%"></div>
          <div class="spark-bar" style="height:60%"></div><div class="spark-bar" style="height:75%"></div>
          <div class="spark-bar" style="height:80%"></div><div class="spark-bar" style="height:90%"></div>
          <div class="spark-bar" style="height:85%"></div><div class="spark-bar" style="height:95%"></div>
        </div>
      </div>
      <div class="metric-card green">
        <div class="metric-header">
          <span class="metric-label">Active Users</span>
          <div class="metric-icon">&#x1F465;</div>
        </div>
        <div class="metric-value">24.8K</div>
        <span class="trend up">&#x2191; 8.3%</span>
        <div class="sparkline">
          <div class="spark-bar" style="height:60%"></div><div class="spark-bar" style="height:55%"></div>
          <div class="spark-bar" style="height:65%"></div><div class="spark-bar" style="height:70%"></div>
          <div class="spark-bar" style="height:68%"></div><div class="spark-bar" style="height:72%"></div>
          <div class="spark-bar" style="height:75%"></div><div class="spark-bar" style="height:78%"></div>
          <div class="spark-bar" style="height:80%"></div><div class="spark-bar" style="height:82%"></div>
          <div class="spark-bar" style="height:85%"></div><div class="spark-bar" style="height:88%"></div>
        </div>
      </div>
      <div class="metric-card purple">
        <div class="metric-header">
          <span class="metric-label">Conversion Rate</span>
          <div class="metric-icon">&#x1F4C8;</div>
        </div>
        <div class="metric-value">3.42%</div>
        <span class="trend down">&#x2193; 0.8%</span>
        <div class="sparkline">
          <div class="spark-bar" style="height:80%"></div><div class="spark-bar" style="height:75%"></div>
          <div class="spark-bar" style="height:78%"></div><div class="spark-bar" style="height:72%"></div>
          <div class="spark-bar" style="height:70%"></div><div class="spark-bar" style="height:68%"></div>
          <div class="spark-bar" style="height:65%"></div><div class="spark-bar" style="height:63%"></div>
          <div class="spark-bar" style="height:60%"></div><div class="spark-bar" style="height:62%"></div>
          <div class="spark-bar" style="height:58%"></div><div class="spark-bar" style="height:55%"></div>
        </div>
      </div>
      <div class="metric-card amber">
        <div class="metric-header">
          <span class="metric-label">Avg. Order Value</span>
          <div class="metric-icon">&#x1F6D2;</div>
        </div>
        <div class="metric-value">$68.50</div>
        <span class="trend up">&#x2191; 4.1%</span>
        <div class="sparkline">
          <div class="spark-bar" style="height:50%"></div><div class="spark-bar" style="height:55%"></div>
          <div class="spark-bar" style="height:52%"></div><div class="spark-bar" style="height:60%"></div>
          <div class="spark-bar" style="height:58%"></div><div class="spark-bar" style="height:65%"></div>
          <div class="spark-bar" style="height:62%"></div><div class="spark-bar" style="height:68%"></div>
          <div class="spark-bar" style="height:70%"></div><div class="spark-bar" style="height:72%"></div>
          <div class="spark-bar" style="height:75%"></div><div class="spark-bar" style="height:78%"></div>
        </div>
      </div>
      <div class="metric-card rose">
        <div class="metric-header">
          <span class="metric-label">Churn Rate</span>
          <div class="metric-icon">&#x1F6AA;</div>
        </div>
        <div class="metric-value">2.1%</div>
        <span class="trend down">&#x2193; 0.3%</span>
        <div class="sparkline">
          <div class="spark-bar" style="height:70%"></div><div class="spark-bar" style="height:65%"></div>
          <div class="spark-bar" style="height:68%"></div><div class="spark-bar" style="height:60%"></div>
          <div class="spark-bar" style="height:55%"></div><div class="spark-bar" style="height:50%"></div>
          <div class="spark-bar" style="height:52%"></div><div class="spark-bar" style="height:48%"></div>
          <div class="spark-bar" style="height:45%"></div><div class="spark-bar" style="height:42%"></div>
          <div class="spark-bar" style="height:40%"></div><div class="spark-bar" style="height:38%"></div>
        </div>
      </div>
      <div class="metric-card cyan">
        <div class="metric-header">
          <span class="metric-label">API Requests</span>
          <div class="metric-icon">&#x26A1;</div>
        </div>
        <div class="metric-value">1.2M</div>
        <span class="trend flat">&#x2192; 0.1%</span>
        <div class="sparkline">
          <div class="spark-bar" style="height:70%"></div><div class="spark-bar" style="height:72%"></div>
          <div class="spark-bar" style="height:68%"></div><div class="spark-bar" style="height:74%"></div>
          <div class="spark-bar" style="height:70%"></div><div class="spark-bar" style="height:71%"></div>
          <div class="spark-bar" style="height:73%"></div><div class="spark-bar" style="height:69%"></div>
          <div class="spark-bar" style="height:72%"></div><div class="spark-bar" style="height:70%"></div>
          <div class="spark-bar" style="height:71%"></div><div class="spark-bar" style="height:72%"></div>
        </div>
      </div>
    </div>
    <div class="period">Last updated: Apr 10, 2026 at 14:32 UTC</div>
  </div>
</body>
</html>"##;

#[no_mangle]
pub extern "C" fn x402_handle() {
    host_log(1, "metrics_display: serving metrics");
    respond(200, BODY, "text/html; charset=utf-8");
}
