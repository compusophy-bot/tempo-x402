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

const PAGE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>Notifications</title>
<style>
*{margin:0;padding:0;box-sizing:border-box}
body{background:##0f0f23;color:##c9d1d9;font-family:-apple-system,sans-serif;padding:24px}
h1{font-size:1.5rem;margin-bottom:16px;color:##e6edf3}
.badge{display:inline-block;background:##da3633;color:white;border-radius:10px;padding:2px 8px;font-size:0.75rem;margin-left:8px}
.notif{border:1px solid ##30363d;border-radius:8px;padding:16px;margin-bottom:8px;display:flex;gap:12px;align-items:flex-start}
.notif.info{border-left:3px solid ##58a6ff}
.notif.warn{border-left:3px solid ##d29922}
.notif.error{border-left:3px solid ##f85149}
.notif.success{border-left:3px solid ##3fb950}
.icon{font-size:1.2rem;flex-shrink:0}
.body{flex:1}
.title{font-weight:600;margin-bottom:4px;color:##e6edf3}
.desc{font-size:0.9rem;color:##8b949e}
.time{font-size:0.75rem;color:##484f58;margin-top:4px}
.actions{display:flex;gap:8px;margin-top:8px}
.btn{padding:4px 12px;border-radius:6px;border:1px solid ##30363d;background:##21262d;color:##c9d1d9;cursor:pointer;font-size:0.8rem}
.btn:hover{background:##30363d}
.btn.primary{background:##238636;border-color:##238636;color:white}
</style>
</head>
<body>
<h1>Notifications <span class="badge">5</span></h1>
<div class="notif success">
  <div class="icon">&#10003;</div>
  <div class="body">
    <div class="title">Deployment successful</div>
    <div class="desc">Service borg-0 deployed v9.2.0 to production</div>
    <div class="time">2 minutes ago</div>
  </div>
</div>
<div class="notif info">
  <div class="icon">&#8505;</div>
  <div class="body">
    <div class="title">New colony member</div>
    <div class="desc">borg-0-3 has joined the colony and synced brain weights</div>
    <div class="time">15 minutes ago</div>
    <div class="actions"><button class="btn primary">View</button><button class="btn">Dismiss</button></div>
  </div>
</div>
<div class="notif warn">
  <div class="icon">&#9888;</div>
  <div class="body">
    <div class="title">High memory usage</div>
    <div class="desc">borg-0-2 memory at 89% — consider pruning sled database</div>
    <div class="time">1 hour ago</div>
    <div class="actions"><button class="btn primary">Investigate</button><button class="btn">Snooze</button></div>
  </div>
</div>
<div class="notif error">
  <div class="icon">&#10007;</div>
  <div class="body">
    <div class="title">Benchmark regression</div>
    <div class="desc">IQ dropped from 127 to 121 after last commit. Review changes.</div>
    <div class="time">3 hours ago</div>
    <div class="actions"><button class="btn primary">Review</button></div>
  </div>
</div>
<div class="notif info">
  <div class="icon">&#128276;</div>
  <div class="body">
    <div class="title">Scheduled maintenance</div>
    <div class="desc">Railway platform maintenance window: Sunday 02:00-04:00 UTC</div>
    <div class="time">Yesterday</div>
  </div>
</div>
</body>
</html>"##;

#[no_mangle]
pub extern "C" fn x402_handle(request_ptr: *const u8, request_len: i32) {
    host_log(1, "notification-center cartridge invoked");
    respond(200, PAGE, "text/html");
}

static mut SCRATCH: [u8; 131072] = [0u8; 131072];

#[no_mangle]
pub extern "C" fn x402_alloc(size: i32) -> *mut u8 {
    unsafe { SCRATCH.as_mut_ptr() }
}
