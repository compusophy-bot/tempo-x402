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
<title>500 — Internal Server Error</title>
<style>
  *, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    font-family: 'Segoe UI', system-ui, sans-serif;
    background: #0f172a; color: #e2e8f0;
    min-height: 100vh; display: flex; align-items: center; justify-content: center;
    overflow: hidden; position: relative;
  }
  .glitch-bg {
    position: absolute; inset: 0; overflow: hidden; opacity: 0.06;
  }
  .glitch-bg::before, .glitch-bg::after {
    content: '500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500 500';
    position: absolute; top: -20px; left: -20px; right: -20px;
    font-size: 5rem; font-weight: 900; color: #ef4444;
    word-break: break-all; line-height: 1;
    animation: glitch-scroll 20s linear infinite;
  }
  .glitch-bg::after { animation-duration: 15s; animation-direction: reverse; opacity: 0.5; color: #3b82f6; }
  @keyframes glitch-scroll { 0% { transform: translateY(0); } 100% { transform: translateY(-50%); } }
  .container { text-align: center; position: relative; z-index: 1; padding: 40px; }
  .error-code {
    font-size: 8rem; font-weight: 900; line-height: 1;
    background: linear-gradient(135deg, #ef4444, #f97316, #ef4444);
    background-size: 200% 200%;
    -webkit-background-clip: text; -webkit-text-fill-color: transparent;
    background-clip: text;
    animation: gradient-shift 3s ease-in-out infinite;
    position: relative;
  }
  @keyframes gradient-shift { 0%,100% { background-position: 0% 50%; } 50% { background-position: 100% 50%; } }
  .error-code::after {
    content: '500';
    position: absolute; left: 0; top: 0; width: 100%;
    -webkit-background-clip: text; -webkit-text-fill-color: transparent;
    background-clip: text;
    background: linear-gradient(135deg, #3b82f6, #8b5cf6);
    animation: glitch 2s ease-in-out infinite;
    clip-path: inset(0 0 80% 0);
  }
  @keyframes glitch {
    0%, 90%, 100% { clip-path: inset(0 0 80% 0); transform: translate(0); }
    92% { clip-path: inset(20% 0 50% 0); transform: translate(-4px, -2px); }
    94% { clip-path: inset(60% 0 10% 0); transform: translate(4px, 2px); }
    96% { clip-path: inset(40% 0 30% 0); transform: translate(-2px, 1px); }
    98% { clip-path: inset(10% 0 60% 0); transform: translate(2px, -1px); }
  }
  .title { font-size: 1.75rem; font-weight: 700; margin: 24px 0 12px; }
  .message { color: #94a3b8; font-size: 1.05rem; max-width: 440px; margin: 0 auto 36px; line-height: 1.6; }
  .server-info {
    display: inline-flex; align-items: center; gap: 8px;
    background: rgba(239, 68, 68, 0.1); border: 1px solid rgba(239, 68, 68, 0.2);
    padding: 8px 20px; border-radius: 8px; font-family: monospace;
    font-size: 0.85rem; color: #f87171; margin-bottom: 32px;
  }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: #ef4444; animation: pulse 1.5s ease-in-out infinite; }
  @keyframes pulse { 0%,100% { opacity: 1; } 50% { opacity: 0.3; } }
  .actions { display: flex; gap: 16px; justify-content: center; flex-wrap: wrap; }
  .btn {
    padding: 12px 28px; border-radius: 10px; font-size: 0.95rem;
    font-weight: 600; cursor: pointer; border: none; transition: all 0.2s;
    text-decoration: none;
  }
  .btn-primary { background: #ef4444; color: #fff; }
  .btn-primary:hover { background: #dc2626; transform: translateY(-2px); }
  .btn-secondary { background: #1e293b; color: #e2e8f0; border: 1px solid #334155; }
  .btn-secondary:hover { border-color: #ef4444; }
  .stack-trace {
    margin-top: 48px; text-align: left; max-width: 520px; margin-left: auto; margin-right: auto;
    background: #1e293b; border: 1px solid #334155; border-radius: 12px; padding: 20px;
    font-family: 'Cascadia Code', 'Fira Code', monospace; font-size: 0.8rem;
    color: #64748b; line-height: 1.8; overflow-x: auto;
  }
  .stack-trace .err { color: #f87171; }
  .stack-trace .fn { color: #38bdf8; }
  .stack-trace .loc { color: #475569; }
</style>
</head>
<body>
  <div class="glitch-bg"></div>
  <div class="container">
    <div class="error-code">500</div>
    <h1 class="title">Internal Server Error</h1>
    <p class="message">Something went wrong on our end. Our engineering team has been notified and is working to resolve the issue.</p>
    <div class="server-info"><span class="dot"></span> Server encountered an unexpected condition</div>
    <div class="actions">
      <a href="/" class="btn btn-primary">Go Home</a>
      <a href="javascript:location.reload()" class="btn btn-secondary">Try Again</a>
    </div>
    <div class="stack-trace">
      <span class="err">Error: INTERNAL_SERVER_ERROR</span><br>
      &nbsp;&nbsp;at <span class="fn">Handler.process</span> <span class="loc">(server/handler.rs:142)</span><br>
      &nbsp;&nbsp;at <span class="fn">Router.dispatch</span> <span class="loc">(server/router.rs:87)</span><br>
      &nbsp;&nbsp;at <span class="fn">Server.serve</span> <span class="loc">(server/main.rs:53)</span><br>
      <span class="loc">--- trace id: 7f3a-b2c1-e8d4 ---</span>
    </div>
  </div>
</body>
</html>"##;

#[no_mangle]
pub extern "C" fn x402_handle() {
    host_log(3, "error_500: serving 500 error page");
    respond(500, BODY, "text/html; charset=utf-8");
}
