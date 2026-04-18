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
<title>Under Maintenance</title>
<style>
  *, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: 'Segoe UI', system-ui, sans-serif;
    background: #fefce8; color: #1c1917;
    min-height: 100vh; display: flex; align-items: center; justify-content: center;
    position: relative; overflow: hidden;
  }
  .gear {
    position: absolute; opacity: 0.04; font-size: 300px;
    animation: spin 20s linear infinite;
  }
  .gear-1 { top: -100px; right: -80px; }
  .gear-2 { bottom: -120px; left: -100px; animation-direction: reverse; animation-duration: 25s; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
  .container { text-align: center; padding: 40px 24px; position: relative; z-index: 1; max-width: 560px; }
  .icon {
    width: 100px; height: 100px; margin: 0 auto 32px;
    background: linear-gradient(135deg, #f59e0b, #d97706);
    border-radius: 24px; display: flex; align-items: center; justify-content: center;
    font-size: 48px; box-shadow: 0 8px 24px rgba(245, 158, 11, 0.3);
    animation: bob 3s ease-in-out infinite;
  }
  @keyframes bob { 0%,100% { transform: translateY(0); } 50% { transform: translateY(-8px); } }
  h1 { font-size: 2.25rem; font-weight: 800; margin-bottom: 12px; color: #292524; }
  .subtitle { font-size: 1.1rem; color: #78716c; line-height: 1.6; margin-bottom: 40px; }
  .countdown {
    display: flex; gap: 16px; justify-content: center; margin-bottom: 40px;
  }
  .countdown-item {
    background: #fff; border: 2px solid #fde68a; border-radius: 16px;
    padding: 20px 16px; min-width: 80px;
    box-shadow: 0 2px 12px rgba(0,0,0,0.04);
  }
  .countdown-value {
    font-size: 2.5rem; font-weight: 800; color: #d97706;
    font-variant-numeric: tabular-nums;
  }
  .countdown-label {
    font-size: 0.72rem; text-transform: uppercase; letter-spacing: 1.5px;
    color: #a8a29e; font-weight: 600; margin-top: 4px;
  }
  .progress-wrap {
    background: #fef3c7; border-radius: 999px; height: 8px;
    overflow: hidden; margin-bottom: 12px; max-width: 360px; margin-left: auto; margin-right: auto;
  }
  .progress-bar {
    height: 100%; width: 65%; border-radius: 999px;
    background: linear-gradient(90deg, #f59e0b, #d97706);
    animation: progress-pulse 2s ease-in-out infinite;
  }
  @keyframes progress-pulse { 0%,100% { opacity: 1; } 50% { opacity: 0.6; } }
  .progress-text { font-size: 0.85rem; color: #a8a29e; margin-bottom: 40px; }
  .checklist {
    text-align: left; max-width: 340px; margin: 0 auto 40px;
    list-style: none;
  }
  .checklist li {
    padding: 10px 0; border-bottom: 1px solid #fef3c7;
    display: flex; align-items: center; gap: 12px;
    font-size: 0.95rem; color: #57534e;
  }
  .checklist .done { color: #16a34a; }
  .checklist .pending { color: #d97706; animation: blink 1.5s ease-in-out infinite; }
  .checklist .waiting { color: #d6d3d1; }
  @keyframes blink { 0%,100% { opacity: 1; } 50% { opacity: 0.4; } }
  .notify {
    display: inline-flex; align-items: center; gap: 8px;
    background: #fff; border: 2px solid #fde68a; border-radius: 12px;
    padding: 14px 28px; font-size: 0.95rem; font-weight: 600;
    color: #92400e; cursor: pointer; transition: all 0.2s;
  }
  .notify:hover { background: #fffbeb; border-color: #f59e0b; transform: translateY(-2px); }
</style>
</head>
<body>
  <div class="gear gear-1">&#x2699;</div>
  <div class="gear gear-2">&#x2699;</div>
  <div class="container">
    <div class="icon">&#x1F527;</div>
    <h1>Under Maintenance</h1>
    <p class="subtitle">We are performing scheduled maintenance to improve your experience. We will be back shortly.</p>
    <div class="countdown">
      <div class="countdown-item">
        <div class="countdown-value">02</div>
        <div class="countdown-label">Hours</div>
      </div>
      <div class="countdown-item">
        <div class="countdown-value">34</div>
        <div class="countdown-label">Minutes</div>
      </div>
      <div class="countdown-item">
        <div class="countdown-value">17</div>
        <div class="countdown-label">Seconds</div>
      </div>
    </div>
    <div class="progress-wrap"><div class="progress-bar"></div></div>
    <p class="progress-text">Estimated 65% complete</p>
    <ul class="checklist">
      <li><span class="done">&#x2714;</span> Database migration</li>
      <li><span class="done">&#x2714;</span> Schema validation</li>
      <li><span class="pending">&#x25CF;</span> Service deployment</li>
      <li><span class="waiting">&#x25CB;</span> Health checks</li>
      <li><span class="waiting">&#x25CB;</span> Cache warm-up</li>
    </ul>
    <button class="notify">&#x1F514; Notify me when ready</button>
  </div>
</body>
</html>"##;

#[no_mangle]
pub extern "C" fn x402_handle() {
    host_log(2, "maintenance_page: serving maintenance page");
    respond(503, BODY, "text/html; charset=utf-8");
}
