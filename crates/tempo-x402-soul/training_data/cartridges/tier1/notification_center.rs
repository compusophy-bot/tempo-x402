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
<title>Notifications</title>
<style>
  *, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: 'Segoe UI', system-ui, sans-serif; background: #f1f5f9; color: #1e293b; padding: 40px 20px; display: flex; justify-content: center; }
  .panel { width: 100%; max-width: 480px; background: #fff; border-radius: 16px; box-shadow: 0 4px 24px rgba(0,0,0,0.08); overflow: hidden; }
  .panel-header { padding: 20px 24px; display: flex; justify-content: space-between; align-items: center; border-bottom: 1px solid #f1f5f9; }
  .panel-header h2 { font-size: 1.15rem; font-weight: 700; }
  .badge { background: #ef4444; color: #fff; font-size: 0.72rem; font-weight: 700; padding: 2px 8px; border-radius: 999px; margin-left: 8px; }
  .mark-all { font-size: 0.82rem; color: #3b82f6; font-weight: 600; background: none; border: none; cursor: pointer; }
  .mark-all:hover { text-decoration: underline; }
  .tabs { display: flex; border-bottom: 1px solid #f1f5f9; }
  .tab { flex: 1; padding: 12px; text-align: center; font-size: 0.82rem; font-weight: 600; color: #94a3b8; cursor: pointer; border: none; background: none; border-bottom: 2px solid transparent; transition: all 0.2s; }
  .tab.active { color: #3b82f6; border-bottom-color: #3b82f6; }
  .tab:hover { color: #64748b; }
  .notifications { max-height: 520px; overflow-y: auto; }
  .notif { display: flex; gap: 14px; padding: 16px 24px; border-bottom: 1px solid #f8fafc; transition: background 0.15s; cursor: pointer; position: relative; }
  .notif:hover { background: #f8fafc; }
  .notif.unread { background: #f0f7ff; }
  .notif.unread::before { content: ''; position: absolute; left: 10px; top: 50%; transform: translateY(-50%); width: 6px; height: 6px; border-radius: 50%; background: #3b82f6; }
  .notif-icon { width: 40px; height: 40px; border-radius: 12px; display: flex; align-items: center; justify-content: center; font-size: 1.1rem; flex-shrink: 0; }
  .notif-icon.info { background: #dbeafe; }
  .notif-icon.warn { background: #fef3c7; }
  .notif-icon.error { background: #fee2e2; }
  .notif-icon.success { background: #dcfce7; }
  .notif-icon.system { background: #f1f5f9; }
  .notif-body { flex: 1; min-width: 0; }
  .notif-title { font-size: 0.88rem; font-weight: 600; margin-bottom: 2px; }
  .notif-text { font-size: 0.82rem; color: #64748b; line-height: 1.4; }
  .notif-time { font-size: 0.72rem; color: #94a3b8; margin-top: 4px; }
  .notif-dismiss { align-self: flex-start; background: none; border: none; color: #cbd5e1; font-size: 1rem; cursor: pointer; padding: 4px; border-radius: 4px; }
  .notif-dismiss:hover { color: #94a3b8; }
  .panel-footer { padding: 16px 24px; text-align: center; border-top: 1px solid #f1f5f9; }
  .view-all { font-size: 0.85rem; color: #3b82f6; font-weight: 600; background: none; border: none; cursor: pointer; }
  .view-all:hover { text-decoration: underline; }
</style>
</head>
<body>
  <div class="panel">
    <div class="panel-header">
      <h2>Notifications <span class="badge">5</span></h2>
      <button class="mark-all">Mark all read</button>
    </div>
    <div class="tabs">
      <button class="tab active">All</button>
      <button class="tab">Unread</button>
      <button class="tab">Mentions</button>
    </div>
    <div class="notifications">
      <div class="notif unread">
        <div class="notif-icon error">&#x26A0;</div>
        <div class="notif-body">
          <div class="notif-title">Deployment Failed</div>
          <div class="notif-text">Build 847 failed on staging: cargo build exited with code 101. Missing dependency in Cargo.toml.</div>
          <div class="notif-time">2 minutes ago</div>
        </div>
        <button class="notif-dismiss">&times;</button>
      </div>
      <div class="notif unread">
        <div class="notif-icon warn">&#x1F514;</div>
        <div class="notif-body">
          <div class="notif-title">High Memory Usage</div>
          <div class="notif-text">Node borg-0 is at 87% memory utilization. Consider scaling up or optimizing queries.</div>
          <div class="notif-time">15 minutes ago</div>
        </div>
        <button class="notif-dismiss">&times;</button>
      </div>
      <div class="notif unread">
        <div class="notif-icon info">&#x1F4AC;</div>
        <div class="notif-body">
          <div class="notif-title">New Comment on PR 142</div>
          <div class="notif-text">@alex: Looks good, but extract the retry logic into a shared util module.</div>
          <div class="notif-time">32 minutes ago</div>
        </div>
        <button class="notif-dismiss">&times;</button>
      </div>
      <div class="notif unread">
        <div class="notif-icon success">&#x2713;</div>
        <div class="notif-body">
          <div class="notif-title">Benchmark Complete</div>
          <div class="notif-text">IQ delta: +3 points. 47/201 problems solved. New best on regex-engine tier.</div>
          <div class="notif-time">1 hour ago</div>
        </div>
        <button class="notif-dismiss">&times;</button>
      </div>
      <div class="notif unread">
        <div class="notif-icon system">&#x2699;</div>
        <div class="notif-body">
          <div class="notif-title">Scheduled Maintenance</div>
          <div class="notif-text">Database maintenance window: Apr 12, 02:00-04:00 UTC. Expect brief read-only periods.</div>
          <div class="notif-time">2 hours ago</div>
        </div>
        <button class="notif-dismiss">&times;</button>
      </div>
      <div class="notif">
        <div class="notif-icon info">&#x1F517;</div>
        <div class="notif-body">
          <div class="notif-title">Clone Registered</div>
          <div class="notif-text">borg-0-3 has joined the colony. Stem cell differentiation initiated.</div>
          <div class="notif-time">5 hours ago</div>
        </div>
        <button class="notif-dismiss">&times;</button>
      </div>
      <div class="notif">
        <div class="notif-icon success">&#x1F389;</div>
        <div class="notif-body">
          <div class="notif-title">v9.1.7 Published</div>
          <div class="notif-text">All 7 crates published to crates.io. Workers compute own ELO from benchmarks.</div>
          <div class="notif-time">Yesterday</div>
        </div>
        <button class="notif-dismiss">&times;</button>
      </div>
    </div>
    <div class="panel-footer">
      <button class="view-all">View all notifications</button>
    </div>
  </div>
</body>
</html>"##;

#[no_mangle]
pub extern "C" fn x402_handle() {
    host_log(1, "notification_center: serving notifications");
    respond(200, BODY, "text/html; charset=utf-8");
}
