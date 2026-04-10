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
<title>Stats Dashboard</title>
<style>
  *, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }
  :root { --bg: #0f172a; --surface: #1e293b; --border: #334155; --text: #e2e8f0; --muted: #94a3b8; }
  body { font-family: 'Segoe UI', system-ui, sans-serif; background: var(--bg); color: var(--text); padding: 40px 20px; }
  .container { max-width: 1100px; margin: 0 auto; }
  .header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 36px; }
  .header h1 { font-size: 1.5rem; font-weight: 700; }
  .period-select {
    display: flex; gap: 4px; background: var(--surface); border-radius: 10px; padding: 4px;
    border: 1px solid var(--border);
  }
  .period-btn {
    padding: 6px 16px; border-radius: 8px; border: none;
    font-size: 0.82rem; font-weight: 600; cursor: pointer;
    background: transparent; color: var(--muted); transition: all 0.2s;
  }
  .period-btn.active { background: #3b82f6; color: #fff; }
  .kpi-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 20px; margin-bottom: 32px; }
  .kpi-card {
    background: var(--surface); border: 1px solid var(--border);
    border-radius: 16px; padding: 24px; position: relative; overflow: hidden;
  }
  .kpi-card::after {
    content: ''; position: absolute; bottom: 0; left: 0; right: 0; height: 3px;
  }
  .kpi-card.blue::after { background: #3b82f6; }
  .kpi-card.green::after { background: #22c55e; }
  .kpi-card.purple::after { background: #8b5cf6; }
  .kpi-card.amber::after { background: #f59e0b; }
  .kpi-label { font-size: 0.78rem; color: var(--muted); font-weight: 600; text-transform: uppercase; letter-spacing: 1px; margin-bottom: 12px; }
  .kpi-row { display: flex; justify-content: space-between; align-items: flex-end; }
  .kpi-value { font-size: 2.25rem; font-weight: 800; line-height: 1; font-variant-numeric: tabular-nums; }
  .kpi-change { font-size: 0.78rem; font-weight: 600; padding: 3px 8px; border-radius: 6px; }
  .kpi-change.up { background: rgba(34,197,94,0.12); color: #4ade80; }
  .kpi-change.down { background: rgba(244,63,94,0.12); color: #fb7185; }
  .kpi-sub { font-size: 0.78rem; color: #475569; margin-top: 12px; }
  .mini-chart { display: flex; align-items: flex-end; gap: 2px; height: 48px; margin-top: 16px; }
  .bar {
    flex: 1; border-radius: 2px 2px 0 0; min-width: 3px;
    transition: height 0.3s;
  }
  .blue .bar { background: rgba(59,130,246,0.5); }
  .blue .bar:last-child { background: #3b82f6; }
  .green .bar { background: rgba(34,197,94,0.5); }
  .green .bar:last-child { background: #22c55e; }
  .purple .bar { background: rgba(139,92,246,0.5); }
  .purple .bar:last-child { background: #8b5cf6; }
  .amber .bar { background: rgba(245,158,11,0.5); }
  .amber .bar:last-child { background: #f59e0b; }
  .secondary-grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 20px; }
  .detail-card {
    background: var(--surface); border: 1px solid var(--border);
    border-radius: 16px; padding: 24px;
  }
  .detail-card h3 { font-size: 0.92rem; font-weight: 700; margin-bottom: 20px; }
  .detail-row {
    display: flex; justify-content: space-between; align-items: center;
    padding: 10px 0; border-bottom: 1px solid rgba(255,255,255,0.04);
  }
  .detail-row:last-child { border-bottom: none; }
  .detail-label { font-size: 0.85rem; color: var(--muted); }
  .detail-value { font-size: 0.85rem; font-weight: 700; font-variant-numeric: tabular-nums; }
  .progress-bar-wrap { width: 80px; height: 6px; background: #334155; border-radius: 3px; overflow: hidden; margin-left: 12px; }
  .progress-fill { height: 100%; border-radius: 3px; }
  .progress-fill.high { background: #22c55e; }
  .progress-fill.mid { background: #f59e0b; }
  .progress-fill.low { background: #ef4444; }
  .detail-inline { display: flex; align-items: center; gap: 4px; }
</style>
</head>
<body>
  <div class="container">
    <div class="header">
      <h1>Dashboard</h1>
      <div class="period-select">
        <button class="period-btn">24h</button>
        <button class="period-btn active">7d</button>
        <button class="period-btn">30d</button>
        <button class="period-btn">90d</button>
      </div>
    </div>
    <div class="kpi-grid">
      <div class="kpi-card blue">
        <div class="kpi-label">Revenue</div>
        <div class="kpi-row">
          <div class="kpi-value">$48.2K</div>
          <span class="kpi-change up">+14.2%</span>
        </div>
        <div class="kpi-sub">vs $42.2K last period</div>
        <div class="mini-chart">
          <div class="bar" style="height:35%"></div><div class="bar" style="height:42%"></div>
          <div class="bar" style="height:38%"></div><div class="bar" style="height:55%"></div>
          <div class="bar" style="height:48%"></div><div class="bar" style="height:62%"></div>
          <div class="bar" style="height:58%"></div><div class="bar" style="height:70%"></div>
          <div class="bar" style="height:65%"></div><div class="bar" style="height:75%"></div>
          <div class="bar" style="height:80%"></div><div class="bar" style="height:85%"></div>
          <div class="bar" style="height:78%"></div><div class="bar" style="height:90%"></div>
        </div>
      </div>
      <div class="kpi-card green">
        <div class="kpi-label">Orders</div>
        <div class="kpi-row">
          <div class="kpi-value">1,284</div>
          <span class="kpi-change up">+8.7%</span>
        </div>
        <div class="kpi-sub">183/day average</div>
        <div class="mini-chart">
          <div class="bar" style="height:60%"></div><div class="bar" style="height:55%"></div>
          <div class="bar" style="height:68%"></div><div class="bar" style="height:72%"></div>
          <div class="bar" style="height:65%"></div><div class="bar" style="height:70%"></div>
          <div class="bar" style="height:78%"></div><div class="bar" style="height:74%"></div>
          <div class="bar" style="height:80%"></div><div class="bar" style="height:76%"></div>
          <div class="bar" style="height:82%"></div><div class="bar" style="height:88%"></div>
          <div class="bar" style="height:84%"></div><div class="bar" style="height:92%"></div>
        </div>
      </div>
      <div class="kpi-card purple">
        <div class="kpi-label">Visitors</div>
        <div class="kpi-row">
          <div class="kpi-value">32.4K</div>
          <span class="kpi-change down">-2.1%</span>
        </div>
        <div class="kpi-sub">4,628/day average</div>
        <div class="mini-chart">
          <div class="bar" style="height:80%"></div><div class="bar" style="height:75%"></div>
          <div class="bar" style="height:78%"></div><div class="bar" style="height:70%"></div>
          <div class="bar" style="height:72%"></div><div class="bar" style="height:68%"></div>
          <div class="bar" style="height:65%"></div><div class="bar" style="height:70%"></div>
          <div class="bar" style="height:66%"></div><div class="bar" style="height:63%"></div>
          <div class="bar" style="height:68%"></div><div class="bar" style="height:60%"></div>
          <div class="bar" style="height:62%"></div><div class="bar" style="height:58%"></div>
        </div>
      </div>
      <div class="kpi-card amber">
        <div class="kpi-label">Avg. Session</div>
        <div class="kpi-row">
          <div class="kpi-value">4m 32s</div>
          <span class="kpi-change up">+18s</span>
        </div>
        <div class="kpi-sub">Bounce rate: 34%</div>
        <div class="mini-chart">
          <div class="bar" style="height:50%"></div><div class="bar" style="height:55%"></div>
          <div class="bar" style="height:52%"></div><div class="bar" style="height:58%"></div>
          <div class="bar" style="height:60%"></div><div class="bar" style="height:62%"></div>
          <div class="bar" style="height:65%"></div><div class="bar" style="height:68%"></div>
          <div class="bar" style="height:64%"></div><div class="bar" style="height:70%"></div>
          <div class="bar" style="height:72%"></div><div class="bar" style="height:74%"></div>
          <div class="bar" style="height:76%"></div><div class="bar" style="height:80%"></div>
        </div>
      </div>
    </div>
    <div class="secondary-grid">
      <div class="detail-card">
        <h3>Top Channels</h3>
        <div class="detail-row">
          <span class="detail-label">Organic Search</span>
          <div class="detail-inline"><span class="detail-value">42%</span><div class="progress-bar-wrap"><div class="progress-fill high" style="width:42%"></div></div></div>
        </div>
        <div class="detail-row">
          <span class="detail-label">Direct</span>
          <div class="detail-inline"><span class="detail-value">28%</span><div class="progress-bar-wrap"><div class="progress-fill high" style="width:28%"></div></div></div>
        </div>
        <div class="detail-row">
          <span class="detail-label">Social Media</span>
          <div class="detail-inline"><span class="detail-value">18%</span><div class="progress-bar-wrap"><div class="progress-fill mid" style="width:18%"></div></div></div>
        </div>
        <div class="detail-row">
          <span class="detail-label">Email</span>
          <div class="detail-inline"><span class="detail-value">8%</span><div class="progress-bar-wrap"><div class="progress-fill low" style="width:8%"></div></div></div>
        </div>
        <div class="detail-row">
          <span class="detail-label">Referral</span>
          <div class="detail-inline"><span class="detail-value">4%</span><div class="progress-bar-wrap"><div class="progress-fill low" style="width:4%"></div></div></div>
        </div>
      </div>
      <div class="detail-card">
        <h3>System Health</h3>
        <div class="detail-row">
          <span class="detail-label">API Uptime</span>
          <span class="detail-value" style="color:#4ade80">99.97%</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Avg Response</span>
          <span class="detail-value">42ms</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Error Rate</span>
          <span class="detail-value" style="color:#4ade80">0.03%</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">CPU Usage</span>
          <span class="detail-value" style="color:#fbbf24">67%</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Memory</span>
          <span class="detail-value">4.2 / 8 GB</span>
        </div>
      </div>
    </div>
  </div>
</body>
</html>"##;

#[no_mangle]
pub extern "C" fn x402_handle() {
    host_log(1, "stats_card: serving stats dashboard");
    respond(200, BODY, "text/html; charset=utf-8");
}
