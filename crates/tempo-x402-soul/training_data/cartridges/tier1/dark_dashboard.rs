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

const PAGE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Dashboard</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; background: #0f172a; color: #e2e8f0; display: flex; min-height: 100vh; }

  /* Sidebar */
  .sidebar { width: 240px; background: #1e293b; border-right: 1px solid #334155; padding: 24px 0; display: flex; flex-direction: column; }
  .logo { padding: 0 20px 24px; font-size: 1.3rem; font-weight: 800; color: #fff; letter-spacing: -0.02em; display: flex; align-items: center; gap: 10px; }
  .logo .dot { width: 10px; height: 10px; background: #10b981; border-radius: 50%; }
  .nav-section { padding: 0 12px; margin-bottom: 24px; }
  .nav-label { font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.1em; color: #64748b; padding: 0 8px 8px; }
  .nav-item { display: flex; align-items: center; gap: 10px; padding: 10px 12px; border-radius: 8px; color: #94a3b8; font-size: 0.9rem; cursor: pointer; transition: all 0.15s; margin-bottom: 2px; }
  .nav-item:hover { background: rgba(99,102,241,0.1); color: #c7d2fe; }
  .nav-item.active { background: #6366f1; color: #fff; }
  .nav-icon { width: 18px; text-align: center; font-size: 0.85rem; }
  .sidebar-footer { margin-top: auto; padding: 16px 20px; border-top: 1px solid #334155; }
  .user-info { display: flex; align-items: center; gap: 10px; }
  .user-avatar { width: 32px; height: 32px; border-radius: 50%; background: linear-gradient(135deg, #6366f1, #818cf8); display: flex; align-items: center; justify-content: center; font-size: 0.8rem; font-weight: 700; color: #fff; }
  .user-name { font-size: 0.85rem; color: #e2e8f0; }
  .user-role { font-size: 0.7rem; color: #64748b; }

  /* Main */
  .main { flex: 1; padding: 32px; overflow-y: auto; }
  .topbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 32px; }
  .topbar h1 { font-size: 1.6rem; font-weight: 700; }
  .topbar .badge { background: #10b981; color: #fff; font-size: 0.75rem; padding: 4px 12px; border-radius: 20px; font-weight: 600; }

  /* Stat cards */
  .stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 20px; margin-bottom: 32px; }
  .stat-card { background: #1e293b; border: 1px solid #334155; border-radius: 14px; padding: 22px; }
  .stat-label { font-size: 0.8rem; color: #64748b; margin-bottom: 8px; text-transform: uppercase; letter-spacing: 0.05em; }
  .stat-value { font-size: 2rem; font-weight: 800; margin-bottom: 6px; }
  .stat-change { font-size: 0.8rem; }
  .stat-up { color: #10b981; }
  .stat-down { color: #ef4444; }
  .sc-purple .stat-value { color: #a78bfa; }
  .sc-blue .stat-value { color: #60a5fa; }
  .sc-green .stat-value { color: #34d399; }
  .sc-amber .stat-value { color: #fbbf24; }

  /* Chart placeholder */
  .chart-row { display: grid; grid-template-columns: 2fr 1fr; gap: 20px; margin-bottom: 32px; }
  .chart-card { background: #1e293b; border: 1px solid #334155; border-radius: 14px; padding: 24px; }
  .chart-title { font-size: 1rem; font-weight: 600; margin-bottom: 16px; }
  .chart-bars { display: flex; align-items: flex-end; gap: 8px; height: 140px; }
  .bar { flex: 1; border-radius: 6px 6px 0 0; min-width: 24px; transition: opacity 0.2s; }
  .bar:hover { opacity: 0.8; }
  .b1 { height: 60%; background: #6366f1; } .b2 { height: 80%; background: #6366f1; }
  .b3 { height: 45%; background: #6366f1; } .b4 { height: 90%; background: #6366f1; }
  .b5 { height: 70%; background: #818cf8; } .b6 { height: 55%; background: #818cf8; }
  .b7 { height: 95%; background: #818cf8; } .b8 { height: 65%; background: #818cf8; }
  .b9 { height: 75%; background: #a78bfa; } .b10 { height: 50%; background: #a78bfa; }
  .b11 { height: 85%; background: #a78bfa; } .b12 { height: 100%; background: #a78bfa; }
  .chart-legend { display: flex; gap: 8px; margin-top: 12px; }
  .chart-legend span { font-size: 0.75rem; color: #64748b; }

  /* Donut placeholder */
  .donut-container { display: flex; align-items: center; justify-content: center; height: 160px; }
  .donut { width: 140px; height: 140px; border-radius: 50%; background: conic-gradient(#6366f1 0% 42%, #10b981 42% 68%, #f59e0b 68% 85%, #64748b 85% 100%); position: relative; }
  .donut::after { content: ''; position: absolute; top: 30px; left: 30px; right: 30px; bottom: 30px; background: #1e293b; border-radius: 50%; }
  .donut-legend { margin-top: 16px; }
  .donut-legend div { display: flex; align-items: center; gap: 8px; font-size: 0.8rem; color: #94a3b8; margin-bottom: 6px; }
  .dot-p { width: 8px; height: 8px; border-radius: 50%; background: #6366f1; }
  .dot-g { width: 8px; height: 8px; border-radius: 50%; background: #10b981; }
  .dot-y { width: 8px; height: 8px; border-radius: 50%; background: #f59e0b; }
  .dot-x { width: 8px; height: 8px; border-radius: 50%; background: #64748b; }

  /* Table */
  .table-card { background: #1e293b; border: 1px solid #334155; border-radius: 14px; padding: 24px; }
  .table-card h3 { font-size: 1rem; font-weight: 600; margin-bottom: 16px; }
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; padding: 12px 14px; font-size: 0.85rem; }
  th { color: #64748b; font-weight: 600; text-transform: uppercase; font-size: 0.7rem; letter-spacing: 0.08em; border-bottom: 1px solid #334155; }
  td { color: #cbd5e1; border-bottom: 1px solid rgba(51,65,85,0.5); }
  tr:last-child td { border-bottom: none; }
  .status-ok { color: #10b981; font-weight: 600; }
  .status-pending { color: #f59e0b; font-weight: 600; }
  .status-error { color: #ef4444; font-weight: 600; }
</style>
</head>
<body>
  <aside class="sidebar">
    <div class="logo"><span class="dot"></span>Nexus</div>
    <div class="nav-section">
      <div class="nav-label">Main</div>
      <div class="nav-item active"><span class="nav-icon">&#9632;</span>Dashboard</div>
      <div class="nav-item"><span class="nav-icon">&#9650;</span>Analytics</div>
      <div class="nav-item"><span class="nav-icon">&#9679;</span>Projects</div>
      <div class="nav-item"><span class="nav-icon">&#9670;</span>Messages</div>
    </div>
    <div class="nav-section">
      <div class="nav-label">Management</div>
      <div class="nav-item"><span class="nav-icon">&#9733;</span>Team</div>
      <div class="nav-item"><span class="nav-icon">&#9881;</span>Settings</div>
      <div class="nav-item"><span class="nav-icon">&#9830;</span>Billing</div>
    </div>
    <div class="sidebar-footer">
      <div class="user-info">
        <div class="user-avatar">AK</div>
        <div><div class="user-name">Alice Kim</div><div class="user-role">Admin</div></div>
      </div>
    </div>
  </aside>

  <main class="main">
    <div class="topbar">
      <h1>Dashboard</h1>
      <span class="badge">All systems operational</span>
    </div>

    <div class="stats">
      <div class="stat-card sc-purple">
        <div class="stat-label">Total Revenue</div>
        <div class="stat-value">$48.2K</div>
        <div class="stat-change stat-up">+12.5% from last month</div>
      </div>
      <div class="stat-card sc-blue">
        <div class="stat-label">Active Users</div>
        <div class="stat-value">2,847</div>
        <div class="stat-change stat-up">+8.1% from last month</div>
      </div>
      <div class="stat-card sc-green">
        <div class="stat-label">Conversion Rate</div>
        <div class="stat-value">3.6%</div>
        <div class="stat-change stat-up">+0.4% from last month</div>
      </div>
      <div class="stat-card sc-amber">
        <div class="stat-label">Avg Response</div>
        <div class="stat-value">142ms</div>
        <div class="stat-change stat-down">+18ms from last month</div>
      </div>
    </div>

    <div class="chart-row">
      <div class="chart-card">
        <div class="chart-title">Monthly Revenue</div>
        <div class="chart-bars">
          <div class="bar b1"></div><div class="bar b2"></div><div class="bar b3"></div>
          <div class="bar b4"></div><div class="bar b5"></div><div class="bar b6"></div>
          <div class="bar b7"></div><div class="bar b8"></div><div class="bar b9"></div>
          <div class="bar b10"></div><div class="bar b11"></div><div class="bar b12"></div>
        </div>
        <div class="chart-legend">
          <span>Jan</span><span>Feb</span><span>Mar</span><span>Apr</span>
          <span>May</span><span>Jun</span><span>Jul</span><span>Aug</span>
          <span>Sep</span><span>Oct</span><span>Nov</span><span>Dec</span>
        </div>
      </div>
      <div class="chart-card">
        <div class="chart-title">Traffic Sources</div>
        <div class="donut-container"><div class="donut"></div></div>
        <div class="donut-legend">
          <div><span class="dot-p"></span>Direct (42%)</div>
          <div><span class="dot-g"></span>Organic (26%)</div>
          <div><span class="dot-y"></span>Referral (17%)</div>
          <div><span class="dot-x"></span>Other (15%)</div>
        </div>
      </div>
    </div>

    <div class="table-card">
      <h3>Recent Transactions</h3>
      <table>
        <tr><th>ID</th><th>Customer</th><th>Amount</th><th>Status</th><th>Date</th></tr>
        <tr><td>#TX-4821</td><td>Acme Corp</td><td>$1,240.00</td><td class="status-ok">Completed</td><td>Apr 10, 2026</td></tr>
        <tr><td>#TX-4820</td><td>Globex Inc</td><td>$890.00</td><td class="status-ok">Completed</td><td>Apr 10, 2026</td></tr>
        <tr><td>#TX-4819</td><td>Initech LLC</td><td>$2,100.00</td><td class="status-pending">Pending</td><td>Apr 9, 2026</td></tr>
        <tr><td>#TX-4818</td><td>Umbrella Co</td><td>$340.00</td><td class="status-ok">Completed</td><td>Apr 9, 2026</td></tr>
        <tr><td>#TX-4817</td><td>Stark Industries</td><td>$5,600.00</td><td class="status-error">Failed</td><td>Apr 8, 2026</td></tr>
        <tr><td>#TX-4816</td><td>Wayne Enterprises</td><td>$780.00</td><td class="status-ok">Completed</td><td>Apr 8, 2026</td></tr>
      </table>
    </div>
  </main>
</body>
</html>"#;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(1, "serving dark dashboard");
    respond(200, PAGE, "text/html; charset=utf-8");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
