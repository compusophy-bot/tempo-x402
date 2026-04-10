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
<title>Loading...</title>
<style>
  *, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }
  :root { --bg: #0f172a; --surface: #1e293b; --shine: #334155; }
  body { font-family: 'Segoe UI', system-ui, sans-serif; background: var(--bg); padding: 40px 20px; }
  .container { max-width: 1000px; margin: 0 auto; }
  @keyframes pulse {
    0% { opacity: 0.4; }
    50% { opacity: 1; }
    100% { opacity: 0.4; }
  }
  @keyframes shimmer {
    0% { background-position: -400px 0; }
    100% { background-position: 400px 0; }
  }
  .skeleton {
    background: var(--surface);
    border-radius: 8px;
    position: relative;
    overflow: hidden;
  }
  .skeleton::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, transparent, rgba(255,255,255,0.04), transparent);
    background-size: 400px 100%;
    animation: shimmer 1.8s ease-in-out infinite;
  }
  .top-bar {
    display: flex; justify-content: space-between; align-items: center;
    padding: 20px 0; margin-bottom: 32px;
    border-bottom: 1px solid #1e293b;
  }
  .logo-sk { width: 140px; height: 32px; border-radius: 8px; }
  .nav-sk { display: flex; gap: 16px; }
  .nav-item-sk { width: 72px; height: 20px; border-radius: 4px; }
  .avatar-sk { width: 36px; height: 36px; border-radius: 50%; }
  .page-title-sk { width: 280px; height: 36px; margin-bottom: 8px; border-radius: 8px; }
  .subtitle-sk { width: 400px; height: 18px; margin-bottom: 32px; border-radius: 4px; }
  .stats-row { display: grid; grid-template-columns: repeat(4, 1fr); gap: 20px; margin-bottom: 32px; }
  .stat-card-sk {
    height: 120px; border-radius: 16px; padding: 24px;
    display: flex; flex-direction: column; justify-content: space-between;
  }
  .stat-label-sk { width: 80px; height: 14px; border-radius: 4px; background: var(--shine); animation: pulse 1.8s ease-in-out infinite; }
  .stat-value-sk { width: 120px; height: 32px; border-radius: 6px; background: var(--shine); animation: pulse 1.8s ease-in-out infinite 0.2s; }
  .stat-trend-sk { width: 60px; height: 16px; border-radius: 4px; background: var(--shine); animation: pulse 1.8s ease-in-out infinite 0.4s; }
  .content-grid { display: grid; grid-template-columns: 2fr 1fr; gap: 24px; }
  .chart-sk { height: 320px; border-radius: 16px; margin-bottom: 24px; }
  .table-sk { border-radius: 16px; overflow: hidden; }
  .table-header-sk { height: 48px; background: var(--shine); margin-bottom: 2px; }
  .table-row-sk { height: 56px; margin-bottom: 2px; animation: pulse 1.8s ease-in-out infinite; }
  .table-row-sk:nth-child(2) { animation-delay: 0.1s; }
  .table-row-sk:nth-child(3) { animation-delay: 0.2s; }
  .table-row-sk:nth-child(4) { animation-delay: 0.3s; }
  .table-row-sk:nth-child(5) { animation-delay: 0.4s; }
  .table-row-sk:nth-child(6) { animation-delay: 0.5s; }
  .sidebar-sk { display: flex; flex-direction: column; gap: 20px; }
  .sidebar-card-sk { height: 160px; border-radius: 16px; }
  .sidebar-list-sk { border-radius: 16px; padding: 20px; }
  .list-item-sk { height: 20px; border-radius: 4px; background: var(--shine); margin-bottom: 12px; animation: pulse 1.8s ease-in-out infinite; }
  .list-item-sk:nth-child(2) { width: 85%; animation-delay: 0.15s; }
  .list-item-sk:nth-child(3) { width: 70%; animation-delay: 0.3s; }
  .list-item-sk:nth-child(4) { width: 90%; animation-delay: 0.45s; }
  .list-item-sk:nth-child(5) { width: 60%; animation-delay: 0.6s; }
  .loading-text {
    text-align: center; margin-top: 40px;
    font-size: 0.85rem; color: #475569;
    animation: pulse 2s ease-in-out infinite;
  }
</style>
</head>
<body>
  <div class="container">
    <div class="top-bar">
      <div class="skeleton logo-sk"></div>
      <div class="nav-sk">
        <div class="skeleton nav-item-sk"></div>
        <div class="skeleton nav-item-sk"></div>
        <div class="skeleton nav-item-sk"></div>
      </div>
      <div class="skeleton avatar-sk"></div>
    </div>
    <div class="skeleton page-title-sk"></div>
    <div class="skeleton subtitle-sk"></div>
    <div class="stats-row">
      <div class="skeleton stat-card-sk">
        <div class="stat-label-sk"></div>
        <div class="stat-value-sk"></div>
        <div class="stat-trend-sk"></div>
      </div>
      <div class="skeleton stat-card-sk">
        <div class="stat-label-sk"></div>
        <div class="stat-value-sk"></div>
        <div class="stat-trend-sk"></div>
      </div>
      <div class="skeleton stat-card-sk">
        <div class="stat-label-sk"></div>
        <div class="stat-value-sk"></div>
        <div class="stat-trend-sk"></div>
      </div>
      <div class="skeleton stat-card-sk">
        <div class="stat-label-sk"></div>
        <div class="stat-value-sk"></div>
        <div class="stat-trend-sk"></div>
      </div>
    </div>
    <div class="content-grid">
      <div>
        <div class="skeleton chart-sk"></div>
        <div class="skeleton table-sk">
          <div class="table-header-sk"></div>
          <div class="skeleton table-row-sk"></div>
          <div class="skeleton table-row-sk"></div>
          <div class="skeleton table-row-sk"></div>
          <div class="skeleton table-row-sk"></div>
          <div class="skeleton table-row-sk"></div>
          <div class="skeleton table-row-sk"></div>
        </div>
      </div>
      <div class="sidebar-sk">
        <div class="skeleton sidebar-card-sk"></div>
        <div class="skeleton sidebar-list-sk">
          <div class="list-item-sk"></div>
          <div class="list-item-sk"></div>
          <div class="list-item-sk"></div>
          <div class="list-item-sk"></div>
          <div class="list-item-sk"></div>
        </div>
      </div>
    </div>
    <div class="loading-text">Loading dashboard data...</div>
  </div>
</body>
</html>"##;

#[no_mangle]
pub extern "C" fn x402_handle() {
    host_log(1, "loading_skeleton: serving loading skeleton");
    respond(200, BODY, "text/html; charset=utf-8");
}
